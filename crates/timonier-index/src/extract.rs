//! Extraction of one file (ADR 0013): a pure function of its content and of the pack. The
//! result is what the cache keeps; names are resolved later, against the whole project.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use timonier_core::{ContentHash, EdgeKind, Position, Role, Span, SymbolKind};
use tree_sitter::{Node, Parser, QueryCursor, StreamingIterator, Tree};

use crate::pack::{Bind, Capture, Grammar, Pack};
use crate::tokens;
use crate::{Error, Result};

/// Bumped when the extraction itself changes meaning: it keys the cache with the pack version.
pub const EXTRACTOR_VERSION: u32 = 3;

/// What tree-sitter says about one file, before name resolution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileFacts {
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub references: Vec<ReferenceFact>,
    pub signals: Vec<SignalFact>,
    /// The tree has syntax errors: the file is being edited, or the grammar lags the language.
    pub has_errors: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolFact {
    /// Segments inside the file: enclosing definitions and containers, then the name.
    pub path: Vec<String>,
    pub kind: SymbolKind,
    pub span: Span,
    /// Where its name starts: LSP requests ask about a symbol at this position (ADR 0014).
    pub name_at: Position,
    pub signature: String,
    pub body_hash: ContentHash,
    /// Index of the parent symbol in `symbols`; `None` for the file.
    pub parent: Option<usize>,
}

impl SymbolFact {
    pub fn name(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }
}

/// One imported path, as written (`crate::node::Node`, `./host`), and the local names it binds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportFact {
    pub path: String,
    pub names: Vec<String>,
}

/// A path cited by the code, with its occurrences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceFact {
    /// `Calls`, `References`, `Implements` or `Inherits`.
    pub kind: EdgeKind,
    pub path: Vec<String>,
    /// For `Implements` and `Inherits`: the symbol of this file that implements or inherits.
    pub source: Option<usize>,
    /// A call on a receiver (`x.push()`, `this.n()`): its type is unknown to tree-sitter, so the
    /// call resolves only through an import or a path, never by its name alone (ADR 0013).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub receiver: bool,
    pub count: u32,
    /// Where the last segment of the path starts, at its first site: the language server is
    /// asked about this name once for all its sites (ADR 0022).
    pub at: Position,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalFact {
    pub role: Role,
    pub count: u32,
}

/// Parses `source` (from `old`, edited, when given) and extracts its facts.
pub(crate) fn extract(
    pack: &Pack,
    grammar: &Grammar,
    parser: &mut Parser,
    source: &[u8],
    old: Option<&Tree>,
    path: &str,
) -> Result<(FileFacts, Tree)> {
    parser
        .set_language(&grammar.language)
        .map_err(|_| Error::Parse {
            path: path.into(),
            grammar: grammar.name.clone(),
        })?;
    let tree = parser.parse(source, old).ok_or_else(|| Error::Parse {
        path: path.into(),
        grammar: grammar.name.clone(),
    })?;
    let facts = Extraction::new(pack, grammar, source).run(tree.root_node());
    Ok((facts, tree))
}

/// A path cited, as references are counted: its kind, its segments, the symbol that implements
/// or inherits, and whether it is a call on a receiver.
type CitedPath = (EdgeKind, Vec<String>, Option<usize>, bool);

/// A path cited at a node: the pattern that found it, the node, its kind and whether it is a
/// call on a receiver, and the `@reference.source` of the match.
type Cited<'t> = (usize, Node<'t>, (EdgeKind, bool), Option<Node<'t>>);

/// A definition or a container (`@scope`) found by the definitions query.
#[derive(Debug)]
struct Item<'t> {
    node: Node<'t>,
    pattern: usize,
    /// `Some` for a definition.
    definition: Option<Definition<'t>>,
    /// For a container: its segment and the segment its contents are ranged under.
    segment: String,
    parent_segment: Option<String>,
}

#[derive(Debug)]
struct Definition<'t> {
    kind: SymbolKind,
    body: Option<Node<'t>>,
    name_at: Position,
}

/// The symbols of a file, the path of every item, and the symbol kept for each path.
struct Outline {
    symbols: Vec<SymbolFact>,
    paths: Vec<Vec<String>>,
    index_of: HashMap<Vec<String>, usize>,
}

struct Extraction<'a> {
    pack: &'a Pack,
    grammar: &'a Grammar,
    source: &'a [u8],
}

impl<'a> Extraction<'a> {
    fn new(pack: &'a Pack, grammar: &'a Grammar, source: &'a [u8]) -> Self {
        Self {
            pack,
            grammar,
            source,
        }
    }

    fn text(&self, node: Node<'_>) -> &'a str {
        std::str::from_utf8(&self.source[node.byte_range()]).unwrap_or("")
    }

    /// Text with the pack's canonical spacing, for names and container segments.
    fn canonical(&self, node: Node<'_>) -> String {
        let manifest = &self.pack.manifest;
        let tokens = tokens::tokens(node, self.source, node.end_byte(), &manifest.fingerprint);
        tokens::canonical(&tokens, &manifest.signature)
    }

    fn run(&self, root: Node<'_>) -> FileFacts {
        let items = self.items(root);
        let outline = self.symbols(&items);
        let (imports, references) = self.references(root, &items, &outline);
        FileFacts {
            symbols: outline.symbols,
            imports,
            references,
            signals: self.signals(root),
            has_errors: root.has_error(),
        }
    }

    /// Definitions and containers, one per node (the first pattern wins), in document order.
    fn items<'t>(&self, root: Node<'t>) -> Vec<Item<'t>> {
        let q = &self.grammar.definitions;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q.query, root, self.source);
        let mut by_node: HashMap<usize, Item<'t>> = HashMap::new();
        while let Some(m) = matches.next() {
            let capture = |index: Option<u32>| {
                index.and_then(|i| m.captures().iter().find(|c| c.index == i).map(|c| c.node))
            };
            let item = if let Some(definition) = m
                .captures()
                .iter()
                .find_map(|c| q.kinds[c.index as usize].map(|kind| (c.node, kind)))
            {
                let Some(name) = capture(q.name) else {
                    continue;
                };
                let (node, kind) = definition;
                Item {
                    node,
                    pattern: m.pattern_index,
                    definition: Some(Definition {
                        kind,
                        body: capture(q.body),
                        name_at: point(name.start_position()),
                    }),
                    segment: self.canonical(name),
                    parent_segment: None,
                }
            } else if let Some(node) = capture(q.scope) {
                let render = |template: &str| {
                    let mut text = template.to_owned();
                    for c in m.captures() {
                        if let Some(part) = q.scope_parts.get(&c.index) {
                            text = text.replace(&format!("{{{part}}}"), &self.canonical(c.node));
                        }
                    }
                    text
                };
                let segment = match &q.segment[m.pattern_index] {
                    Some(template) => render(template),
                    None => match capture(q.name) {
                        Some(name) => self.canonical(name),
                        None => continue,
                    },
                };
                Item {
                    node,
                    pattern: m.pattern_index,
                    definition: None,
                    segment,
                    parent_segment: q.parent[m.pattern_index].as_deref().map(render),
                }
            } else {
                continue;
            };
            if item.segment.is_empty() {
                continue;
            }
            match by_node.get(&item.node.id()) {
                Some(existing) if existing.pattern <= item.pattern => {}
                _ => {
                    by_node.insert(item.node.id(), item);
                }
            }
        }
        let mut items: Vec<Item<'t>> = by_node.into_values().collect();
        // Outer items first when two start at the same byte.
        items.sort_by_key(|i| (i.node.start_byte(), std::cmp::Reverse(i.node.end_byte())));
        items
    }

    /// Symbols, deduplicated by path.
    fn symbols(&self, items: &[Item<'_>]) -> Outline {
        // Containment: items are sorted by start, outer first, so a stack finds the encloser.
        let mut enclosing = vec![None; items.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            while let Some(&top) = stack.last() {
                if items[top].node.end_byte() >= item.node.end_byte()
                    && items[top].node.start_byte() <= item.node.start_byte()
                {
                    break;
                }
                stack.pop();
            }
            enclosing[i] = stack.last().copied();
            stack.push(i);
        }
        let mut paths: Vec<Vec<String>> = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let mut path = enclosing[i].map(|e| paths[e].clone()).unwrap_or_default();
            path.push(item.segment.clone());
            paths.push(path);
        }
        // One symbol per path: the first with a body, else the first.
        let mut chosen: BTreeMap<&[String], usize> = BTreeMap::new();
        for (i, item) in items.iter().enumerate() {
            let Some(definition) = &item.definition else {
                continue;
            };
            match chosen.get(paths[i].as_slice()) {
                Some(&kept)
                    if items[kept]
                        .definition
                        .as_ref()
                        .is_some_and(|d| d.body.is_some()) => {}
                Some(_) if definition.body.is_none() => {}
                _ => {
                    chosen.insert(&paths[i], i);
                }
            }
        }
        let mut kept: Vec<usize> = chosen.values().copied().collect();
        kept.sort_unstable();
        let index_of: HashMap<Vec<String>, usize> = kept
            .iter()
            .enumerate()
            .map(|(s, &i)| (paths[i].clone(), s))
            .collect();
        let manifest = &self.pack.manifest;
        let symbols = kept
            .iter()
            .map(|&i| {
                let item = &items[i];
                let definition = item.definition.as_ref().expect("only definitions are kept");
                // The parent: the container's chosen symbol, else the nearest enclosing definition.
                let mut parent = None;
                let mut up = enclosing[i];
                while let Some(e) = up {
                    let outer = &items[e];
                    if outer.definition.is_some() {
                        parent = index_of.get(&paths[e]).copied();
                        break;
                    }
                    if let Some(segment) = &outer.parent_segment {
                        let mut candidate =
                            enclosing[e].map(|o| paths[o].clone()).unwrap_or_default();
                        candidate.push(segment.clone());
                        if let Some(&p) = index_of.get(&candidate) {
                            parent = Some(p);
                            break;
                        }
                    }
                    up = enclosing[e];
                }
                let node = item.node;
                let (head_end, body) = match definition.body {
                    Some(body) => (body.start_byte(), body),
                    None => (node.end_byte(), node),
                };
                let head = tokens::tokens(node, self.source, head_end, &manifest.fingerprint);
                let body_tokens =
                    tokens::tokens(body, self.source, body.end_byte(), &manifest.fingerprint);
                SymbolFact {
                    path: paths[i].clone(),
                    kind: definition.kind,
                    span: span(node),
                    name_at: definition.name_at,
                    signature: tokens::canonical(&head, &manifest.signature),
                    body_hash: tokens::hash(&body_tokens),
                    parent,
                }
            })
            .collect();
        Outline {
            symbols,
            paths,
            index_of,
        }
    }

    fn references(
        &self,
        root: Node<'_>,
        items: &[Item<'_>],
        outline: &Outline,
    ) -> (Vec<ImportFact>, Vec<ReferenceFact>) {
        let q = &self.grammar.references;
        let rules = &self.pack.manifest.paths;
        let mut import_roots: HashSet<usize> = HashSet::new();
        let mut import_paths: Vec<(Node<'_>, Option<String>)> = Vec::new();
        let mut import_names: Vec<Node<'_>> = Vec::new();
        // Per node: (pattern, kind, source).
        let mut cited: HashMap<usize, Cited<'_>> = HashMap::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q.query, root, self.source);
        while let Some(m) = matches.next() {
            let mut alias = None;
            let mut source = None;
            for c in m.captures() {
                match q.captures[c.index as usize] {
                    Some(Capture::ImportAlias) => alias = Some(self.text(c.node).trim().to_owned()),
                    Some(Capture::Source) => source = Some(c.node),
                    _ => {}
                }
            }
            for c in m.captures() {
                let kind = match q.captures[c.index as usize] {
                    Some(Capture::Import) => {
                        import_roots.insert(c.node.id());
                        continue;
                    }
                    Some(Capture::ImportPath) => {
                        // Once per node: a destructuring `require` matches once per name.
                        if !import_paths.iter().any(|(n, _)| n.id() == c.node.id()) {
                            import_paths.push((c.node, alias.clone()));
                        }
                        continue;
                    }
                    Some(Capture::ImportName) => {
                        import_names.push(c.node);
                        continue;
                    }
                    Some(Capture::Call) => (EdgeKind::Calls, false),
                    Some(Capture::Method) => (EdgeKind::Calls, true),
                    Some(Capture::Type) => (EdgeKind::References, false),
                    Some(Capture::Implements) => (EdgeKind::Implements, false),
                    Some(Capture::Inherits) => (EdgeKind::Inherits, false),
                    Some(Capture::ImportAlias | Capture::Source) | None => continue,
                };
                match cited.get(&c.node.id()) {
                    Some((pattern, ..)) if *pattern <= m.pattern_index => {}
                    _ => {
                        cited.insert(c.node.id(), (m.pattern_index, c.node, kind, source));
                    }
                }
            }
        }
        // The import statement a node belongs to: its nearest ancestor captured as `@import`.
        let root_of = |node: Node<'_>| -> Option<usize> {
            let mut up = Some(node);
            while let Some(n) = up {
                if import_roots.contains(&n.id()) {
                    return Some(n.id());
                }
                up = n.parent();
            }
            None
        };
        let mut names_by_root: HashMap<usize, Vec<String>> = HashMap::new();
        for node in &import_names {
            if let Some(r) = root_of(*node) {
                names_by_root
                    .entry(r)
                    .or_default()
                    .push(self.text(*node).trim().to_owned());
            }
        }
        let mut imports = Vec::new();
        import_paths.sort_by_key(|(n, _)| n.start_byte());
        for (node, alias) in &import_paths {
            let Some((path, binds)) = self.import_path(*node, &import_roots) else {
                continue;
            };
            let names = match rules.bind {
                Bind::Captures => root_of(*node)
                    .and_then(|r| names_by_root.get(&r).cloned())
                    .unwrap_or_default(),
                Bind::LastSegment => match (alias, binds) {
                    (Some(alias), _) if alias != "_" => vec![alias.clone()],
                    (Some(_), _) => Vec::new(),
                    (None, Some(name)) => vec![name],
                    (None, None) => Vec::new(),
                },
            };
            imports.push(ImportFact { path, names });
        }
        // References: one per node, outer paths win, nothing inside an import.
        let mut cited: Vec<(Node<'_>, (EdgeKind, bool), Option<Node<'_>>)> =
            cited.into_values().map(|(_, n, k, s)| (n, k, s)).collect();
        cited.sort_by_key(|(n, ..)| (n.start_byte(), std::cmp::Reverse(n.end_byte())));
        // Per cited path: its sites, and where the first one names it.
        let mut counts: BTreeMap<CitedPath, (u32, Position)> = BTreeMap::new();
        let mut outer_end = 0;
        for (node, (kind, receiver), source) in cited {
            if node.start_byte() < outer_end || root_of(node).is_some() {
                continue;
            }
            outer_end = node.end_byte();
            let path = split_path(self.text(node), &rules.separators);
            if path.is_empty() {
                continue;
            }
            let source = match (kind, source) {
                (EdgeKind::Implements | EdgeKind::Inherits, Some(source)) => {
                    match self.source_symbol(source, items, outline) {
                        Some(s) => Some(s),
                        None => continue,
                    }
                }
                (EdgeKind::Implements | EdgeKind::Inherits, None) => continue,
                _ => None,
            };
            let at = name_start(node, self.text(node), path.last().map_or(0, String::len));
            counts
                .entry((kind, path, source, receiver))
                .or_insert((0, at))
                .0 += 1;
        }
        let references = counts
            .into_iter()
            .map(
                |((kind, path, source, receiver), (count, at))| ReferenceFact {
                    kind,
                    path,
                    source,
                    receiver,
                    count,
                    at,
                },
            )
            .collect();
        (imports, references)
    }

    /// The full path of an imported node: the `prefix_field` of the enclosing import lists come
    /// first. Returns the path and the name it binds (none for `*`; the module for `self`).
    fn import_path(
        &self,
        node: Node<'_>,
        roots: &HashSet<usize>,
    ) -> Option<(String, Option<String>)> {
        let rules = &self.pack.manifest.paths;
        let separator = rules.separators.first().map_or("::", String::as_str);
        let mut segments: Vec<String> = vec![compact(self.text(node))];
        if let Some(field) = &rules.prefix_field {
            let mut child = node;
            let mut up = node.parent();
            while let Some(n) = up {
                if roots.contains(&child.id()) {
                    break;
                }
                if let Some(prefix) = n.child_by_field_name(field)
                    && prefix.id() != child.id()
                    && !(prefix.start_byte() <= node.start_byte()
                        && node.end_byte() <= prefix.end_byte())
                {
                    segments.insert(0, compact(self.text(prefix)));
                }
                if roots.contains(&n.id()) {
                    break;
                }
                child = n;
                up = n.parent();
            }
        }
        let mut path = segments.join(separator);
        if path.is_empty() {
            return None;
        }
        if let Some(own) = &rules.trailing_self {
            let suffix = format!("{separator}{own}");
            if let Some(module) = path.strip_suffix(&suffix) {
                path = module.to_owned();
            }
        }
        let last = path.rsplit(separator).next().unwrap_or(&path).to_owned();
        let binds = (last != "*").then_some(last);
        Some((path, binds))
    }

    /// The symbol of this file named by a `@reference.source`: the deepest type or trait with
    /// that name whose container encloses the reference.
    fn source_symbol(
        &self,
        node: Node<'_>,
        items: &[Item<'_>],
        outline: &Outline,
    ) -> Option<usize> {
        let paths = &outline.paths;
        let name = self.canonical(node);
        let contains = |i: &Item<'_>| {
            i.node.start_byte() <= node.start_byte() && node.end_byte() <= i.node.end_byte()
        };
        // The innermost definition that contains the node gives the context.
        let context: &[String] = items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.definition.is_some() && contains(i))
            .max_by_key(|(i, _)| paths[*i].len())
            .map_or(&[], |(i, _)| paths[i].as_slice());
        items
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                let path = &paths[*i];
                matches!(
                    item.definition.as_ref().map(|d| d.kind),
                    Some(SymbolKind::Type | SymbolKind::Trait)
                ) && path.last() == Some(&name)
                    && context.starts_with(&path[..path.len() - 1])
            })
            .max_by_key(|(i, _)| paths[*i].len())
            .and_then(|(i, _)| outline.index_of.get(&paths[i]).copied())
    }

    fn signals(&self, root: Node<'_>) -> Vec<SignalFact> {
        let q = &self.grammar.roles;
        let mut counts: BTreeMap<Role, u32> = BTreeMap::new();
        let mut seen: HashSet<(usize, Role)> = HashSet::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q.query, root, self.source);
        while let Some(m) = matches.next() {
            for c in m.captures() {
                if let Some(role) = q.roles[c.index as usize]
                    && seen.insert((c.node.id(), role))
                {
                    *counts.entry(role).or_default() += 1;
                }
            }
        }
        counts
            .into_iter()
            .map(|(role, count)| SignalFact { role, count })
            .collect()
    }
}

fn point(p: tree_sitter::Point) -> Position {
    let narrow = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Position::new(narrow(p.row), narrow(p.column))
}

/// Where the last segment of a cited path starts, in the node's own text: the path is that text
/// without its whitespace and its generic arguments (`split_path`), so the segment is the last
/// `len` bytes kept. `a::b::C` and `ns.T` end with it; `Tr<Q>` does not.
fn name_start(node: Node<'_>, text: &str, len: usize) -> Position {
    let start = node.start_position();
    let Some(offset) = last_segment_offset(text, len) else {
        return point(start);
    };
    let before = &text[..offset];
    let point_at = match before.rfind('\n') {
        Some(newline) => tree_sitter::Point::new(
            start.row + before.matches('\n').count(),
            offset - newline - 1,
        ),
        None => tree_sitter::Point::new(start.row, start.column + offset),
    };
    point(point_at)
}

/// The byte offset in `text` where the last `len` bytes `split_path` keeps begin.
fn last_segment_offset(text: &str, len: usize) -> Option<usize> {
    // The bytes kept, with where each comes from; a call or an index cuts what came before.
    let mut kept: Vec<usize> = Vec::with_capacity(text.len());
    let mut depth = 0_u32;
    for (at, c) in text.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            ')' | ']' if depth == 0 => kept.clear(),
            c if depth == 0 && !c.is_whitespace() => {
                kept.extend((at..at + c.len_utf8()).take(c.len_utf8()));
            }
            _ => {}
        }
    }
    kept.len()
        .checked_sub(len)
        .and_then(|first| kept.get(first).copied())
}

fn span(node: Node<'_>) -> Span {
    Span::new(point(node.start_position()), point(node.end_position()))
        .expect("tree-sitter ranges are ordered")
}

/// Text without whitespace.
fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

/// Segments of a path written in the code: whitespace and generic arguments removed, the
/// pack's separators split it, punctuation around a segment (`a?.b`, `x!`) is dropped.
pub(crate) fn split_path(text: &str, separators: &[String]) -> Vec<String> {
    let mut plain = String::with_capacity(text.len());
    let mut depth = 0_u32;
    for c in text.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            c if depth == 0 && !c.is_whitespace() => plain.push(c),
            _ => {}
        }
    }
    // A call or an index in the chain hides the receiver: `f(x).g` keeps `g`.
    if let Some(cut) = plain.rfind([')', ']']) {
        plain = plain[cut + 1..].to_owned();
    }
    let mut segments = vec![plain];
    for separator in separators {
        segments = segments
            .iter()
            .flat_map(|s| {
                s.split(separator.as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    segments
        .into_iter()
        .map(|s| {
            s.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                .to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_split_without_generics_or_punctuation() {
        let rust = ["::".to_owned()];
        let ts = [".".to_owned()];
        assert_eq!(split_path("Vec::<u8>::new", &rust), ["Vec", "new"]);
        assert_eq!(
            split_path("crate :: node::Node", &rust),
            ["crate", "node", "Node"]
        );
        assert_eq!(split_path("<T as Trait>::f", &rust), ["f"]);
        assert_eq!(split_path("a?.b!.c", &ts), ["a", "b", "c"]);
        assert_eq!(split_path("this.store", &ts), ["this", "store"]);
        assert!(split_path("<T>", &rust).is_empty());
        assert_eq!(
            split_path("createRoot(document.body).render", &ts),
            ["render"]
        );
        assert_eq!(split_path("items[0].name", &ts), ["name"]);
        assert_eq!(split_path("Foo::bar(x).baz", &rust), ["baz"]);
    }
}
