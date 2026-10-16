//! The indexer (ADR 0013): walks the repository, extracts files in parallel with the cache
//! first, keeps the project model, and turns changes on disk into scope updates.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rayon::prelude::*;
use timonier_core::{ContentHash, Language, NodeId, ScopeKey, ScopeUpdate};
use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::components::Components;
use crate::ecosystem::{self, MANIFESTS, PackageInfo};
use crate::extract::{self, EXTRACTOR_VERSION};
use crate::project::Project;
use crate::{Error, FileFacts, Packs, Result};

/// Larger files are generated or minified code: they are not indexed (ADR 0013).
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

/// Syntax trees kept for incremental reparsing (ADR 0013).
const KEPT_TREES: usize = 64;

/// What an extraction was computed from: it is valid only for the same key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheKey {
    pub language: Language,
    pub pack_version: String,
    pub extractor: u32,
    pub content_hash: ContentHash,
}

/// A cached extraction, as the store gives it back.
#[derive(Clone, Debug, PartialEq)]
pub struct Cached {
    pub key: CacheKey,
    pub facts: FileFacts,
}

/// What one indexing pass produced.
#[derive(Debug, Default)]
pub struct Batch {
    /// For `Store::apply_all`, in one transaction.
    pub updates: Vec<ScopeUpdate>,
    /// New extractions to cache.
    pub cache_put: Vec<(String, Cached)>,
    /// Paths to forget from the cache.
    pub cache_remove: Vec<String>,
    /// Files whose content changed or appeared.
    pub changed: BTreeSet<String>,
    /// Files that disappeared.
    pub removed: BTreeSet<String>,
    /// Other files citing a symbol whose signature changed (ADR 0014).
    pub dependents: BTreeSet<String>,
    /// The packages changed: ids may have changed too (ADR 0013).
    pub packages_changed: bool,
    /// What could not be indexed, and why: a broken manifest, an unusable path.
    pub problems: Vec<String>,
    pub stats: BatchStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchStats {
    /// Files of the repository a pack reads.
    pub files: usize,
    /// Files parsed by tree-sitter.
    pub parsed: usize,
    /// Of those, parsed from their previous tree.
    pub incremental: usize,
    /// Files whose extraction came from the cache.
    pub cached: usize,
    /// Files read again with the same content.
    pub unchanged: usize,
    pub removed: usize,
    /// Files whose edges were resolved again, without reparse.
    pub resolved: usize,
    /// Files too large, unreadable, or outside the repository.
    pub skipped: usize,
}

pub struct Indexer {
    root: PathBuf,
    /// The root as given, when it differs from the canonical one (a symbolic link, such as
    /// macOS's `/var` → `/private/var`): callers may name paths through it.
    given_root: Option<PathBuf>,
    repo_name: String,
    packs: Packs,
    project: Project,
    trees: Trees,
    parser: Parser,
    /// Directories the last walk entered (`""` for the root): a new path elsewhere is ignored
    /// or hidden, and costs no walk.
    walked: BTreeSet<String>,
    /// Manifests the packages were discovered from. Any walk that finds others changes the
    /// packages: a new directory that brings one, a crate created next to a new file.
    manifests: BTreeSet<String>,
}

impl Indexer {
    pub fn new(root: impl Into<PathBuf>, packs: Packs) -> Result<Self> {
        let root = root.into();
        let canonical = root.canonicalize().map_err(|source| Error::Io {
            path: root.clone(),
            source,
        })?;
        let repo_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository")
            .to_owned();
        let project = Project::new(&repo_name, &packs, Vec::new())?;
        let given_root = std::path::absolute(&root)
            .ok()
            .filter(|given| *given != canonical);
        Ok(Self {
            root: canonical,
            given_root,
            repo_name,
            packs,
            project,
            trees: Trees::default(),
            parser: Parser::new(),
            walked: BTreeSet::new(),
            manifests: BTreeSet::new(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn packs(&self) -> &Packs {
        &self.packs
    }

    /// Indexes the whole repository. `cache` holds the stored extractions by path; `stored`
    /// lists the scopes the store holds, so that files deleted meanwhile are removed;
    /// `components` are those of the head, `None` if nothing was ever indexed: a restart keeps
    /// them, since clustering only happens at a commit (ADR 0016).
    pub fn index_all(
        &mut self,
        cache: &HashMap<String, Cached>,
        stored: &[ScopeKey],
        components: Option<&Components>,
    ) -> Result<Batch> {
        let mut batch = Batch::default();
        let walk = self.walk();
        batch.problems.extend(
            walk.rejected
                .iter()
                .map(|p| format!("`{p}`: not a valid file path for an id")),
        );
        let (packages, problems) = ecosystem::discover(&self.root, &walk.manifests);
        self.manifests = walk.manifests.iter().cloned().collect();
        batch
            .problems
            .extend(problems.iter().map(ToString::to_string));
        self.project = Project::new(&self.repo_name, &self.packs, packages)?;
        self.trees = Trees::default();
        let packs = &self.packs;
        let root = &self.root;
        let read: Vec<Result<Option<Read>>> = walk
            .sources
            .par_iter()
            .map_init(Parser::new, |parser, path| {
                let Source::Bytes(bytes) = read_source(root, path) else {
                    return Ok(None);
                };
                let (pack, grammar) = packs
                    .for_path(path)
                    .ok_or_else(|| Error::NoPack { path: path.clone() })?;
                let key = cache_key(pack, grammar, content_hash(&bytes));
                if let Some(cached) = cache.get(path).filter(|c| c.key == key) {
                    return Ok(Some(Read {
                        path: path.clone(),
                        key,
                        facts: cached.facts.clone(),
                        parsed: false,
                    }));
                }
                let (facts, _) = extract::extract(pack, grammar, parser, &bytes, None, path)?;
                Ok(Some(Read {
                    path: path.clone(),
                    key,
                    facts,
                    parsed: true,
                }))
            })
            .collect();
        let mut indexed = BTreeSet::new();
        for result in read {
            let Some(read) = result? else {
                batch.stats.skipped += 1;
                continue;
            };
            if read.parsed {
                batch.stats.parsed += 1;
                batch.cache_put.push((
                    read.path.clone(),
                    Cached {
                        key: read.key.clone(),
                        facts: read.facts.clone(),
                    },
                ));
            } else {
                batch.stats.cached += 1;
            }
            self.project.upsert(
                &read.path,
                read.key.language,
                read.key.content_hash,
                read.facts,
            )?;
            indexed.insert(read.path);
        }
        batch.stats.files = indexed.len();
        match components {
            // Never indexed: the first clustering.
            None => self.project.recluster(&BTreeMap::new()),
            // A package of the head is gone (renamed while away): its components go with it,
            // and the clustering runs again from what was there.
            Some(components) if !self.project.restore_components(components.clone()) => {
                self.project.recluster(&components.assignment);
            }
            Some(_) => {}
        }
        batch.updates = self.project.updates(true, &indexed, &BTreeSet::new());
        for scope in stored {
            if let Some(file) = scope.file()
                && let Some(path) = file_path(&file)
                && !indexed.contains(path)
            {
                batch.updates.push(ScopeUpdate::removal(scope.clone()));
                batch.stats.removed += 1;
            }
        }
        batch.cache_remove = cache
            .keys()
            .filter(|p| !indexed.contains(*p))
            .cloned()
            .collect();
        batch.cache_remove.sort();
        batch.changed = indexed;
        Ok(batch)
    }

    /// Files or directories changed on disk: created, modified, deleted or renamed. Paths are
    /// absolute or relative to the root.
    pub fn refresh(&mut self, paths: &[PathBuf]) -> Result<Batch> {
        let relative: BTreeSet<String> = paths.iter().filter_map(|p| self.relative(p)).collect();
        let mut batch = Batch::default();
        // A manifest, or a Cargo library that appears or disappears, may move packages
        // (ADR 0013). A library saved again changes nothing of the kind. A directory that
        // appears may hold a manifest: the walk tells.
        let lib_appears = relative.iter().any(|p| {
            is_cargo_lib(p) && self.root.join(p).is_file() != self.project.file(p).is_some()
        });
        let manifest_named = relative.iter().any(|p| is_manifest(p));
        let dir_named = relative.iter().any(|p| self.root.join(p).is_dir());
        // A new path in a walked directory: only the walk says whether it is indexed.
        let new_path = relative.iter().any(|p| {
            self.walked.contains(parent(p))
                && self.project.paths_under(p).next().is_none()
                && self.root.join(p).exists()
        });
        let mut accepted: Option<BTreeSet<String>> = None;
        if lib_appears || manifest_named || dir_named || new_path {
            let walk = self.walk();
            let found: BTreeSet<String> = walk.manifests.iter().cloned().collect();
            if lib_appears || manifest_named || found != self.manifests {
                let (mut packages, problems) = ecosystem::discover(&self.root, &walk.manifests);
                self.manifests = found;
                // A manifest that does not parse, mid-edit, keeps its previous package; the
                // others follow their manifests.
                let broken: BTreeSet<&Path> = problems.iter().filter_map(Error::path).collect();
                for previous in self.project.packages() {
                    let manifest = self.root.join(previous.manifest());
                    let replaced = packages
                        .iter()
                        .any(|p| p.dir == previous.dir && p.ecosystem == previous.ecosystem);
                    if broken.contains(manifest.as_path()) && !replaced {
                        packages.push(previous.clone());
                    }
                }
                packages.sort_by(|a, b| a.id.cmp(&b.id));
                batch
                    .problems
                    .extend(problems.iter().map(ToString::to_string));
                if packages.as_slice() != self.project.packages() {
                    return self.resync(batch, packages, walk.sources);
                }
            }
            accepted = Some(walk.sources.into_iter().collect());
        }
        let mut structure = false;
        let mut changed = BTreeSet::new();
        let mut resolve = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for path in &relative {
            let on_disk = self.root.join(path);
            let known: Vec<String> = self.project.paths_under(path).map(str::to_owned).collect();
            for file in &known {
                // Gone, or now too large to be source code (ADR 0013). The metadata say it, without
                // reading the file.
                let gone = match std::fs::metadata(self.root.join(file)) {
                    Ok(meta) => !meta.is_file() || meta.len() > MAX_FILE_BYTES as u64,
                    Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                };
                if gone {
                    let affected = self.project.remove(file);
                    structure |= affected.structure;
                    batch.dependents.extend(affected.dependents);
                    resolve.extend(affected.files);
                    removed.insert(file.clone());
                    self.trees.remove(file);
                }
            }
            if on_disk.is_file() && known.contains(path) && !removed.contains(path) {
                self.reindex(path, &mut batch, &mut changed, &mut resolve)?;
            } else if on_disk.exists() && known.is_empty() && self.walked.contains(parent(path)) {
                // New files, or a directory moved in: only what the walk accepts.
                let Some(accepted) = &accepted else {
                    continue;
                };
                let new: Vec<String> = accepted
                    .iter()
                    .filter(|p| {
                        (*p == path || p.starts_with(&format!("{path}/")))
                            && self.project.file(p).is_none()
                    })
                    .cloned()
                    .collect();
                for file in new {
                    let before = changed.len();
                    self.reindex(&file, &mut batch, &mut changed, &mut resolve)?;
                    structure |= changed.len() > before;
                }
            }
        }
        for file in &removed {
            if let Ok(scope) = NodeId::file(file).and_then(|id| ScopeKey::of_file(&id)) {
                batch.updates.push(ScopeUpdate::removal(scope));
            }
            batch.cache_remove.push(file.clone());
        }
        batch.stats.removed = removed.len();
        let resolve: BTreeSet<String> = resolve.difference(&removed).cloned().collect();
        batch.dependents = batch
            .dependents
            .difference(&removed)
            .filter(|p| !changed.contains(*p))
            .cloned()
            .collect();
        batch.stats.resolved = resolve.difference(&changed).count();
        let mut updates = self.project.updates(structure, &changed, &resolve);
        updates.append(&mut batch.updates);
        batch.updates = updates;
        batch.stats.files = self.project.files().count();
        batch.changed = changed;
        batch.removed = removed;
        Ok(batch)
    }

    /// Reads one known or new file again; parses it only if its content changed.
    fn reindex(
        &mut self,
        path: &str,
        batch: &mut Batch,
        changed: &mut BTreeSet<String>,
        resolve: &mut BTreeSet<String>,
    ) -> Result<()> {
        let Source::Bytes(bytes) = read_source(&self.root, path) else {
            batch.stats.skipped += 1;
            return Ok(());
        };
        let hash = content_hash(&bytes);
        if self
            .project
            .file(path)
            .is_some_and(|f| f.content_hash == hash)
        {
            batch.stats.unchanged += 1;
            return Ok(());
        }
        let (pack, grammar) = self
            .packs
            .for_path(path)
            .ok_or_else(|| Error::NoPack { path: path.into() })?;
        let old = self.trees.take(path).map(|(old_bytes, mut tree)| {
            tree.edit(&edit(&old_bytes, &bytes));
            tree
        });
        let incremental = old.is_some();
        let (facts, tree) =
            extract::extract(pack, grammar, &mut self.parser, &bytes, old.as_ref(), path)?;
        batch.stats.parsed += 1;
        batch.stats.incremental += usize::from(incremental);
        let key = cache_key(pack, grammar, hash);
        batch.cache_put.push((
            path.to_owned(),
            Cached {
                key: key.clone(),
                facts: facts.clone(),
            },
        ));
        self.trees.put(path, bytes, tree);
        let affected = self.project.upsert(path, key.language, hash, facts)?;
        changed.insert(path.to_owned());
        resolve.extend(affected.files);
        batch.dependents.extend(affected.dependents);
        Ok(())
    }

    /// Computes the components again, from the current ones (at commit, ADR 0015 and 0016).
    /// Returns the structure update.
    pub fn recluster(&mut self) -> Batch {
        let previous = self.project.components().assignment.clone();
        self.project.recluster(&previous);
        Batch {
            updates: self
                .project
                .updates(true, &BTreeSet::new(), &BTreeSet::new()),
            ..Batch::default()
        }
    }

    /// Applies contents given in memory, `None` for an absent file: a commit's versions of the
    /// files that differ from it (ADR 0015). Nothing is cached and no tree is kept; `refresh`
    /// on the same paths brings the working tree back. Paths no pack reads, or that make no
    /// file id, are ignored.
    pub fn apply_contents(
        &mut self,
        contents: &BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<Batch> {
        let mut batch = Batch::default();
        let mut structure = false;
        let mut changed = BTreeSet::new();
        let mut resolve = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for (path, content) in contents {
            if NodeId::file(path).is_err() {
                continue;
            }
            let bytes = content
                .as_ref()
                .filter(|b| b.len() <= MAX_FILE_BYTES && self.packs.reads(path));
            let Some(bytes) = bytes else {
                if self.project.file(path).is_some() {
                    let affected = self.project.remove(path);
                    structure |= affected.structure;
                    resolve.extend(affected.files);
                    removed.insert(path.clone());
                }
                continue;
            };
            let hash = content_hash(bytes);
            if self
                .project
                .file(path)
                .is_some_and(|f| f.content_hash == hash)
            {
                batch.stats.unchanged += 1;
                continue;
            }
            let (pack, grammar) = self
                .packs
                .for_path(path)
                .ok_or_else(|| Error::NoPack { path: path.clone() })?;
            let (facts, _) = extract::extract(pack, grammar, &mut self.parser, bytes, None, path)?;
            batch.stats.parsed += 1;
            structure |= self.project.file(path).is_none();
            let affected = self
                .project
                .upsert(path, pack.language.clone(), hash, facts)?;
            changed.insert(path.clone());
            resolve.extend(affected.files);
        }
        let resolve: BTreeSet<String> = resolve.difference(&removed).cloned().collect();
        batch.updates = self.project.updates(structure, &changed, &resolve);
        for file in &removed {
            if let Ok(scope) = NodeId::file(file).and_then(|id| ScopeKey::of_file(&id)) {
                batch.updates.push(ScopeUpdate::removal(scope));
            }
        }
        batch.stats.removed = removed.len();
        batch.stats.files = self.project.files().count();
        batch.changed = changed;
        batch.removed = removed;
        Ok(batch)
    }

    /// The source files the walk accepts now (`.gitignore`, hidden directories, valid paths):
    /// what the consolidation may take from a commit (ADR 0015).
    pub fn walk_sources(&mut self) -> BTreeSet<String> {
        self.walk().sources.into_iter().collect()
    }

    /// Whether the last walk entered this directory: a file in a directory it skipped (hidden,
    /// ignored) is not indexed.
    pub fn walked(&self, dir: &str) -> bool {
        self.walked.contains(dir)
    }

    /// Directories the last walk entered, absolute: what a watcher needs to watch.
    pub fn walked_dirs(&self) -> Vec<PathBuf> {
        self.walked
            .iter()
            .map(|d| {
                if d.is_empty() {
                    self.root.clone()
                } else {
                    self.root.join(d)
                }
            })
            .collect()
    }

    /// The packages changed: every file is placed and named again, components are computed
    /// again from the current ones (a package renamed takes its components along), and only
    /// the files whose content changed are parsed.
    fn resync(
        &mut self,
        mut batch: Batch,
        packages: Vec<PackageInfo>,
        sources: Vec<String>,
    ) -> Result<Batch> {
        let sources: BTreeSet<String> = sources.into_iter().collect();
        let known: BTreeSet<String> = self.project.files().map(|f| f.path.clone()).collect();
        let mut removed = BTreeSet::new();
        for gone in known.difference(&sources) {
            self.project.remove(gone);
            self.trees.remove(gone);
            removed.insert(gone.clone());
        }
        let mut affected = self.project.set_packages(packages)?;
        let mut changed = BTreeSet::new();
        for path in &sources {
            self.reindex(path, &mut batch, &mut changed, &mut affected.files)?;
        }
        let previous = self.project.components().assignment.clone();
        self.project.recluster(&previous);
        // Every file was placed again: its nodes may have new ids.
        changed.extend(affected.files.iter().cloned());
        let changed: BTreeSet<String> = changed.difference(&removed).cloned().collect();
        batch.updates = self.project.updates(true, &changed, &BTreeSet::new());
        for file in &removed {
            if let Ok(scope) = NodeId::file(file).and_then(|id| ScopeKey::of_file(&id)) {
                batch.updates.push(ScopeUpdate::removal(scope));
            }
            batch.cache_remove.push(file.clone());
        }
        batch.stats.removed = removed.len();
        batch.stats.files = self.project.files().count();
        batch.changed = changed;
        batch.removed = removed;
        batch.dependents.clear();
        batch.packages_changed = true;
        Ok(batch)
    }

    /// The manifests and the source files a pack reads, relative to the root, sorted. The walk
    /// respects `.gitignore`, skips hidden directories, and rejects paths that make no file id.
    fn walk(&mut self) -> Walk {
        let mut walk = Walk::default();
        let mut walked = BTreeSet::from([String::new()]);
        let walker = WalkBuilder::new(&self.root)
            .hidden(true)
            .require_git(false)
            .build();
        for entry in walker.flatten() {
            if entry.file_type().is_some_and(|t| t.is_dir()) {
                walked.extend(self.relative(entry.path()));
                continue;
            }
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Some(path) = self.relative(entry.path()) else {
                continue;
            };
            if is_manifest(&path) {
                walk.manifests.push(path);
            } else if self.packs.reads(&path) {
                if NodeId::file(&path).is_ok() {
                    walk.sources.push(path);
                } else {
                    walk.rejected.push(path);
                }
            }
        }
        walk.manifests.sort();
        walk.sources.sort();
        self.walked = walked;
        walk
    }

    /// A path relative to the root, with `/` separators; `None` outside the root.
    fn relative(&self, path: &Path) -> Option<String> {
        let resolved;
        let relative = if path.is_absolute() {
            if let Ok(inside) = path.strip_prefix(&self.root) {
                inside
            } else if let Some(inside) = self
                .given_root
                .as_ref()
                .and_then(|g| path.strip_prefix(g).ok())
            {
                inside
            } else {
                // Through another symbolic link: the parent still exists, even for a file
                // that was just deleted.
                let parent = path.parent()?.canonicalize().ok()?;
                resolved = parent.join(path.file_name()?);
                resolved.strip_prefix(&self.root).ok()?
            }
        } else {
            path
        };
        let parts: Option<Vec<&str>> = relative
            .components()
            .map(|c| match c {
                std::path::Component::Normal(part) => part.to_str(),
                _ => None,
            })
            .collect();
        let parts = parts?;
        (!parts.is_empty()).then(|| parts.join("/"))
    }
}

#[derive(Default)]
struct Walk {
    manifests: Vec<String>,
    sources: Vec<String>,
    /// Files a pack would read, whose path makes no file id (a `\\` in the name).
    rejected: Vec<String>,
}

/// What reading a file gave.
enum Source {
    Bytes(Vec<u8>),
    /// Generated or minified code (ADR 0013).
    TooLarge,
    Unreadable,
}

/// The cache key: the pack's version and its grammar's (ADR 0012, 0013).
fn cache_key(
    pack: &crate::Pack,
    grammar: &crate::pack::Grammar,
    content_hash: ContentHash,
) -> CacheKey {
    CacheKey {
        language: pack.language.clone(),
        pack_version: format!("{}+{}", pack.manifest.version, grammar.version),
        extractor: EXTRACTOR_VERSION,
        content_hash,
    }
}

struct Read {
    path: String,
    key: CacheKey,
    facts: FileFacts,
    parsed: bool,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn read_source(root: &Path, path: &str) -> Source {
    match std::fs::read(root.join(path)) {
        Ok(bytes) if bytes.len() > MAX_FILE_BYTES => Source::TooLarge,
        Ok(bytes) => Source::Bytes(bytes),
        Err(_) => Source::Unreadable,
    }
}

pub fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::from_bytes(*blake3::hash(bytes).as_bytes())
}

fn file_path(id: &NodeId) -> Option<&str> {
    id.as_str().strip_prefix("file:")
}

fn is_manifest(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    MANIFESTS.iter().any(|(file, _)| *file == name)
}

/// Creating or deleting a library changes how `src/main.rs` is named (ADR 0013).
fn is_cargo_lib(path: &str) -> bool {
    path == "src/lib.rs" || path.ends_with("/src/lib.rs")
}

/// The edit that turns `old` into `new`: the bytes between their common prefix and suffix.
fn edit(old: &[u8], new: &[u8]) -> InputEdit {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    InputEdit {
        start_byte: prefix,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: point(old, prefix),
        old_end_position: point(old, old_end),
        new_end_position: point(new, new_end),
    }
}

fn point(text: &[u8], byte: usize) -> Point {
    let before = &text[..byte];
    let row = before.iter().filter(|b| **b == b'\n').count();
    let column = before.iter().rev().take_while(|b| **b != b'\n').count();
    Point { row, column }
}

/// The trees of the last edited files, for incremental reparsing.
#[derive(Default)]
struct Trees {
    trees: HashMap<String, (Vec<u8>, Tree)>,
    order: VecDeque<String>,
}

impl Trees {
    fn take(&mut self, path: &str) -> Option<(Vec<u8>, Tree)> {
        self.order.retain(|p| p != path);
        self.trees.remove(path)
    }

    fn put(&mut self, path: &str, bytes: Vec<u8>, tree: Tree) {
        self.order.retain(|p| p != path);
        self.order.push_back(path.to_owned());
        self.trees.insert(path.to_owned(), (bytes, tree));
        while self.order.len() > KEPT_TREES {
            if let Some(oldest) = self.order.pop_front() {
                self.trees.remove(&oldest);
            }
        }
    }

    fn remove(&mut self, path: &str) {
        self.take(path);
    }
}

/// The edits are only an optimisation: the extraction from an edited tree equals the one from
/// a fresh parse (tested in `tests/indexer.rs`).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_cover_the_changed_bytes() {
        let e = edit(b"fn a() {}\nfn b() {}\n", b"fn a() {}\nfn bc() {}\n");
        assert_eq!((e.start_byte, e.old_end_byte, e.new_end_byte), (14, 14, 15));
        assert_eq!(e.start_position, Point { row: 1, column: 4 });
        let e = edit(b"abc", b"abc");
        assert_eq!((e.start_byte, e.old_end_byte, e.new_end_byte), (3, 3, 3));
        let e = edit(b"aaa", b"aa");
        assert_eq!((e.start_byte, e.old_end_byte, e.new_end_byte), (2, 3, 2));
        let e = edit(b"", b"x\ny");
        assert_eq!(e.new_end_position, Point { row: 1, column: 1 });
    }

    #[test]
    fn trees_keep_the_last_edited_files() {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let mut trees = Trees::default();
        for i in 0..KEPT_TREES + 3 {
            let tree = parser.parse("fn a() {}", None).unwrap();
            trees.put(&format!("f{i}.rs"), Vec::new(), tree);
        }
        assert_eq!(trees.trees.len(), KEPT_TREES);
        assert!(trees.take("f0.rs").is_none());
        assert!(trees.take(&format!("f{}.rs", KEPT_TREES + 2)).is_some());
    }

    #[test]
    fn manifests_and_libraries_trigger_a_resync() {
        assert!(is_manifest("Cargo.toml"));
        assert!(is_manifest("crates/a/package.json"));
        assert!(!is_manifest("crates/a/Cargo.lock"));
        assert!(is_cargo_lib("crates/a/src/lib.rs"));
        assert!(!is_cargo_lib("crates/a/src/lib.rsx"));
    }
}
