//! A SCIP index read into the graph (ADR 0073): its references and relationships become `Scip`
//! observations on Timonier's own identifiers (Q95), each occurrence placed in the smallest
//! symbol that contains it, as the language server's callees are (ADR 0014).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ::scip::types::{Document, Index, Occurrence, PositionEncoding, SymbolRole, occurrence};
use protobuf::Message;
use timonier_core::{
    ContentHash, EdgeKind, NodeId, Observation, Position, Provenance, ScopeKey, ScopeUpdate,
    SourceObservations,
};

/// The kinds a SCIP index verifies in each file it indexed (ADR 0073): its references always;
/// `Implements` and `Inherits` only from an index that records relationships at all, since
/// rust-analyzer records none and tree-sitter's must then stay.
pub const SCIP_KINDS: [EdgeKind; 4] = [
    EdgeKind::Calls,
    EdgeKind::References,
    EdgeKind::Implements,
    EdgeKind::Inherits,
];

/// An index larger than this is not read: the crate decodes it whole (ADR 0073).
pub const MAX_INDEX_BYTES: u64 = 256 * 1024 * 1024;

/// A file of the repository as the indexer read it: its node, and the hash of the content it
/// was given, taken before the indexer started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indexed {
    pub id: NodeId,
    pub content_hash: ContentHash,
}

/// What an index gave.
#[derive(Debug, Default, PartialEq)]
pub struct Read {
    /// One update per file of the repository the index covers.
    pub updates: Vec<ScopeUpdate>,
    /// The file-to-file uses it records: a file citing a symbol defined in another one. What
    /// the completeness check compares with tree-sitter's links (ADR 0073).
    pub links: BTreeSet<(String, String)>,
    /// Documents of the index that are no file of the model (generated, ignored, outside).
    pub skipped: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ScipError {
    #[error("the SCIP index does not parse: {0}")]
    Parse(#[from] protobuf::Error),
}

/// Reads `bytes`, an `index.scip` written at the repository root. `indexed` gives the files of
/// the model; `source` their bytes, to turn UTF-16 and UTF-32 columns into UTF-8 ones;
/// `locate` the smallest symbol of a file that contains a position.
pub fn read(
    bytes: &[u8],
    indexed: impl Fn(&str) -> Option<Indexed>,
    mut source: impl FnMut(&str) -> Option<Vec<u8>>,
    mut locate: impl FnMut(&str, Position) -> Option<NodeId>,
) -> Result<Read, ScipError> {
    let index = Index::parse_from_bytes(bytes)?;
    let mut out = Read::default();
    let mut files: Vec<(&Document, Indexed)> = Vec::new();
    let relationships = index
        .documents
        .iter()
        .flat_map(|d| &d.symbols)
        .any(|info| info.relationships.iter().any(|r| r.is_implementation));
    let verified: &[EdgeKind] = if relationships {
        &SCIP_KINDS
    } else {
        &SCIP_KINDS[..2]
    };
    for document in &index.documents {
        match indexed(&document.relative_path) {
            Some(file) => files.push((document, file)),
            None => out.skipped += 1,
        }
    }
    let definition = SymbolRole::Definition as i32;

    // Where each global symbol is defined, and what it is.
    let mut defined: HashMap<&str, (&str, Position)> = HashMap::new();
    let mut kinds: HashMap<&str, i32> = HashMap::new();
    for (document, _) in &files {
        let mut columns = Columns::new(document, &mut source);
        for occ in &document.occurrences {
            if occ.symbol_roles & definition != 0
                && ::scip::symbol::is_global_symbol(&occ.symbol)
                && let Some(at) = start(occ).map(|at| columns.utf8(at))
            {
                defined
                    .entry(occ.symbol.as_str())
                    .or_insert((document.relative_path.as_str(), at));
            }
        }
        for info in &document.symbols {
            kinds.insert(info.symbol.as_str(), info.kind.value());
        }
    }
    for info in &index.external_symbols {
        kinds
            .entry(info.symbol.as_str())
            .or_insert(info.kind.value());
    }
    let is_callable = |symbol: &str| match kinds.get(symbol).copied() {
        Some(kind) if kind != 0 => CALLABLE.contains(&kind),
        // No kind: a method or function descriptor ends with `().` (SCIP's symbol syntax).
        _ => symbol.ends_with(")."),
    };
    let is_interface = |symbol: &str| {
        kinds
            .get(symbol)
            .is_some_and(|kind| INTERFACES.contains(kind))
    };

    for (document, file) in &files {
        let path = document.relative_path.as_str();
        let mut columns = Columns::new(document, &mut source);
        let mut edges: BTreeMap<(NodeId, NodeId, EdgeKind), u32> = BTreeMap::new();
        for occ in &document.occurrences {
            if occ.symbol_roles & definition != 0 || !::scip::symbol::is_global_symbol(&occ.symbol)
            {
                continue;
            }
            let Some(&(to_path, to_at)) = defined.get(occ.symbol.as_str()) else {
                // Defined outside the repository: no edge.
                continue;
            };
            if to_path != path {
                out.links.insert((path.to_owned(), to_path.to_owned()));
            }
            let Some(at) = start(occ).map(|at| columns.utf8(at)) else {
                continue;
            };
            let from = locate(path, at);
            let callee = is_callable(&occ.symbol)
                .then(|| locate(to_path, to_at))
                .flatten();
            match (from, callee) {
                // A call between symbols, the only kind of edge between symbols (spec).
                (Some(from), Some(to)) if from != to => {
                    *edges.entry((from, to, EdgeKind::Calls)).or_default() += 1;
                }
                (Some(_), Some(_)) => {}
                // Otherwise a reference, between files, as tree-sitter and the LSP keep them.
                _ if to_path != path => {
                    if let Some(to) = indexed(to_path) {
                        *edges
                            .entry((file.id.clone(), to.id, EdgeKind::References))
                            .or_default() += 1;
                    }
                }
                _ => {}
            }
        }
        // `A implements B`, between types, as tree-sitter sees it: a method that implements
        // another is no edge of the map.
        for info in document.symbols.iter().filter(|i| !is_callable(&i.symbol)) {
            let Some(&(from_path, from_at)) = defined.get(info.symbol.as_str()) else {
                continue;
            };
            for relation in info.relationships.iter().filter(|r| r.is_implementation) {
                let Some(&(to_path, to_at)) = defined.get(relation.symbol.as_str()) else {
                    continue;
                };
                if let (Some(from), Some(to)) = (locate(from_path, from_at), locate(to_path, to_at))
                    && from != to
                {
                    let kind = if is_interface(&relation.symbol) {
                        EdgeKind::Implements
                    } else {
                        EdgeKind::Inherits
                    };
                    *edges.entry((from, to, kind)).or_default() += 1;
                }
            }
        }
        let Ok(scope) = ScopeKey::of_file(&file.id) else {
            continue;
        };
        let observations = edges
            .into_iter()
            .map(|((src, dst, kind), sites)| Observation::new(src, dst, kind).with_weight(sites))
            .collect();
        out.updates.push(
            ScopeUpdate::new(scope).with_observations(
                SourceObservations::new(Provenance::Scip, observations)
                    .verifying(verified.iter().copied())
                    .computed_on(file.content_hash),
            ),
        );
    }
    Ok(out)
}

/// Confirms tree-sitter's imports with the index (ADR 0073): no indexer marks its import
/// occurrences (`SymbolRole::Import`; none of rust-analyzer, scip-typescript, scip-python,
/// scip-go, scip-java, scip-dotnet does), so an import from A to B is confirmed when A cites a
/// symbol defined in B. It becomes a `Scip` edge of the same weight, which fusion prefers; an
/// import the index does not confirm stays tree-sitter's, `Imports` being no verified kind.
/// Returns how many were confirmed.
pub fn confirm_imports(read: &mut Read, imports: &BTreeMap<String, Vec<(String, u32)>>) -> usize {
    let mut confirmed = 0;
    for update in &mut read.updates {
        let (Some(from), Some(observations)) = (update.scope.file(), &mut update.observations)
        else {
            continue;
        };
        let Some(path) = from.as_str().strip_prefix("file:") else {
            continue;
        };
        for (to, weight) in imports.get(path).into_iter().flatten() {
            if read.links.contains(&(path.to_owned(), to.clone()))
                && let Ok(to) = NodeId::file(to)
            {
                observations.observations.push(
                    Observation::new(from.clone(), to, EdgeKind::Imports).with_weight(*weight),
                );
                confirmed += 1;
            }
        }
    }
    confirmed
}

/// The share of tree-sitter's file links the index confirms: under 70 %, the index is judged
/// incomplete and not applied (ADR 0073). `None` when tree-sitter sees no link to compare with.
pub fn confirmed_share(
    index: &BTreeSet<(String, String)>,
    tree_sitter: &BTreeSet<(String, String)>,
) -> Option<f64> {
    if tree_sitter.is_empty() {
        return None;
    }
    let confirmed = tree_sitter
        .iter()
        .filter(|link| index.contains(*link))
        .count();
    Some(confirmed as f64 / tree_sitter.len() as f64)
}

/// tree-sitter's file links against an index's, on the files both know (PLAN.md 11.1): how
/// many of tree-sitter's are right (precision) and how many of the index's it finds (recall).
/// Leniently, a link to a file that only gathers its folder (`mod.rs`, `__init__.py`,
/// `index.ts`) stands for the index's links to the files of that folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub files: usize,
    pub index_links: usize,
    pub tree_sitter_links: usize,
    pub precision: f64,
    pub recall: f64,
    pub lenient_precision: f64,
    pub lenient_recall: f64,
}

/// The files that gather their folder, for the lenient comparison.
const HUBS: [&str; 7] = [
    "__init__.py",
    "mod.rs",
    "lib.rs",
    "main.rs",
    "index.ts",
    "index.tsx",
    "index.js",
];

/// Compares tree-sitter's file links with what an index read, on the files it indexed.
pub fn compare(read: &Read, tree_sitter: &BTreeSet<(String, String)>) -> Comparison {
    let files: BTreeSet<String> = read
        .updates
        .iter()
        .filter_map(|u| u.scope.file())
        .filter_map(|f| f.as_str().strip_prefix("file:").map(str::to_owned))
        .collect();
    let both = |(a, b): &(String, String)| files.contains(a) && files.contains(b);
    let index: BTreeSet<&(String, String)> = read.links.iter().filter(|l| both(l)).collect();
    let ours: BTreeSet<&(String, String)> = tree_sitter.iter().filter(|l| both(l)).collect();
    let share = |part: usize, all: usize| {
        if all == 0 {
            0.0
        } else {
            part as f64 / all as f64
        }
    };
    let hit = ours.iter().filter(|l| index.contains(*l)).count();
    let folder = |path: &str| {
        path.rsplit_once('/')
            .map_or(String::new(), |(d, _)| format!("{d}/"))
    };
    let mut covered: BTreeSet<&(String, String)> = BTreeSet::new();
    let mut good = 0;
    for link in &ours {
        let (a, b) = (&link.0, &link.1);
        let stands: Vec<&(String, String)> = if index.contains(*link) {
            vec![*link]
        } else if HUBS.iter().any(|h| b.rsplit('/').next() == Some(*h)) {
            let dir = folder(b);
            index
                .iter()
                .copied()
                .filter(|(x, y)| x == a && y.starts_with(&dir))
                .collect()
        } else {
            Vec::new()
        };
        if !stands.is_empty() {
            good += 1;
            covered.extend(stands);
        }
    }
    Comparison {
        files: files.len(),
        index_links: index.len(),
        tree_sitter_links: ours.len(),
        precision: share(hit, ours.len()),
        recall: share(hit, index.len()),
        lenient_precision: share(good, ours.len()),
        lenient_recall: share(covered.len(), index.len()),
    }
}

/// The confirmed share under which an index is not applied (ADR 0073).
pub const COMPLETE_ENOUGH: f64 = 0.7;

/// SCIP's kinds of callables: what a reference makes a call to.
const CALLABLE: [i32; 6] = [
    ::scip::types::symbol_information::Kind::Function as i32,
    ::scip::types::symbol_information::Kind::Method as i32,
    ::scip::types::symbol_information::Kind::Constructor as i32,
    ::scip::types::symbol_information::Kind::StaticMethod as i32,
    ::scip::types::symbol_information::Kind::AbstractMethod as i32,
    ::scip::types::symbol_information::Kind::Macro as i32,
];

/// What a type implements, rather than inherits from.
const INTERFACES: [i32; 3] = [
    ::scip::types::symbol_information::Kind::Interface as i32,
    ::scip::types::symbol_information::Kind::Trait as i32,
    ::scip::types::symbol_information::Kind::Protocol as i32,
];

/// The start of an occurrence: `typed_range` when it is there, else the deprecated `range`.
fn start(occ: &Occurrence) -> Option<(u32, u32)> {
    let (line, character) = match &occ.typed_range {
        Some(occurrence::Typed_range::SingleLineRange(r)) => (r.line, r.start_character),
        Some(occurrence::Typed_range::MultiLineRange(r)) => (r.start_line, r.start_character),
        // None, or a form newer than this crate: the deprecated `range`, which stays filled.
        _ => (*occ.range.first()?, *occ.range.get(1)?),
    };
    Some((u32::try_from(line).ok()?, u32::try_from(character).ok()?))
}

/// A document's columns turned into UTF-8 byte offsets (ADR 0008), its lines read once.
struct Columns {
    encoding: PositionEncoding,
    lines: Option<Vec<String>>,
}

impl Columns {
    fn new(document: &Document, source: &mut impl FnMut(&str) -> Option<Vec<u8>>) -> Self {
        // Unspecified: LSP's UTF-16, which the indexers built on language servers emit.
        let encoding = document
            .position_encoding
            .enum_value()
            .unwrap_or(PositionEncoding::UnspecifiedPositionEncoding);
        let lines = (encoding != PositionEncoding::UTF8CodeUnitOffsetFromLineStart)
            .then(|| source(&document.relative_path))
            .flatten()
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .split('\n')
                    .map(str::to_owned)
                    .collect()
            });
        Self { encoding, lines }
    }

    fn utf8(&mut self, (line, character): (u32, u32)) -> Position {
        let Some(text) = self.lines.as_ref().and_then(|l| l.get(line as usize)) else {
            return Position::new(line, character);
        };
        let mut units = 0u32;
        let mut bytes = 0u32;
        for c in text.chars() {
            if units >= character {
                break;
            }
            units += match self.encoding {
                PositionEncoding::UTF32CodeUnitOffsetFromLineStart => 1,
                _ => c.len_utf16() as u32,
            };
            bytes += c.len_utf8() as u32;
        }
        Position::new(line, bytes)
    }
}
