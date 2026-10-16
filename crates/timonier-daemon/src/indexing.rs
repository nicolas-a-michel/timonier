//! The indexing pipeline: the indexer feeds the store, and the store keeps the extraction cache
//! (ADR 0013). Synchronous: the daemon runs it on a dedicated thread.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use timonier_core::{
    EdgeKind, Freeze, Language, NodeId, NodeKind, Provenance, ScopeKey, ScopeUpdate, SnapshotKind,
    SourceObservations,
};
use timonier_index::components::{Component, Components, Parent};
use timonier_index::lsp::{self, Call, LanguageServer, LspError, Outcome, Priority, Queue};
use timonier_index::{BatchStats, CacheKey, Cached, Indexer, Packs};
use timonier_store::{ApplyStats, CachedExtraction, Store};

use crate::{Error, Result, git};

/// What one pass did, and how long it took from its start to the commit in SQLite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub index: BatchStats,
    pub apply: ApplyStats,
    pub elapsed: Duration,
}

/// A commit consolidated into a frozen snapshot (ADR 0015).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consolidation {
    pub sha: String,
    /// The frozen `commit` snapshot.
    pub seq: u64,
    /// Files whose committed version differs from the working tree.
    pub dirty: usize,
    pub elapsed: Duration,
}

/// What one refinement step did (ADR 0014).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refinement {
    /// The file's calls are verified.
    Verified {
        path: String,
        calls: usize,
        /// Older verified files whose calls now go between files only (ADR 0018).
        compacted: usize,
        apply: ApplyStats,
        elapsed: Duration,
    },
    /// Other work came first: the file waits again.
    Yielded { path: String },
    /// No server reads the file, or the file is gone.
    Skipped { path: String, reason: String },
}

/// The budget of symbol → symbol calls from the LSP in the head (ADR 0018).
pub const SYMBOL_EDGE_BUDGET: usize = 50_000;

pub struct Pipeline {
    store: Store,
    indexer: Indexer,
    refinements: Queue,
    /// Calls of each file verified in detail, and the order of their verifications.
    detailed: HashMap<String, (u64, Vec<Call>)>,
    compacted: HashSet<String>,
    verifications: u64,
    budget: usize,
}

impl Pipeline {
    pub fn new(root: impl Into<PathBuf>, store: Store) -> Result<Self> {
        Ok(Self {
            indexer: Indexer::new(root, Packs::builtin()?)?,
            store,
            refinements: Queue::default(),
            detailed: HashMap::new(),
            compacted: HashSet::new(),
            verifications: 0,
            budget: SYMBOL_EDGE_BUDGET,
        })
    }

    pub fn root(&self) -> &Path {
        self.indexer.root()
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn indexer(&self) -> &Indexer {
        &self.indexer
    }

    /// Indexes the whole repository, reusing the cached extractions, and records the result.
    /// The components and the LSP verifications of the head are kept (ADR 0014, 0016). The
    /// commit HEAD points at gets its snapshot if it has none yet (ADR 0015).
    pub fn index_all(&mut self) -> Result<Report> {
        let report = self.index_everything()?;
        self.consolidate()?;
        Ok(report)
    }

    /// The whole repository against the store: what a start does, and what brings the memory
    /// back in line with the disk after a failed write.
    fn index_everything(&mut self) -> Result<Report> {
        let start = Instant::now();
        let mut reader = self.store.reader()?;
        let cache = cached(reader.extractions()?);
        let stored = reader.scopes()?;
        let components = components_at_head(&mut reader)?;
        let batch = self
            .indexer
            .index_all(&cache, &stored, components.as_ref())?;
        let report = self.record(batch, start)?;
        let resets = self.reload_refinements(&reader)?;
        if !resets.is_empty() {
            self.store.apply_all(resets)?;
        }
        Ok(report)
    }

    /// The LSP state after a restart (ADR 0014, 0018): the files the store says are verified
    /// come back as verified, their calls held in detail or compacted. Those whose calls leave
    /// symbols that no longer exist (renamed while away) lose their verification and wait for
    /// refinement again: the updates to apply are returned.
    fn reload_refinements(&mut self, reader: &timonier_store::Reader) -> Result<Vec<ScopeUpdate>> {
        // Files waiting (displayed, opened) keep their place.
        self.refinements.reset_verified();
        self.detailed.clear();
        self.compacted.clear();
        let files = self.indexer.project().symbol_files();
        let mut resets = Vec::new();
        for (scope, observations) in reader.observations(Provenance::Lsp)? {
            let Some(file) = scope.file() else { continue };
            let Some(path) = file.as_str().strip_prefix("file:").map(str::to_owned) else {
                continue;
            };
            if !observations.verified.contains(&EdgeKind::Calls) {
                continue;
            }
            let Some(entry) = self.indexer.project().file(&path) else {
                continue;
            };
            let compacted = !observations.observations.is_empty()
                && observations.observations.iter().all(|o| o.src == file);
            let mut calls = Vec::new();
            let mut stale = false;
            for o in &observations.observations {
                if o.src == file {
                    continue;
                }
                match (files.get(&o.src), files.get(&o.dst)) {
                    (Some(src_file), Some(dst_file)) if *src_file == path => calls.push(Call {
                        src: o.src.clone(),
                        dst: o.dst.clone(),
                        dst_file: dst_file.clone(),
                        sites: o.weight,
                    }),
                    _ => stale = true,
                }
            }
            if stale {
                resets.push(
                    ScopeUpdate::new(scope.clone()).with_observations(
                        SourceObservations::new(Provenance::Lsp, Vec::new())
                            .computed_on(entry.content_hash),
                    ),
                );
                self.refinements.request(&path, Priority::Dependent);
                continue;
            }
            self.refinements.verified(&path, Priority::Dependent);
            self.verifications += 1;
            if compacted {
                self.compacted.insert(path);
            } else {
                self.detailed.insert(path, (self.verifications, calls));
            }
        }
        Ok(resets)
    }

    /// Computes the components again now, from the current ones (ADR 0016). The daemon does it
    /// at each commit; this is for an explicit request.
    pub fn recluster(&mut self) -> Result<ApplyStats> {
        let batch = self.indexer.recluster();
        Ok(self.store.apply_all(batch.updates)?)
    }

    /// Walks the repository now and gives the directories it enters: what a watcher watches
    /// before the first indexing.
    pub fn walk_dirs(&mut self) -> Vec<PathBuf> {
        self.indexer.walk_sources();
        self.indexer.walked_dirs()
    }

    /// The git directory, where a commit shows (`None` outside a repository).
    pub fn git_dir(&self) -> Option<PathBuf> {
        git::git_dir(self.root())
    }

    /// When HEAD points at a commit that has no snapshot yet, freezes one that describes the
    /// committed tree exactly: the indexable files that differ from the commit take their
    /// committed version for the time of the freeze (ADR 0015). The working tree always comes
    /// back, even when the freeze fails.
    pub fn consolidate(&mut self) -> Result<Option<Consolidation>> {
        let start = Instant::now();
        let root = self.root().to_path_buf();
        let Some(sha) = git::head(&root) else {
            return Ok(None);
        };
        let latest = self
            .store
            .reader()?
            .latest(SnapshotKind::Commit)?
            .and_then(|s| s.commit_sha);
        if latest.as_deref() == Some(sha.as_str()) {
            return Ok(None);
        }
        // The commit may be seen before the saves that led to it: what changed since the last
        // commit snapshot is read again first, so that the head is the working tree.
        if let Some(previous) = &latest {
            match git::changed_between(&root, previous, &sha) {
                Some(changed) if !changed.is_empty() => {
                    let paths: Vec<PathBuf> = changed.iter().map(PathBuf::from).collect();
                    self.refresh(&paths)?;
                }
                Some(_) => {}
                None => tracing::warn!(%previous, %sha, "git diff failed: the head may lag behind"),
            }
        }
        let Some(dirty) = git::dirty(&root) else {
            tracing::warn!(%sha, "git status failed: no commit snapshot this time");
            return Ok(None);
        };
        let Some(dirty) = self.indexable(&root, dirty) else {
            tracing::warn!(%sha, "git check-ignore failed: no commit snapshot this time");
            return Ok(None);
        };
        let Some(committed) = git::committed(&root, &sha, &dirty) else {
            tracing::warn!(%sha, "git could not read the commit: no commit snapshot this time");
            return Ok(None);
        };
        let frozen = self.freeze_commit(&sha, &committed);
        if !dirty.is_empty() {
            // Back to the working tree. The committed versions dropped the LSP verification of
            // these files: `refresh` sends the verified ones back to the queue.
            let paths: Vec<PathBuf> = dirty.iter().map(PathBuf::from).collect();
            self.refresh(&paths)?;
        }
        let snapshot = frozen?;
        Ok(Some(Consolidation {
            sha,
            seq: snapshot.seq,
            dirty: dirty.len(),
            elapsed: start.elapsed(),
        }))
    }

    /// The dirty paths the walk would index (ADR 0013, 0015): read by a pack, a valid id, and
    /// accepted by the walk; or, gone from the working tree, in no hidden directory and not
    /// ignored by git. `None` if git fails.
    fn indexable(&mut self, root: &Path, dirty: Vec<String>) -> Option<Vec<String>> {
        let candidates: Vec<String> = dirty
            .into_iter()
            .filter(|p| self.indexer.packs().reads(p) && NodeId::file(p).is_ok())
            .collect();
        if candidates.is_empty() {
            return Some(candidates);
        }
        let sources = self.indexer.walk_sources();
        let gone: Vec<String> = candidates
            .iter()
            .filter(|p| !root.join(p).exists() && !p.split('/').any(|s| s.starts_with('.')))
            .cloned()
            .collect();
        let ignored = git::ignored(root, &gone)?;
        Some(
            candidates
                .into_iter()
                .filter(|p| sources.contains(p) || (gone.contains(p) && !ignored.contains(p)))
                .collect(),
        )
    }

    /// The committed versions in, the full re-clustering (spec, ADR 0016), the freeze.
    fn freeze_commit(
        &mut self,
        sha: &str,
        committed: &BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<timonier_core::Snapshot> {
        if !committed.is_empty() {
            let batch = self.indexer.apply_contents(committed)?;
            if !batch.updates.is_empty() {
                self.store.apply_all(batch.updates)?;
            }
        }
        let structure = self.indexer.recluster();
        self.store.apply_all(structure.updates)?;
        Ok(self.store.freeze(Freeze::Commit {
            sha: sha.to_owned(),
        })?)
    }

    /// Files or directories changed on disk. Verified files that changed, and verified files
    /// that depend on a changed signature, wait for refinement again (ADR 0014). When the
    /// packages changed, ids may have changed: every LSP verification is dropped.
    pub fn refresh(&mut self, paths: &[PathBuf]) -> Result<Report> {
        match self.refresh_once(paths) {
            Ok(report) => Ok(report),
            // The indexer's memory moved ahead of the store: index everything again, from the
            // disk and the cache, to bring both back in line.
            Err(error) => {
                tracing::warn!(%error, "an update failed: indexing everything again");
                self.index_everything().map_err(|_| error)
            }
        }
    }

    fn refresh_once(&mut self, paths: &[PathBuf]) -> Result<Report> {
        let start = Instant::now();
        let mut batch = self.indexer.refresh(paths)?;
        if batch.packages_changed {
            let verified: Vec<String> = self
                .refinements
                .verified_paths()
                .map(|(p, _)| p.clone())
                .collect();
            for path in verified {
                if let Some(entry) = self.indexer.project().file(&path)
                    && let Ok(scope) = ScopeKey::of_file(&entry.id)
                {
                    batch.updates.push(
                        ScopeUpdate::new(scope).with_observations(
                            SourceObservations::new(Provenance::Lsp, Vec::new())
                                .computed_on(entry.content_hash),
                        ),
                    );
                }
            }
        }
        for path in &batch.removed {
            self.refinements.forget(path);
        }
        // A changed file lost its verification: its calls are gone from the store.
        for path in batch.removed.iter().chain(&batch.changed) {
            self.detailed.remove(path);
            self.compacted.remove(path);
        }
        self.refinements.changed(&batch.changed);
        self.refinements.dependents(&batch.dependents);
        self.record(batch, start)
    }

    /// Changes the budget of symbol → symbol calls (ADR 0018); for tests and measurements.
    pub fn set_symbol_edge_budget(&mut self, budget: usize) {
        self.budget = budget;
    }

    /// Symbol → symbol calls from the LSP held in detail, and files compacted.
    pub fn symbol_edges(&self) -> (usize, usize) {
        (
            self.detailed.values().map(|(_, c)| c.len()).sum(),
            self.compacted.len(),
        )
    }

    /// Asks for the LSP refinement of a file: displayed on the map, or opened in the editor.
    pub fn request_refinement(&mut self, path: &str, priority: Priority) {
        self.refinements.request(path, priority);
    }

    pub fn refinements_waiting(&self) -> usize {
        self.refinements.len()
    }

    /// Refines the next waiting file with `server`, unless `yield_now` says other work waits.
    pub fn refine_next(
        &mut self,
        server: &mut dyn LanguageServer,
        yield_now: impl Fn() -> bool,
    ) -> Result<Option<Refinement>> {
        let Some((path, priority)) = self.refinements.pop() else {
            return Ok(None);
        };
        let start = Instant::now();
        let Some(job) = self.indexer.project().refine_job(&path) else {
            self.refinements.forget(&path);
            return Ok(Some(Refinement::Skipped {
                path,
                reason: "not indexed".into(),
            }));
        };
        let reader = self.store.reader()?;
        let locate = |file: &str, at| -> Option<NodeId> {
            let id = NodeId::file(file).ok()?;
            reader.symbol_at(&id, at).ok().flatten()
        };
        match lsp::refine(&job, server, locate, yield_now) {
            Ok(Outcome::Yielded) => {
                self.refinements.put_back(&path, priority);
                Ok(Some(Refinement::Yielded { path }))
            }
            Ok(Outcome::Done(update, calls)) => {
                let count = calls.len();
                let mut apply = self.store.apply(update)?;
                self.refinements.verified(&path, priority);
                self.verifications += 1;
                self.compacted.remove(&path);
                self.detailed
                    .insert(path.clone(), (self.verifications, calls));
                let (compacted, more) = self.compact(&path)?;
                apply = add(apply, more);
                Ok(Some(Refinement::Verified {
                    path,
                    calls: count,
                    compacted,
                    apply,
                    elapsed: start.elapsed(),
                }))
            }
            Err(error @ (LspError::Unavailable(_) | LspError::Failed(_))) => {
                Ok(Some(Refinement::Skipped {
                    path,
                    reason: error.to_string(),
                }))
            }
        }
    }

    /// Over the budget, the files verified longest ago keep their calls between files only,
    /// down to 90 % of the budget (ADR 0018). `keep` is the file just verified.
    fn compact(&mut self, keep: &str) -> Result<(usize, ApplyStats)> {
        let mut total: usize = self.detailed.values().map(|(_, c)| c.len()).sum();
        if total <= self.budget {
            return Ok((0, ApplyStats::default()));
        }
        let target = self.budget * 9 / 10;
        let mut oldest: Vec<(u64, String)> = self
            .detailed
            .iter()
            .filter(|(path, _)| path.as_str() != keep)
            .map(|(path, (order, _))| (*order, path.clone()))
            .collect();
        oldest.sort();
        let mut updates = Vec::new();
        for (_, path) in oldest {
            if total <= target {
                break;
            }
            let Some((_, calls)) = self.detailed.remove(&path) else {
                continue;
            };
            total -= calls.len();
            let Some(job) = self.indexer.project().refine_job(&path) else {
                continue;
            };
            let file = NodeId::file(&path).map_err(timonier_index::Error::from)?;
            updates.push(lsp::compacted(&job.scope, &file, job.content_hash, &calls));
            self.compacted.insert(path);
        }
        let compacted = updates.len();
        let apply = if updates.is_empty() {
            ApplyStats::default()
        } else {
            self.store.apply_all(updates)?
        };
        Ok((compacted, apply))
    }

    fn record(&mut self, batch: timonier_index::Batch, start: Instant) -> Result<Report> {
        for problem in &batch.problems {
            tracing::warn!(%problem, "not indexed");
        }
        let apply = if batch.updates.is_empty() {
            ApplyStats::default()
        } else {
            self.store.apply_all(batch.updates)?
        };
        if !batch.cache_put.is_empty() || !batch.cache_remove.is_empty() {
            let put = batch
                .cache_put
                .into_iter()
                .map(|(path, cached)| encode(path, cached))
                .collect::<Result<Vec<_>>>()?;
            self.store.save_extractions(put, batch.cache_remove)?;
        }
        Ok(Report {
            index: batch.stats,
            apply,
            elapsed: start.elapsed(),
        })
    }
}

fn add(a: ApplyStats, b: ApplyStats) -> ApplyStats {
    ApplyStats {
        nodes_added: a.nodes_added + b.nodes_added,
        nodes_removed: a.nodes_removed + b.nodes_removed,
        nodes_changed: a.nodes_changed + b.nodes_changed,
        spans_moved: a.spans_moved + b.spans_moved,
        edges_added: a.edges_added + b.edges_added,
        edges_removed: a.edges_removed + b.edges_removed,
        edges_changed: a.edges_changed + b.edges_changed,
        nodes_taken_over: a.nodes_taken_over + b.nodes_taken_over,
        edges_taken_over: a.edges_taken_over + b.edges_taken_over,
        nodes_waiting: a.nodes_waiting + b.nodes_waiting,
        edges_waiting: a.edges_waiting + b.edges_waiting,
        stale_observations: a.stale_observations + b.stale_observations,
    }
}

/// The components of the head, and what hangs in them: a restart keeps them (ADR 0016). The
/// groups of external dependencies are not among them: they follow the manifests.
fn components_at_head(reader: &mut timonier_store::Reader) -> Result<Option<Components>> {
    let head = reader.head()?.seq;
    let nodes = reader.nodes_at(head)?;
    // Nothing indexed yet: no components to keep, the first clustering will compute them.
    if !nodes.iter().any(|n| n.kind == NodeKind::Repo) {
        return Ok(None);
    }
    let key = |id: &NodeId| id.as_str().strip_prefix("component:").map(str::to_owned);
    let external = |key: &str| key == "external" || key.starts_with("external/");
    let mut components = Components::default();
    for node in nodes.iter().filter(|n| n.kind == NodeKind::Component) {
        let Some(own) = key(&node.id).filter(|k| !external(k)) else {
            continue;
        };
        let parent = match node.parent.as_ref() {
            Some(p) if p.as_str().starts_with("pkg:") => Parent::Package(p.clone()),
            Some(p) => match key(p) {
                Some(parent) => Parent::Component(parent),
                None => Parent::Repo,
            },
            None => continue,
        };
        components.components.insert(
            own,
            Component {
                name: node.name.clone(),
                parent,
            },
        );
    }
    for node in nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Package | NodeKind::File))
    {
        if let Some(own) = node.parent.as_ref().and_then(key).filter(|k| !external(k)) {
            components.assignment.insert(node.id.clone(), own);
        }
    }
    Ok(Some(components))
}

fn encode(path: String, cached: Cached) -> Result<CachedExtraction> {
    let facts = serde_json::to_string(&cached.facts).map_err(|source| Error::Cache {
        path: path.clone(),
        source,
    })?;
    Ok(CachedExtraction {
        path,
        language: cached.key.language.to_string(),
        pack_version: cached.key.pack_version,
        extractor: cached.key.extractor,
        content_hash: cached.key.content_hash,
        facts,
    })
}

/// Stored extractions the indexer can use. An entry that no longer decodes (written by another
/// version) is simply not used: the file is parsed again.
fn cached(entries: Vec<CachedExtraction>) -> HashMap<String, Cached> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let facts = serde_json::from_str(&entry.facts).ok()?;
            let key = CacheKey {
                language: Language::new(entry.language).ok()?,
                pack_version: entry.pack_version,
                extractor: entry.extractor,
                content_hash: entry.content_hash,
            };
            Some((entry.path, Cached { key, facts }))
        })
        .collect()
}
