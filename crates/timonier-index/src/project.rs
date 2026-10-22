//! The project model (ADR 0013): packages, files and their facts, in memory; the approximate
//! resolution of the paths they cite; the scope updates for the store.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use timonier_core::{
    Confidence, ContentHash, EdgeKind, Language, Node, NodeId, NodeKind, Observation, Position,
    Provenance, ScopeKey, ScopeUpdate, SourceObservations, SymbolKind,
};

use crate::components::Components;
use crate::ecosystem::{self, ModulePath, PackageInfo};
use crate::lsp::{Lookup, Usage};
use crate::pack::{Ecosystem, Naming, Packs, RoleRule};
use crate::{FileFacts, Result};

/// Confidence of a tree-sitter edge (ADR 0013).
const IMPORT: f32 = 0.9;
const BY_PATH: f32 = 0.8;
const BY_NAME: f32 = 0.5;

/// Language rules the model needs, copied from the packs.
#[derive(Clone, Debug)]
pub(crate) struct LanguageRules {
    pub(crate) naming: Naming,
    pub(crate) ecosystem: Ecosystem,
    pub(crate) separators: Vec<String>,
    pub(crate) roles: Vec<RoleRule>,
    /// Names no repository defines: not asked about when tree-sitter leaves them (ADR 0022).
    pub(crate) outside: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub id: NodeId,
    pub language: Language,
    /// Index in `Project::packages`; `None` outside any package (Q14).
    pub package: Option<usize>,
    pub module: ModulePath,
    pub content_hash: ContentHash,
    pub facts: FileFacts,
}

impl FileEntry {
    /// The root of its module path: the crate (or Rust target) it belongs to.
    pub fn target(&self) -> &[String] {
        &self.module.segments[..self.module.root_len.min(self.module.segments.len())]
    }
}

/// Which updates a change calls for.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Affected {
    /// The structure scope changed (a file appeared or disappeared).
    pub structure: bool,
    /// Files whose resolution may have changed, the changed file included.
    pub files: BTreeSet<String>,
    /// Other files that cite a symbol which disappeared: their LSP verification may be out of
    /// date (ADR 0014).
    pub dependents: BTreeSet<String>,
    /// Symbols whose signature changed, or that appeared: the language server says which files
    /// use them (ADR 0022).
    pub lookups: Vec<Lookup>,
}

#[derive(Debug)]
pub struct Project {
    pub(crate) repo: NodeId,
    pub(crate) repo_name: String,
    pub(crate) rules: HashMap<Language, LanguageRules>,
    /// (ecosystem, external package) → its usage category (ADR 0017).
    pub(crate) categories: HashMap<(Ecosystem, String), String>,
    pub(crate) packages: Vec<PackageInfo>,
    pub(crate) files: BTreeMap<String, FileEntry>,
    pub(crate) components: Components,
    /// Module path (per language) → file.
    modules: HashMap<(Language, Vec<String>), String>,
    /// (package, name) → files that define a symbol of that name (fields excepted).
    names: HashMap<(Option<usize>, String), BTreeSet<String>>,
    /// Name → files whose imports or references cite it, as any segment.
    citing: HashMap<String, BTreeSet<String>>,
}

/// What the updates of one batch share.
pub(crate) struct Context {
    pub(crate) kept: BTreeSet<usize>,
    pub(crate) externals: BTreeMap<(Ecosystem, String), NodeId>,
}

/// Where a cited path leads.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    File(String),
    /// A package of the repository, when no file of it was found.
    Package(usize),
    /// A declared dependency, by registry name.
    External(String),
}

impl Project {
    pub fn new(repo_name: &str, packs: &Packs, packages: Vec<PackageInfo>) -> Result<Self> {
        let rules = packs
            .iter()
            .map(|pack| {
                (
                    pack.language.clone(),
                    LanguageRules {
                        naming: pack.manifest.naming.clone(),
                        ecosystem: pack.manifest.ecosystem,
                        separators: pack.manifest.paths.separators.clone(),
                        roles: pack.manifest.roles.clone(),
                        outside: pack.manifest.lsp.outside.iter().cloned().collect(),
                    },
                )
            })
            .collect();
        let mut categories = HashMap::new();
        for pack in packs.iter() {
            for (category, names) in &pack.manifest.dependencies {
                for name in names {
                    categories
                        .entry((pack.manifest.ecosystem, name.clone()))
                        .or_insert_with(|| category.clone());
                }
            }
        }
        Ok(Self {
            repo: NodeId::repo(repo_name)?,
            repo_name: repo_name.to_owned(),
            rules,
            categories,
            packages,
            files: BTreeMap::new(),
            components: Components::default(),
            modules: HashMap::new(),
            names: HashMap::new(),
            citing: HashMap::new(),
        })
    }

    pub fn repo(&self) -> &NodeId {
        &self.repo
    }

    pub fn packages(&self) -> &[PackageInfo] {
        &self.packages
    }

    pub fn files(&self) -> impl Iterator<Item = &FileEntry> {
        self.files.values()
    }

    pub fn file(&self, path: &str) -> Option<&FileEntry> {
        self.files.get(path)
    }

    /// The file at `path`, or the files under the directory `path`, by a range of the sorted
    /// map: no walk through every file.
    pub fn paths_under<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        let prefix = format!("{path}/");
        let exact = self.files.get_key_value(path).map(|(p, _)| p.as_str());
        let inside = self
            .files
            .range(prefix.clone()..)
            .map(|(p, _)| p.as_str())
            .take_while(move |p| p.starts_with(&prefix));
        exact.into_iter().chain(inside)
    }

    /// Replaces the packages (a manifest changed): every file is placed and named again.
    pub fn set_packages(&mut self, packages: Vec<PackageInfo>) -> Result<Affected> {
        self.packages = packages;
        let entries: Vec<FileEntry> = std::mem::take(&mut self.files).into_values().collect();
        self.modules.clear();
        self.names.clear();
        self.citing.clear();
        let mut affected = Affected {
            structure: true,
            ..Affected::default()
        };
        for entry in entries {
            affected.files.insert(entry.path.clone());
            self.insert(entry.path, entry.language, entry.content_hash, entry.facts)?;
        }
        Ok(affected)
    }

    /// Adds or replaces a file and its facts.
    pub fn upsert(
        &mut self,
        path: &str,
        language: Language,
        content_hash: ContentHash,
        facts: FileFacts,
    ) -> Result<Affected> {
        let old = self.unindex(path);
        let structure = old.is_none();
        let new_names = defined_names_of(&facts);
        let mut changed: BTreeSet<String> = match &old {
            Some(old) => {
                let mut changed: BTreeSet<String> = defined_names(old)
                    .symmetric_difference(&new_names)
                    .cloned()
                    .collect();
                // A re-export that now leads elsewhere keeps its name, not its target.
                changed.extend(rebound_names(&old.facts, &facts));
                changed
            }
            None => new_names,
        };
        let mut signed = changed.clone();
        if let Some(old) = &old {
            signed.extend(signature_changes(&old.facts, &facts));
        }
        self.insert(path.to_owned(), language, content_hash, facts)?;
        if structure {
            changed.extend(self.files[path].module.segments.last().cloned());
        }
        // A symbol still here: the language server names its dependents, the files citing its
        // name if it cannot. One gone: the files citing its name. A file that uses a symbol
        // cites its name: with no other file citing it, there is nothing to look up.
        let mut dependents = BTreeSet::new();
        let mut lookups = Vec::new();
        let file = &self.files[path];
        for name in &signed {
            let mut by_name = self.citing_any(&BTreeSet::from([name.clone()]), path);
            by_name.remove(path);
            if by_name.is_empty() {
                continue;
            }
            let mut found = false;
            for (index, symbol) in file.facts.symbols.iter().enumerate() {
                if symbol.name() != name {
                    continue;
                }
                let Some(id) = self.symbol_id(file, index) else {
                    continue;
                };
                found = true;
                lookups.push(Lookup {
                    path: path.to_owned(),
                    symbol: id,
                    at: symbol.name_at,
                    content_hash,
                    by_name: by_name.clone(),
                });
            }
            if !found {
                dependents.extend(by_name);
            }
        }
        Ok(Affected {
            structure,
            files: self.citing_any(&changed, path),
            dependents,
            lookups,
        })
    }

    /// Removes a file.
    pub fn remove(&mut self, path: &str) -> Affected {
        let Some(old) = self.unindex(path) else {
            return Affected::default();
        };
        let mut changed = defined_names(&old);
        changed.extend(old.module.segments.last().cloned());
        let mut files = self.citing_any(&changed, path);
        files.remove(path);
        Affected {
            structure: true,
            dependents: files.clone(),
            files,
            lookups: Vec::new(),
        }
    }

    /// The files whose code cites `name` (ADR 0022: the users of a symbol are among them).
    pub fn citing(&self, name: &str) -> impl Iterator<Item = &String> {
        self.citing.get(name).into_iter().flatten()
    }

    fn citing_any(&self, names: &BTreeSet<String>, path: &str) -> BTreeSet<String> {
        let mut files: BTreeSet<String> = names
            .iter()
            .filter_map(|n| self.citing.get(n))
            .flatten()
            .cloned()
            .collect();
        if self.files.contains_key(path) {
            files.insert(path.to_owned());
        }
        files
    }

    fn insert(
        &mut self,
        path: String,
        language: Language,
        content_hash: ContentHash,
        facts: FileFacts,
    ) -> Result<()> {
        let rules = self.rules.get(&language).cloned();
        let ecosystem = rules.as_ref().map(|r| r.ecosystem);
        let package = self.package_of(&path, ecosystem);
        let relative = match package {
            Some(p) if !self.packages[p].dir.is_empty() => {
                path[self.packages[p].dir.len() + 1..].to_owned()
            }
            _ => path.clone(),
        };
        let info = package.map(|p| &self.packages[p]);
        let module = match ecosystem {
            Some(Ecosystem::Cargo) => {
                ecosystem::rust_module(info, info.is_some_and(|i| i.has_lib), &relative)
            }
            _ => ecosystem::npm_module(info, &relative),
        };
        let entry = FileEntry {
            id: NodeId::file(&path)?,
            path: path.clone(),
            language: language.clone(),
            package,
            module,
            content_hash,
            facts,
        };
        self.modules.insert(
            (language.clone(), entry.module.segments.clone()),
            path.clone(),
        );
        for symbol in &entry.facts.symbols {
            if symbol.kind != SymbolKind::Field {
                self.names
                    .entry((package, symbol.name().to_owned()))
                    .or_default()
                    .insert(path.clone());
            }
        }
        let separators = rules.map(|r| r.separators).unwrap_or_default();
        for name in cited_names(&entry.facts, &separators) {
            self.citing.entry(name).or_default().insert(path.clone());
        }
        self.files.insert(path, entry);
        Ok(())
    }

    fn unindex(&mut self, path: &str) -> Option<FileEntry> {
        let old = self.files.remove(path)?;
        self.modules
            .remove(&(old.language.clone(), old.module.segments.clone()));
        for symbol in &old.facts.symbols {
            if let Some(files) = self.names.get_mut(&(old.package, symbol.name().to_owned())) {
                files.remove(path);
            }
        }
        let separators = self
            .rules
            .get(&old.language)
            .map(|r| r.separators.clone())
            .unwrap_or_default();
        for name in cited_names(&old.facts, &separators) {
            if let Some(files) = self.citing.get_mut(&name) {
                files.remove(path);
            }
        }
        Some(old)
    }

    /// The deepest package of the file's ecosystem whose directory contains it.
    fn package_of(&self, path: &str, ecosystem: Option<Ecosystem>) -> Option<usize> {
        self.packages
            .iter()
            .enumerate()
            .filter(|(_, p)| Some(p.ecosystem) == ecosystem && ecosystem::contains(&p.dir, path))
            .max_by_key(|(_, p)| p.dir.len())
            .map(|(i, _)| i)
    }

    /// What every update of a batch shares: kept and external packages.
    pub(crate) fn context(&self) -> Context {
        Context {
            kept: self.kept_packages(),
            externals: self.externals(),
        }
    }

    /// The updates of a batch: the structure if asked, the full content of `changed` files,
    /// and only the edges of `resolve` files.
    pub fn updates(
        &self,
        structure: bool,
        changed: &BTreeSet<String>,
        resolve: &BTreeSet<String>,
    ) -> Vec<ScopeUpdate> {
        let context = self.context();
        let mut updates = Vec::new();
        if structure {
            updates.push(self.structure_with(&context));
        }
        for path in changed {
            if let Some(update) = self.file_update_with(path, &context) {
                updates.push(update);
            }
        }
        for path in resolve.difference(changed) {
            if let Some(file) = self.files.get(path)
                && let Ok(scope) = ScopeKey::of_file(&file.id)
            {
                updates.push(
                    ScopeUpdate::new(scope).with_observations(self.observations(file, &context)),
                );
            }
        }
        updates
    }

    /// Packages that own at least one file: the others are not on the map (ADR 0013).
    fn kept_packages(&self) -> BTreeSet<usize> {
        self.files.values().filter_map(|f| f.package).collect()
    }

    /// External packages on the map: runtime dependencies of kept packages that are not
    /// packages of the repository.
    fn externals(&self) -> BTreeMap<(Ecosystem, String), NodeId> {
        let internal: BTreeSet<(Ecosystem, &str)> = self
            .packages
            .iter()
            .map(|p| (p.ecosystem, p.name.as_str()))
            .collect();
        let mut externals = BTreeMap::new();
        for p in self.kept_packages() {
            let package = &self.packages[p];
            for dependency in package.dependencies.iter().filter(|d| d.runtime) {
                if internal.contains(&(package.ecosystem, dependency.name.as_str())) {
                    continue;
                }
                if let Ok(id) = NodeId::package(package.ecosystem.as_str(), &dependency.name) {
                    externals.insert((package.ecosystem, dependency.name.clone()), id);
                }
            }
        }
        externals
    }

    /// The structure scope: repo, packages, external packages, files; and the manifests'
    /// `DependsOn` edges.
    pub fn structure(&self) -> ScopeUpdate {
        self.structure_with(&self.context())
    }

    /// The scope of a file's content: its hash, its symbols and its tree-sitter edges.
    pub fn file_update(&self, path: &str) -> Option<ScopeUpdate> {
        self.file_update_with(path, &self.context())
    }

    fn file_update_with(&self, path: &str, context: &Context) -> Option<ScopeUpdate> {
        let file = self.files.get(path)?;
        let scope = ScopeKey::of_file(&file.id).ok()?;
        Some(
            ScopeUpdate::new(scope)
                .with_content_hash(file.content_hash)
                .with_nodes(self.symbol_nodes(file))
                .with_observations(self.observations(file, context)),
        )
    }

    /// The file of every symbol: where an LSP callee lives, after a restart (ADR 0018).
    pub fn symbol_files(&self) -> HashMap<NodeId, String> {
        let mut files = HashMap::new();
        for file in self.files.values() {
            for index in 0..file.facts.symbols.len() {
                if let Some(id) = self.symbol_id(file, index) {
                    files.insert(id, file.path.clone());
                }
            }
        }
        files
    }

    /// Where the name of `symbol` starts in the current content of `path`: a lookup made on an
    /// older content finds its symbol again (ADR 0022).
    pub fn name_at(&self, path: &str, symbol: &NodeId) -> Option<Position> {
        let file = self.files.get(path)?;
        (0..file.facts.symbols.len())
            .find(|i| self.symbol_id(file, *i).as_ref() == Some(symbol))
            .map(|i| file.facts.symbols[i].name_at)
    }

    /// The qualified id of a symbol of a file (ADR 0013).
    pub fn symbol_id(&self, file: &FileEntry, symbol: usize) -> Option<NodeId> {
        let naming = &self.rules.get(&file.language)?.naming;
        let symbol = file.facts.symbols.get(symbol)?;
        let mut name = file.module.segments.join(&naming.module);
        if !name.is_empty() {
            name.push_str(&naming.join);
        }
        name.push_str(&symbol.path.join(&naming.member));
        NodeId::qualified(&file.language, &name).ok()
    }

    fn symbol_nodes(&self, file: &FileEntry) -> Vec<Node> {
        let ids: Vec<Option<NodeId>> = (0..file.facts.symbols.len())
            .map(|i| self.symbol_id(file, i))
            .collect();
        file.facts
            .symbols
            .iter()
            .zip(&ids)
            .filter_map(|(symbol, id)| {
                let parent = match symbol.parent {
                    Some(p) => ids[p].clone()?,
                    None => file.id.clone(),
                };
                Some(
                    Node::new(id.clone()?, NodeKind::Symbol(symbol.kind), symbol.name())
                        .with_parent(parent)
                        .with_language(file.language.clone())
                        .with_span(symbol.span)
                        .with_signature(symbol.signature.clone())
                        .with_body_hash(symbol.body_hash),
                )
            })
            .collect()
    }

    /// The tree-sitter edges of a file, resolved against the project.
    pub(crate) fn observations(&self, file: &FileEntry, context: &Context) -> SourceObservations {
        let (kept, externals) = (&context.kept, &context.externals);
        let mut edges: BTreeMap<(NodeId, NodeId, EdgeKind), (u32, f32)> = BTreeMap::new();
        let mut add = |src: NodeId, dst: NodeId, kind, weight: u32, confidence: f32| {
            let entry = edges.entry((src, dst, kind)).or_insert((0, 0.0));
            entry.0 += weight;
            entry.1 = entry.1.max(confidence);
        };
        let package_node = |target: &Target| -> Option<NodeId> {
            match target {
                Target::Package(p) if kept.contains(p) => Some(self.packages[*p].id.clone()),
                Target::External(name) => {
                    let ecosystem = file.package.map(|p| self.packages[p].ecosystem)?;
                    externals.get(&(ecosystem, name.clone())).cloned()
                }
                _ => None,
            }
        };
        for import in &file.facts.imports {
            match self.resolve_import(file, &import.path) {
                Some(Target::File(path)) if path != file.path => {
                    add(
                        file.id.clone(),
                        self.files[&path].id.clone(),
                        EdgeKind::Imports,
                        1,
                        IMPORT,
                    );
                }
                Some(target) => {
                    if let Some(dst) = package_node(&target) {
                        add(file.id.clone(), dst, EdgeKind::Imports, 1, IMPORT);
                    }
                }
                None => {}
            }
        }
        for reference in &file.facts.references {
            let Some((target, confidence)) =
                self.resolve_reference(file, &reference.path, reference.receiver)
            else {
                continue;
            };
            match reference.kind {
                EdgeKind::Implements | EdgeKind::Inherits => {
                    let (Some(source), Target::File(path)) = (reference.source, &target) else {
                        continue;
                    };
                    let Some(src) = self.symbol_id(file, source) else {
                        continue;
                    };
                    let Some(dst) =
                        self.type_named(path, reference.path.last().map_or("", String::as_str))
                    else {
                        continue;
                    };
                    if src != dst {
                        add(src, dst, reference.kind, reference.count, confidence);
                    }
                }
                kind => {
                    if let Target::File(path) = target
                        && path != file.path
                    {
                        add(
                            file.id.clone(),
                            self.files[&path].id.clone(),
                            kind,
                            reference.count,
                            confidence,
                        );
                    }
                }
            }
        }
        let observations = edges
            .into_iter()
            .map(|((src, dst, kind), (weight, confidence))| {
                Observation::new(src, dst, kind)
                    .with_weight(weight)
                    .with_confidence(
                        Confidence::new(confidence).expect("tree-sitter confidences are in [0, 1]"),
                    )
            })
            .collect();
        SourceObservations::new(Provenance::TreeSitter, observations).computed_on(file.content_hash)
    }

    /// The usages of a file the language server is asked about (ADR 0022): one per path cited,
    /// at its first site, not one per site. Left out: a path tree-sitter finds in the file
    /// itself (no edge between files), one it leads to a dependency outside the repository, an
    /// unresolved one whose first segment the pack says is defined outside any repository, and
    /// an unresolved name of one character (`_`, a type parameter such as `T`): declared in the
    /// item itself.
    ///
    /// Paths are not grouped further: two paths with the same first segment may be defined in
    /// two files (a re-export by `*` between them, an `impl` in another file), and the answer
    /// for one would replace a right edge of the other.
    pub(crate) fn usages(&self, file: &FileEntry) -> Vec<Usage> {
        let outside = self.rules.get(&file.language).map(|r| &r.outside);
        let mut usages = Vec::new();
        for reference in &file.facts.references {
            let source = match reference.kind {
                EdgeKind::References => file.id.clone(),
                EdgeKind::Implements | EdgeKind::Inherits => {
                    match reference.source.and_then(|s| self.symbol_id(file, s)) {
                        Some(source) => source,
                        None => continue,
                    }
                }
                _ => continue,
            };
            let Some(first) = reference.path.first() else {
                continue;
            };
            let elsewhere = match self.resolve_reference(file, &reference.path, reference.receiver)
            {
                Some((Target::External(_), _)) => continue,
                // Within the file, a `References` makes no edge. An `Implements` does: asked.
                Some((Target::File(path), _))
                    if path == file.path && reference.kind == EdgeKind::References =>
                {
                    continue;
                }
                // Bound by an import or a path: there, a server that answers « here » points at
                // the import, and does not know yet. By a name alone, tree-sitter guesses: the
                // server's « here » corrects it.
                Some((Target::File(path), confidence)) => {
                    path != file.path && confidence >= BY_PATH
                }
                Some((Target::Package(_), _)) => true,
                None if first.chars().count() == 1
                    || outside.is_some_and(|o| o.contains(first)) =>
                {
                    continue;
                }
                None => false,
            };
            usages.push(Usage {
                kind: reference.kind,
                source,
                at: reference.at,
                sites: reference.count,
                elsewhere,
            });
        }
        usages
    }

    /// The type or trait of that name in a file, if exactly one.
    fn type_named(&self, path: &str, name: &str) -> Option<NodeId> {
        let file = self.files.get(path)?;
        let mut found = file.facts.symbols.iter().enumerate().filter(|(_, s)| {
            s.name() == name && matches!(s.kind, SymbolKind::Type | SymbolKind::Trait)
        });
        let (index, _) = found.next()?;
        if found.next().is_some() {
            return None;
        }
        self.symbol_id(file, index)
    }

    fn ecosystem_of(&self, file: &FileEntry) -> Option<Ecosystem> {
        self.rules.get(&file.language).map(|r| r.ecosystem)
    }

    fn resolve_import(&self, file: &FileEntry, path: &str) -> Option<Target> {
        match self.ecosystem_of(file)? {
            Ecosystem::Cargo => {
                let mut segments: Vec<String> = path.split("::").map(str::to_owned).collect();
                if segments.last().is_some_and(|s| s == "*") {
                    segments.pop();
                }
                self.rust_path(file, &segments).map(|(target, _)| target)
            }
            Ecosystem::Npm => self.npm_specifier(file, path),
        }
    }

    /// A path cited in the code: through the names the file imports, then as a path, then as a
    /// name defined once in the package (ADR 0013).
    fn resolve_reference(
        &self,
        file: &FileEntry,
        segments: &[String],
        receiver: bool,
    ) -> Option<(Target, f32)> {
        let first = segments.first()?;
        let ecosystem = self.ecosystem_of(file)?;
        // `x.f()` in Rust: the path is the method alone, its receiver unknown.
        if receiver && ecosystem == Ecosystem::Cargo {
            return None;
        }
        let bound = file
            .facts
            .imports
            .iter()
            .find(|i| i.names.iter().any(|n| n == first));
        if let Some(import) = bound {
            return match ecosystem {
                Ecosystem::Cargo => {
                    let mut expanded: Vec<String> =
                        import.path.split("::").map(str::to_owned).collect();
                    expanded.extend(segments[1..].iter().cloned());
                    self.rust_path(file, &expanded)
                }
                Ecosystem::Npm => {
                    let target = self.npm_specifier(file, &import.path)?;
                    Some((self.npm_reexported(target, first, 0), BY_PATH))
                }
            };
        }
        if segments.len() > 1
            && ecosystem == Ecosystem::Cargo
            && let Some(found) = self.rust_path(file, segments)
        {
            return Some(found);
        }
        // A name alone. A member of an unknown receiver (`this.n()`, `x.push()`) stays
        // unresolved: by its name alone, `.iter()` would lead to any `iter` of the package.
        let name = match segments {
            [name] if !receiver => name,
            _ => return None,
        };
        if file.facts.symbols.iter().any(|s| s.name() == name) {
            return Some((Target::File(file.path.clone()), BY_PATH));
        }
        if ecosystem == Ecosystem::Cargo
            && let Some(found) = self.through_globs(file, name)
        {
            return Some(found);
        }
        let defining = self.names.get(&(file.package, name.clone()))?;
        // A Rust name stays inside its crate: the library does not see an integration test,
        // nor a binary's modules.
        let same_target = |path: &String| {
            ecosystem != Ecosystem::Cargo
                || self
                    .files
                    .get(path)
                    .is_some_and(|other| other.target() == file.target())
        };
        let mut others = defining
            .iter()
            .filter(|p| **p != file.path && same_target(p));
        let only = others.next()?;
        if others.next().is_some() {
            return None;
        }
        Some((Target::File(only.clone()), BY_NAME))
    }

    /// `use a::b::*` brings the names of `a::b` into scope.
    fn through_globs(&self, file: &FileEntry, name: &str) -> Option<(Target, f32)> {
        for import in file
            .facts
            .imports
            .iter()
            .filter(|i| i.path.ends_with("::*"))
        {
            let segments: Vec<String> = import
                .path
                .trim_end_matches("::*")
                .split("::")
                .map(str::to_owned)
                .collect();
            if let Some((Target::File(path), _)) = self.rust_path(file, &segments)
                && self.files[&path]
                    .facts
                    .symbols
                    .iter()
                    .any(|s| s.name() == name)
            {
                return Some((Target::File(path), BY_PATH));
            }
        }
        None
    }

    /// A Rust path: `crate`, `self`, `super`, a workspace crate, a dependency, or a path
    /// relative to the current module or to the crate root.
    fn rust_path(&self, file: &FileEntry, segments: &[String]) -> Option<(Target, f32)> {
        self.rust_path_at(file, segments, 0)
    }

    fn rust_path_at(
        &self,
        file: &FileEntry,
        segments: &[String],
        depth: usize,
    ) -> Option<(Target, f32)> {
        let first = segments.first()?.as_str();
        let module = &file.module.segments;
        let root = &module[..file.module.root_len.min(module.len())];
        let candidates: Vec<(Vec<String>, &[String])> = match first {
            "std" | "core" | "alloc" | "Self" => return None,
            "crate" => vec![(root.to_vec(), &segments[1..])],
            "self" => vec![(module.clone(), &segments[1..])],
            "super" => {
                let ups = segments.iter().take_while(|s| *s == "super").count();
                let base = module[..module.len().saturating_sub(ups)].to_vec();
                vec![(base, &segments[ups..])]
            }
            name => {
                if let Some(p) = self
                    .packages
                    .iter()
                    .position(|p| p.ecosystem == Ecosystem::Cargo && p.code_name == name)
                {
                    return Some(
                        self.module_of(file, &[], segments, depth)
                            .map_or((Target::Package(p), BY_PATH), |path| {
                                (Target::File(path), BY_PATH)
                            }),
                    );
                }
                if let Some(dependency) = file.package.and_then(|p| {
                    self.packages[p]
                        .dependencies
                        .iter()
                        .find(|d| d.code_name == name)
                }) {
                    return Some((Target::External(dependency.name.clone()), BY_PATH));
                }
                vec![(module.clone(), segments), (root.to_vec(), segments)]
            }
        };
        // A match deeper than the base wins; a match on the base alone ("this module") is kept
        // in case no candidate goes deeper.
        let mut shallow = None;
        for (base, rest) in &candidates {
            let mut full = base.clone();
            full.extend(rest.iter().cloned());
            for n in (base.len().max(1)..=full.len()).rev() {
                if let Some(path) = self
                    .modules
                    .get(&(file.language.clone(), full[..n].to_vec()))
                {
                    if n > base.len() {
                        let path = self
                            .reexported(path, &full[n..], depth)
                            .unwrap_or_else(|| path.clone());
                        return Some((Target::File(path), BY_PATH));
                    }
                    shallow.get_or_insert_with(|| {
                        self.reexported(path, &full[n..], depth)
                            .unwrap_or_else(|| path.clone())
                    });
                    break;
                }
            }
        }
        shallow.map(|path| (Target::File(path), BY_PATH))
    }

    /// The file of the longest module path `base + prefix of rest`, at least `base` long, with
    /// re-exports followed.
    fn module_of(
        &self,
        file: &FileEntry,
        base: &[String],
        rest: &[String],
        depth: usize,
    ) -> Option<String> {
        let mut full = base.to_vec();
        full.extend(rest.iter().cloned());
        let min = base.len().max(1);
        (min..=full.len()).rev().find_map(|n| {
            let path = self
                .modules
                .get(&(file.language.clone(), full[..n].to_vec()))?;
            Some(
                self.reexported(path, &full[n..], depth)
                    .unwrap_or_else(|| path.clone()),
            )
        })
    }

    /// When a path goes on past the module of `path` (`timonier_core::Node`, the module being
    /// `lib.rs`), and that module imports the next name (`pub use node::Node`), the file that
    /// import leads to.
    fn reexported(&self, path: &str, rest: &[String], depth: usize) -> Option<String> {
        const MAX_DEPTH: usize = 4;
        let next = rest.first()?;
        if depth >= MAX_DEPTH {
            return None;
        }
        let module = self.files.get(path)?;
        let import = module
            .facts
            .imports
            .iter()
            .find(|i| i.names.contains(next))?;
        let mut segments: Vec<String> = import.path.split("::").map(str::to_owned).collect();
        segments.extend(rest[1..].iter().cloned());
        match self.rust_path_at(module, &segments, depth + 1)? {
            (Target::File(found), _) if found != path => Some(found),
            _ => None,
        }
    }

    /// A TypeScript module specifier: a relative file, a package of the repository, or a
    /// dependency.
    fn npm_specifier(&self, file: &FileEntry, specifier: &str) -> Option<Target> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let dir = file.path.rsplit_once('/').map_or("", |(d, _)| d);
            let joined = normalise(dir, specifier)?;
            return self.ts_file(&joined).map(Target::File);
        }
        let package_name = if specifier.starts_with('@') {
            specifier
                .splitn(3, '/')
                .take(2)
                .collect::<Vec<_>>()
                .join("/")
        } else {
            specifier.split('/').next()?.to_owned()
        };
        if let Some(p) = self
            .packages
            .iter()
            .position(|p| p.ecosystem == Ecosystem::Npm && p.name == package_name)
        {
            return Some(Target::Package(p));
        }
        let package = file.package?;
        self.packages[package]
            .dependencies
            .iter()
            .find(|d| d.name == package_name)
            .map(|d| Target::External(d.name.clone()))
    }

    /// A name imported from a file that only re-exports it (`export { x } from "./x"`): the
    /// file it comes from.
    fn npm_reexported(&self, target: Target, name: &str, depth: usize) -> Target {
        const MAX_DEPTH: usize = 4;
        let Target::File(path) = &target else {
            return target;
        };
        if depth >= MAX_DEPTH {
            return target;
        }
        let Some(module) = self.files.get(path) else {
            return target;
        };
        let Some(import) = module
            .facts
            .imports
            .iter()
            .find(|i| i.names.iter().any(|n| n == name))
        else {
            return target;
        };
        match self.npm_specifier(module, &import.path) {
            Some(Target::File(next)) if next != *path => {
                self.npm_reexported(Target::File(next), name, depth + 1)
            }
            _ => target,
        }
    }

    /// The file an npm module specifier names: TypeScript first, then JavaScript (T-120), as
    /// TypeScript's resolution does with `allowJs`.
    fn ts_file(&self, stem: &str) -> Option<String> {
        let mut candidates = vec![stem.to_owned()];
        for extension in ["ts", "tsx", "mts", "cts", "d.ts", "js", "jsx", "mjs", "cjs"] {
            candidates.push(format!("{stem}.{extension}"));
        }
        for index in ["index.ts", "index.tsx", "index.js", "index.jsx"] {
            candidates.push(format!("{stem}/{index}"));
        }
        // ESM imports name the emitted `.js`: `./host.js` is `./host.ts`.
        for (js, ts) in [
            (".js", ".ts"),
            (".jsx", ".tsx"),
            (".mjs", ".mts"),
            (".cjs", ".cts"),
            (".js", ".tsx"),
        ] {
            if let Some(base) = stem.strip_suffix(js) {
                candidates.push(format!("{base}{ts}"));
            }
        }
        candidates.into_iter().find(|c| self.files.contains_key(c))
    }
}

/// `dir` joined with a relative specifier, `..` resolved; `None` if it leaves the repository.
fn normalise(dir: &str, specifier: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn defined_names(entry: &FileEntry) -> BTreeSet<String> {
    defined_names_of(&entry.facts)
}

/// Names an import binds to another path than before (`pub use a::X` → `pub use b::X`).
fn rebound_names(old: &FileFacts, new: &FileFacts) -> BTreeSet<String> {
    let bindings = |facts: &FileFacts| -> BTreeSet<(String, String)> {
        facts
            .imports
            .iter()
            .flat_map(|i| i.names.iter().map(|n| (n.clone(), i.path.clone())))
            .collect()
    };
    bindings(old)
        .symmetric_difference(&bindings(new))
        .map(|(name, _)| name.clone())
        .collect()
}

/// Names whose symbols changed kind or signature between two extractions of a file.
fn signature_changes(old: &FileFacts, new: &FileFacts) -> BTreeSet<String> {
    let signatures = |facts: &FileFacts| -> BTreeSet<(String, String, String)> {
        facts
            .symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Field)
            .map(|s| {
                (
                    s.name().to_owned(),
                    format!("{:?}", s.kind),
                    s.signature.clone(),
                )
            })
            .collect()
    };
    signatures(old)
        .symmetric_difference(&signatures(new))
        .map(|(name, ..)| name.clone())
        .collect()
}

/// The names a file makes available: its definitions, and what its imports bind, since a
/// re-export (`pub use node::Node`, `export { x } from "./x"`) is followed by resolution.
fn defined_names_of(facts: &FileFacts) -> BTreeSet<String> {
    facts
        .symbols
        .iter()
        .map(|s| s.name().to_owned())
        .chain(facts.imports.iter().flat_map(|i| i.names.iter().cloned()))
        .collect()
}

/// Every segment a file cites: in its import paths and in its references.
fn cited_names(facts: &FileFacts, separators: &[String]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for import in &facts.imports {
        let mut parts = vec![import.path.clone()];
        for separator in separators.iter().map(String::as_str).chain(["/", "::"]) {
            parts = parts
                .iter()
                .flat_map(|p| p.split(separator).map(str::to_owned).collect::<Vec<_>>())
                .collect();
        }
        names.extend(
            parts
                .into_iter()
                .filter(|p| !p.is_empty() && p != "." && p != ".."),
        );
        names.extend(import.names.iter().cloned());
    }
    for reference in &facts.references {
        names.extend(reference.path.iter().cloned());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(text: &str) -> ContentHash {
        ContentHash::from_bytes(*blake3::hash(text.as_bytes()).as_bytes())
    }

    fn cargo(name: &str, dir: &str, deps: &[(&str, bool)]) -> PackageInfo {
        PackageInfo {
            id: NodeId::package("cargo", name).unwrap(),
            name: name.into(),
            ecosystem: Ecosystem::Cargo,
            dir: dir.into(),
            code_name: name.replace('-', "_"),
            lib_path: None,
            has_lib: true,
            bins: Vec::new(),
            dependencies: deps
                .iter()
                .map(|(d, runtime)| ecosystem::Dependency {
                    name: (*d).into(),
                    code_name: d.replace('-', "_"),
                    runtime: *runtime,
                })
                .collect(),
        }
    }

    fn npm(name: &str, dir: &str, deps: &[&str]) -> PackageInfo {
        PackageInfo {
            id: NodeId::package("npm", name).unwrap(),
            name: name.into(),
            ecosystem: Ecosystem::Npm,
            dir: dir.into(),
            code_name: name.into(),
            lib_path: None,
            has_lib: false,
            bins: Vec::new(),
            dependencies: deps
                .iter()
                .map(|d| ecosystem::Dependency {
                    name: (*d).into(),
                    code_name: (*d).into(),
                    runtime: true,
                })
                .collect(),
        }
    }

    fn project(packages: Vec<PackageInfo>, files: &[(&str, &str)]) -> Project {
        let packs = Packs::builtin().unwrap();
        let mut project = Project::new("demo", &packs, packages).unwrap();
        for (path, text) in files {
            let facts = packs.extract(path, text.as_bytes()).unwrap();
            let language = packs.language_of(path).unwrap().clone();
            project.upsert(path, language, hash(text), facts).unwrap();
        }
        project
    }

    fn edges(project: &Project, path: &str) -> Vec<String> {
        project
            .file_update(path)
            .unwrap()
            .observations
            .unwrap()
            .observations
            .iter()
            .map(|o| {
                format!(
                    "{} {} -> {} w{} c{}",
                    o.kind,
                    o.src,
                    o.dst,
                    o.weight,
                    o.confidence.get()
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    const CORE_LIB: &str = "pub mod node;\npub mod edge;\npub use node::Node;\n";
    const CORE_NODE: &str =
        "pub struct Node;\nimpl Node { pub fn new() -> Self { Node } }\npub trait Named {}\n";
    const CORE_EDGE: &str = "use crate::{node::Named, Node};\nuse serde::Serialize;\npub struct Edge { pub src: Node }\nimpl Named for Edge {}\npub fn link() -> Edge { Edge { src: Node::new() } }\n";
    const STORE_LIB: &str = "use my_core::Node;\nuse my_core::edge::*;\npub fn save(n: &Node) { let e = link(); unique_helper(); }\nmod util;\n";
    const STORE_UTIL: &str = "pub fn unique_helper() {}\n";

    fn rust_project() -> Project {
        project(
            vec![
                cargo(
                    "my-core",
                    "crates/core",
                    &[("serde", true), ("proptest", false)],
                ),
                cargo("my-store", "crates/store", &[("my-core", true)]),
            ],
            &[
                ("crates/core/src/lib.rs", CORE_LIB),
                ("crates/core/src/node.rs", CORE_NODE),
                ("crates/core/src/edge.rs", CORE_EDGE),
                ("crates/store/src/lib.rs", STORE_LIB),
                ("crates/store/src/util.rs", STORE_UTIL),
            ],
        )
    }

    #[test]
    fn rust_paths_resolve_to_files_and_packages() {
        let project = rust_project();
        assert_eq!(
            edges(&project, "crates/core/src/edge.rs"),
            [
                "calls file:crates/core/src/edge.rs -> file:crates/core/src/node.rs w1 c0.8",
                "implements rust:my_core::edge::Edge -> rust:my_core::node::Named w1 c0.8",
                "imports file:crates/core/src/edge.rs -> file:crates/core/src/node.rs w2 c0.9",
                "imports file:crates/core/src/edge.rs -> pkg:cargo/serde w1 c0.9",
                "references file:crates/core/src/edge.rs -> file:crates/core/src/node.rs w1 c0.8",
            ]
        );
        assert_eq!(
            edges(&project, "crates/store/src/lib.rs"),
            [
                "calls file:crates/store/src/lib.rs -> file:crates/core/src/edge.rs w1 c0.8",
                "calls file:crates/store/src/lib.rs -> file:crates/store/src/util.rs w1 c0.5",
                "imports file:crates/store/src/lib.rs -> file:crates/core/src/edge.rs w1 c0.9",
                // `my_core::Node` is re-exported by the crate root: the import goes to its module.
                "imports file:crates/store/src/lib.rs -> file:crates/core/src/node.rs w1 c0.9",
                "references file:crates/store/src/lib.rs -> file:crates/core/src/node.rs w1 c0.8",
            ]
        );
    }

    #[test]
    fn symbols_get_qualified_ids_and_parents() {
        let project = rust_project();
        let update = project.file_update("crates/core/src/node.rs").unwrap();
        let nodes: Vec<String> = update
            .nodes
            .unwrap()
            .iter()
            .map(|n| format!("{} {} under {}", n.kind, n.id, n.parent.as_ref().unwrap()))
            .collect();
        assert_eq!(
            nodes,
            [
                "type rust:my_core::node::Node under file:crates/core/src/node.rs",
                "method rust:my_core::node::Node::new under rust:my_core::node::Node",
                "trait rust:my_core::node::Named under file:crates/core/src/node.rs",
            ]
        );
    }

    #[test]
    fn the_structure_lists_kept_packages_runtime_dependencies_and_files() {
        let project = rust_project();
        let structure = project.structure();
        let nodes: Vec<String> = structure
            .nodes
            .unwrap()
            .iter()
            .map(|n| format!("{} {}", n.kind, n.id))
            .collect();
        assert_eq!(
            nodes,
            [
                "repo repo:demo",
                "package pkg:cargo/my-core",
                "package pkg:cargo/my-store",
                // External dependencies, grouped by usage (ADR 0016, 0017).
                "component component:external",
                "component component:external/serialization",
                "package pkg:cargo/serde",
                "file file:crates/core/src/edge.rs",
                "file file:crates/core/src/lib.rs",
                "file file:crates/core/src/node.rs",
                "file file:crates/store/src/lib.rs",
                "file file:crates/store/src/util.rs",
            ]
        );
        let deps: Vec<String> = structure
            .observations
            .unwrap()
            .observations
            .iter()
            .map(|o| format!("{} -> {}", o.src, o.dst))
            .collect();
        assert_eq!(
            deps,
            [
                "pkg:cargo/my-core -> pkg:cargo/serde",
                "pkg:cargo/my-store -> pkg:cargo/my-core"
            ]
        );
    }

    /// ADR 0022: what refinement asks the language server about a file.
    #[test]
    fn usages_are_asked_once_per_path_and_never_for_what_cannot_lead_elsewhere() {
        let kind = "pub enum Kind { A, B }\npub struct Here;\n";
        let user = "use crate::kind::Kind;\nuse serde::Serialize;\n\
            pub struct Mine;\n\
            pub fn f(a: Kind, b: Kind, c: Mine, d: Vec<u8>, e: Serialize) -> Kind { Kind::A }\n\
            pub fn g<T>(t: T, u: Option<_>) -> Kind { Kind::B }\n";
        let project = project(
            vec![cargo("app", "", &[("serde", true)])],
            &[
                ("src/lib.rs", "pub mod kind;\npub mod user;\n"),
                ("src/kind.rs", kind),
                ("src/user.rs", user),
            ],
        );
        let file = project.file("src/user.rs").unwrap();
        let usages = project.usages(file);
        // One question per path, at its first site: `Kind` (four sites), `Kind::A`, `Kind::B`.
        // `Mine` is in the file, `Serialize` in a dependency, `Vec` and `Option` in the standard
        // library, `T` and `_` in the item itself: none is asked.
        let mut asked: Vec<_> = usages.iter().map(|u| (u.kind, u.at, u.sites)).collect();
        asked.sort();
        assert_eq!(
            asked,
            [
                (EdgeKind::References, Position::new(3, 12), 4),
                // At the variant's name: `A` of `Kind::A`, `B` of `Kind::B`.
                (EdgeKind::References, Position::new(3, 78), 1),
                (EdgeKind::References, Position::new(4, 48), 1),
            ],
            "{usages:?}"
        );
    }

    /// ADR 0022, review n° 4, m-A: a name bound by an import is placed elsewhere by tree-sitter
    /// for sure, and a server's « here » is doubtful; a name found by its name alone is a guess,
    /// which the server's « here » may correct.
    #[test]
    fn a_usage_is_elsewhere_only_when_an_import_or_a_path_binds_it() {
        let project = project(
            vec![cargo("app", "", &[])],
            &[
                (
                    "src/lib.rs",
                    "pub mod kind;\npub mod item;\npub mod user;\n",
                ),
                ("src/kind.rs", "pub struct Kind;\n"),
                ("src/item.rs", "pub struct Item;\n"),
                (
                    "src/user.rs",
                    "use crate::kind::Kind;\npub fn f(k: Kind, i: Item) {}\n",
                ),
            ],
        );
        let file = project.file("src/user.rs").unwrap();
        let mut found: Vec<(Position, bool)> = project
            .usages(file)
            .iter()
            .map(|u| (u.at, u.elsewhere))
            .collect();
        found.sort();
        assert_eq!(
            found,
            [(Position::new(1, 12), true), (Position::new(1, 21), false)]
        );
    }

    #[test]
    fn changes_reresolve_only_the_files_that_cite_changed_names() {
        let packs = Packs::builtin().unwrap();
        let mut project = rust_project();
        let rust = Language::new("rust").unwrap();
        // A body change defines the same names: only the file itself.
        let text = STORE_UTIL.replace("{}", "{ let _ = 1; }");
        let facts = packs
            .extract("crates/store/src/util.rs", text.as_bytes())
            .unwrap();
        let affected = project
            .upsert("crates/store/src/util.rs", rust.clone(), hash(&text), facts)
            .unwrap();
        assert_eq!(
            affected,
            Affected {
                structure: false,
                files: ["crates/store/src/util.rs".to_owned()].into(),
                dependents: BTreeSet::new(),
                lookups: Vec::new(),
            }
        );
        // A changed signature asks the language server for its users (ADR 0022). Should it not
        // answer, they are the citers of that name, not the file itself.
        let text = "pub fn unique_helper(x: u32) {}\n";
        let facts = packs
            .extract("crates/store/src/util.rs", text.as_bytes())
            .unwrap();
        let affected = project
            .upsert("crates/store/src/util.rs", rust.clone(), hash(text), facts)
            .unwrap();
        assert_eq!(
            affected.files,
            ["crates/store/src/util.rs".to_owned()].into()
        );
        assert_eq!(affected.dependents, BTreeSet::new());
        // The symbol, by its id: found again at its position in the current content.
        let symbol = affected.lookups[0].symbol.clone();
        assert!(
            symbol.as_str().ends_with("::util::unique_helper"),
            "{symbol}"
        );
        assert_eq!(
            project.name_at("crates/store/src/util.rs", &symbol),
            Some(Position::new(0, 7))
        );
        assert_eq!(
            affected.lookups,
            [Lookup {
                path: "crates/store/src/util.rs".into(),
                symbol,
                at: Position::new(0, 7),
                content_hash: hash(text),
                by_name: ["crates/store/src/lib.rs".to_owned()].into(),
            }]
        );
        // Renaming a definition re-resolves the files citing either name. The name gone is
        // looked up by name: no position is left to ask about.
        let text = "pub fn other_helper() {}\n";
        let facts = packs
            .extract("crates/store/src/util.rs", text.as_bytes())
            .unwrap();
        let affected = project
            .upsert("crates/store/src/util.rs", rust.clone(), hash(text), facts)
            .unwrap();
        assert_eq!(
            affected.files,
            [
                "crates/store/src/lib.rs".to_owned(),
                "crates/store/src/util.rs".to_owned()
            ]
            .into()
        );
        assert!(
            !edges(&project, "crates/store/src/lib.rs")
                .iter()
                .any(|e| e.contains("util.rs")),
            "the call no longer resolves"
        );
        assert_eq!(
            affected.dependents,
            ["crates/store/src/lib.rs".to_owned()].into()
        );
        assert_eq!(
            affected.lookups,
            [],
            "the new name, cited nowhere yet: no file can use it"
        );
        // Removing a file updates the structure and its citers.
        let affected = project.remove("crates/core/src/node.rs");
        assert!(affected.structure);
        assert!(affected.files.contains("crates/core/src/edge.rs"));
        assert!(!affected.files.contains("crates/core/src/node.rs"));
    }

    #[test]
    fn typescript_specifiers_resolve_relative_files_packages_and_dependencies() {
        let project = project(
            vec![
                npm("@me/web", "web", &["react"]),
                npm("ext", "editors/vscode", &["@me/web"]),
            ],
            &[
                (
                    "web/src/main.tsx",
                    "import { App } from \"./App\";\nimport { createRoot } from \"react-dom/client\";\nimport * as React from \"react\";\nimport { send } from \"./host.js\";\ncreateRoot(x).render(<App />);\nsend();\n",
                ),
                (
                    "web/src/App.tsx",
                    "export function App() { return null; }\n",
                ),
                ("web/src/host.ts", "export function send() {}\n"),
                ("web/src/util/index.ts", "export const k = 1;\n"),
                (
                    "editors/vscode/src/extension.ts",
                    "import * as vscode from \"vscode\";\nimport { k } from \"../../../web/src/util\";\nimport { App } from \"@me/web\";\nexport function activate() { vscode.window; }\n",
                ),
            ],
        );
        assert_eq!(
            edges(&project, "web/src/main.tsx"),
            [
                "calls file:web/src/main.tsx -> file:web/src/App.tsx w1 c0.8",
                "calls file:web/src/main.tsx -> file:web/src/host.ts w1 c0.8",
                "imports file:web/src/main.tsx -> file:web/src/App.tsx w1 c0.9",
                "imports file:web/src/main.tsx -> file:web/src/host.ts w1 c0.9",
                "imports file:web/src/main.tsx -> pkg:npm/react w1 c0.9",
            ]
        );
        assert_eq!(
            edges(&project, "editors/vscode/src/extension.ts"),
            [
                "imports file:editors/vscode/src/extension.ts -> file:web/src/util/index.ts w1 c0.9",
                "imports file:editors/vscode/src/extension.ts -> pkg:npm/@me/web w1 c0.9",
            ]
        );
        let ids: Vec<String> = project
            .file_update("web/src/App.tsx")
            .unwrap()
            .nodes
            .unwrap()
            .iter()
            .map(|n| n.id.to_string())
            .collect();
        assert_eq!(ids, ["typescript:@me/web/src/App#App"]);
    }

    #[test]
    fn re_exports_are_followed_and_their_changes_resolve_citers_again() {
        let packs = Packs::builtin().unwrap();
        let mut project = project(
            vec![npm("@me/web", "web", &[])],
            &[
                ("web/src/index.ts", "export { send } from \"./host\";\n"),
                ("web/src/host.ts", "export function send() {}\n"),
                (
                    "web/src/main.ts",
                    "import { send } from \"./index\";\nsend();\n",
                ),
            ],
        );
        assert!(
            edges(&project, "web/src/main.ts")
                .contains(&"calls file:web/src/main.ts -> file:web/src/host.ts w1 c0.8".to_owned()),
            "{:?}",
            edges(&project, "web/src/main.ts")
        );
        // The re-export goes away: the citer is resolved again.
        let text = "export const other = 1;\n";
        let facts = packs.extract("web/src/index.ts", text.as_bytes()).unwrap();
        let typescript = Language::new("typescript").unwrap();
        let affected = project
            .upsert("web/src/index.ts", typescript, hash(text), facts)
            .unwrap();
        assert!(affected.files.contains("web/src/main.ts"), "{affected:?}");
    }

    #[test]
    fn a_re_export_that_changes_target_resolves_its_citers_again() {
        let packs = Packs::builtin().unwrap();
        let mut project = project(
            vec![npm("@me/web", "web", &[])],
            &[
                ("web/src/index.ts", "export { send } from \"./a\";\n"),
                ("web/src/a.ts", "export function send() {}\n"),
                ("web/src/b.ts", "export function send() {}\n"),
                (
                    "web/src/main.ts",
                    "import { send } from \"./index\";\nsend();\n",
                ),
            ],
        );
        let text = "export { send } from \"./b\";\n";
        let facts = packs.extract("web/src/index.ts", text.as_bytes()).unwrap();
        let typescript = Language::new("typescript").unwrap();
        let affected = project
            .upsert("web/src/index.ts", typescript, hash(text), facts)
            .unwrap();
        assert!(affected.files.contains("web/src/main.ts"), "{affected:?}");
        assert!(
            edges(&project, "web/src/main.ts")
                .contains(&"calls file:web/src/main.ts -> file:web/src/b.ts w1 c0.8".to_owned()),
            "{:?}",
            edges(&project, "web/src/main.ts")
        );
        // Rust: `pub use crate::a::Thing` becomes `pub use crate::b::Thing`.
        let mut project = project_rust_reexport();
        let text = "pub use crate::b::Thing;\n";
        let facts = packs
            .extract("crates/k/src/prelude.rs", text.as_bytes())
            .unwrap();
        let affected = project
            .upsert(
                "crates/k/src/prelude.rs",
                Language::new("rust").unwrap(),
                hash(text),
                facts,
            )
            .unwrap();
        assert!(
            affected.files.contains("crates/k/src/user.rs"),
            "{affected:?}"
        );
        assert!(
            edges(&project, "crates/k/src/user.rs")
                .iter()
                .any(|e| e.starts_with("references")
                    && e.ends_with("file:crates/k/src/b.rs w1 c0.8")),
            "{:?}",
            edges(&project, "crates/k/src/user.rs")
        );
    }

    fn project_rust_reexport() -> Project {
        project(
            vec![cargo("k", "crates/k", &[])],
            &[
                (
                    "crates/k/src/lib.rs",
                    "pub mod a;\npub mod b;\npub mod prelude;\npub mod user;\n",
                ),
                ("crates/k/src/a.rs", "pub struct Thing;\n"),
                ("crates/k/src/b.rs", "pub struct Thing;\n"),
                ("crates/k/src/prelude.rs", "pub use crate::a::Thing;\n"),
                (
                    "crates/k/src/user.rs",
                    "use crate::prelude::Thing;\npub fn f(_: Thing) {}\n",
                ),
            ],
        )
    }

    /// Second review, m-d: the library cannot call an integration test by name.
    #[test]
    fn names_stay_inside_their_rust_target() {
        let project = project(
            vec![cargo("k", "crates/k", &[])],
            &[
                (
                    "crates/k/src/lib.rs",
                    "pub fn f() { let render = |x: u32| x; render(1); }\n",
                ),
                (
                    "crates/k/tests/extract.rs",
                    "fn render() {}\n#[test]\nfn t() { render(); }\n",
                ),
            ],
        );
        assert!(
            edges(&project, "crates/k/src/lib.rs").is_empty(),
            "{:?}",
            edges(&project, "crates/k/src/lib.rs")
        );
    }

    #[test]
    fn files_outside_packages_hang_under_the_repo() {
        let project = project(Vec::new(), &[("scripts/gen.rs", "fn main() {}\n")]);
        let structure = project.structure();
        let file = structure
            .nodes
            .unwrap()
            .into_iter()
            .find(|n| n.kind == NodeKind::File)
            .unwrap();
        assert_eq!(file.parent, Some(NodeId::repo("demo").unwrap()));
        let ids: Vec<String> = project
            .file_update("scripts/gen.rs")
            .unwrap()
            .nodes
            .unwrap()
            .iter()
            .map(|n| n.id.to_string())
            .collect();
        assert_eq!(ids, ["rust:scripts::gen::main"]);
    }

    #[test]
    fn specifiers_normalise_and_stay_inside_the_repository() {
        assert_eq!(
            normalise("web/src", "./a/../b").as_deref(),
            Some("web/src/b")
        );
        assert_eq!(normalise("web", "../x").as_deref(), Some("x"));
        assert_eq!(normalise("", "../x"), None);
    }
}
