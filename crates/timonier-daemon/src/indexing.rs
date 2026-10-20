//! The indexing pipeline: the indexer feeds the store, and the store keeps the extraction cache
//! (ADR 0013). Synchronous: the daemon runs it on a dedicated thread.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use timonier_core::{
    ContentHash, EdgeKind, Freeze, Language, NodeId, NodeKind, Provenance, ScopeKey, ScopeUpdate,
    Snapshot, SnapshotKind, SourceObservations,
};
use timonier_index::components::{Component, Components, Parent};
use timonier_index::ecosystem::PackageInfo;
use timonier_index::indexer::is_manifest;
use timonier_index::lsp::{
    self, Call, LanguageServer, Lookup, LspError, Outcome, Priority, Queue, Task, USAGE_KINDS,
    Verified,
};
use timonier_index::project::FileEntry;
use timonier_index::{BatchStats, CacheKey, Cached, Indexer, Packs};
use timonier_store::{ApplyStats, CachedExtraction, PendingLookup, RefinementDebt, Store};

use crate::{Error, Result, git};

/// What one pass did, and how long it took from its start to the commit in SQLite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub index: BatchStats,
    pub apply: ApplyStats,
    pub elapsed: Duration,
}

/// What a change in the git directory led to (ADR 0015).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Consolidated {
    /// The snapshot of a commit HEAD points at, frozen if it had none.
    pub snapshot: Option<Consolidation>,
    /// What changed in the head on the way: a store caught up, changes the watcher has yet to
    /// bring. The views read the head again, even when no snapshot is frozen (review n° 16,
    /// m16-5).
    pub head: ApplyStats,
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
    /// The file's calls are verified, and its other usages when the server could (ADR 0022).
    Verified {
        path: String,
        calls: usize,
        /// Its `References`, `Implements` and `Inherits` found by their definitions.
        usages: usize,
        /// Why its other usages stay those of tree-sitter, when they do.
        unverified: Option<String>,
        /// Older verified files whose calls now go between files only (ADR 0018).
        compacted: usize,
        apply: ApplyStats,
        elapsed: Duration,
    },
    /// The dependents of a changed symbol, named by the language server (ADR 0022), or by name
    /// when it could not say.
    LookedUp {
        path: String,
        /// Verified files that come back for refinement.
        dependents: usize,
        by_name: bool,
        elapsed: Duration,
    },
    /// Other work came first: the file waits again.
    Yielded { path: String },
    /// The client's documents, or the disk, are not the content indexed: the file or the lookup
    /// is held until they are (ADR 0022).
    Held { path: String, reason: String },
    /// No server reads the file, or the file is gone.
    Skipped { path: String, reason: String },
}

/// Why a file or a lookup is held (ADR 0022).
const UNSAVED: &str = "the client's documents are not saved";
const DISK: &str = "a file on disk is not the content indexed";

/// The budget of symbol → symbol calls from the LSP in the head (ADR 0018).
pub const SYMBOL_EDGE_BUDGET: usize = 50_000;

pub struct Pipeline {
    store: Store,
    indexer: Indexer,
    refinements: Queue,
    /// What each file verified in detail holds, and the order of their verifications.
    detailed: HashMap<String, (u64, Verified)>,
    compacted: HashSet<String>,
    /// Files whose disk was found not to hold the content indexed: read again next.
    unseen: BTreeSet<String>,
    /// What the refinement owed when last saved (ADR 0022): kept across a restart.
    owed: RefinementDebt,
    /// The store missed a batch the indexer holds (an update failed twice): the next refresh
    /// indexes everything first (review n° 12, m12-2).
    behind: bool,
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
            unseen: BTreeSet::new(),
            owed: RefinementDebt::default(),
            behind: false,
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
        // Owed in memory and not in the store yet (a batch that failed): owed still (review
        // n° 12, m12-1).
        let carried = self.refinements.take_stale();
        let mut reader = self.store.reader()?;
        let cache = cached(reader.extractions()?);
        let stored = reader.scopes()?;
        // What each file held before this start: a verified edge whose target changed while
        // the daemon was away may lead elsewhere now (ADR 0022).
        let mut before: HashMap<String, ContentHash> = HashMap::new();
        for scope in &stored {
            let Some(file) = scope.file() else { continue };
            if let Some(path) = file.as_str().strip_prefix("file:")
                && let Some(hash) = reader.content_hash(scope)?
            {
                before.insert(path.to_owned(), hash);
            }
        }
        let components = components_at_head(&mut reader)?;
        let mut batch = self
            .indexer
            .index_all(&cache, &stored, components.as_ref())?;
        // Found before the batch is recorded, and recorded with it: a stop between the two would
        // leave the new hashes stored, and the verifications they make stale unseen at the next
        // start (review n° 10, m10-1).
        let resets = self.reload_refinements(&reader, &before)?;
        batch.updates.extend(resets);
        // What the previous run owed: its stale dependents, its lookups not answered. Owed
        // again, and recorded with the batch.
        let owed = reader.refinement_debt()?;
        let lookups = owed
            .lookups
            .iter()
            .filter_map(|pending| {
                Some(Lookup {
                    path: pending.path.clone(),
                    symbol: NodeId::parse(&pending.symbol).ok()?,
                    at: pending.at,
                    content_hash: pending.content_hash,
                    by_name: pending.by_name.iter().cloned().collect(),
                })
            })
            .collect();
        self.owed = owed.clone();
        self.refinements
            .owe(owed.dependents.into_iter().chain(carried), lookups);
        self.record(batch, start)
    }

    /// What the refinement owes now (ADR 0022).
    fn debt(&self) -> RefinementDebt {
        let (dependents, lookups) = self.refinements.debt();
        RefinementDebt {
            dependents,
            lookups: lookups
                .into_iter()
                .map(|lookup| PendingLookup {
                    path: lookup.path,
                    symbol: lookup.symbol.to_string(),
                    at: lookup.at,
                    content_hash: lookup.content_hash,
                    by_name: lookup.by_name.into_iter().collect(),
                })
                .collect(),
        }
    }

    /// Saves what the refinement owes, when it changed (ADR 0022): a restart, even after a
    /// crash, picks it up again (review n° 9, I9-2). Only the change is written (review n° 10,
    /// m10-2). A failure is not the step's: kept in memory, the debt is written at the next
    /// step (m10-3). After a refinement step only: a batch carries its debt (`record`).
    fn save_debt(&mut self) {
        let debt = self.debt();
        let change = debt.changes_from(&self.owed);
        if change.is_empty() {
            return;
        }
        match self.store.update_refinement_debt(change) {
            Ok(()) => self.owed = debt,
            Err(error) => tracing::warn!(%error, "the refinement debt was not saved"),
        }
    }

    /// The LSP state after a restart (ADR 0014, 0018, 0022): the files the store says are
    /// verified come back as verified, their calls held in detail or compacted. Those whose
    /// edges leave symbols or files that no longer exist (renamed while away), or lead between
    /// files to one whose content changed meanwhile (`before`), lose their verification and
    /// wait for refinement again: the updates to apply are returned.
    fn reload_refinements(
        &mut self,
        reader: &timonier_store::Reader,
        before: &HashMap<String, ContentHash>,
    ) -> Result<Vec<ScopeUpdate>> {
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
            // Changed while away: its verification goes with the batch of this start.
            if before.get(&path) != Some(&entry.content_hash) {
                continue;
            }
            // Only calls are compacted: the other usages are kept as they are (ADR 0022).
            let is_call = |o: &&timonier_core::Observation| o.kind == EdgeKind::Calls;
            let compacted = observations.observations.iter().any(|o| is_call(&o))
                && observations
                    .observations
                    .iter()
                    .filter(is_call)
                    .all(|o| o.src == file);
            let usages: Vec<timonier_core::Observation> = observations
                .observations
                .iter()
                .filter(|o| USAGE_KINDS.contains(&o.kind))
                .cloned()
                .collect();
            let kinds: Vec<EdgeKind> = observations.verified.to_vec();
            let project = self.indexer.project();
            // Still indexed, with the content it had when the edge was verified: a definition
            // may have left a file changed meanwhile, or a signature changed (ADR 0022).
            let unchanged = |p: &str| {
                project
                    .file(p)
                    .is_some_and(|f| before.get(p) == Some(&f.content_hash))
            };
            let indexed = |id: &timonier_core::NodeId| {
                id.as_str().strip_prefix("file:").is_some_and(unchanged)
            };
            let mut calls = Vec::new();
            let mut stale = false;
            for o in observations.observations.iter().filter(is_call) {
                if o.src == file {
                    continue;
                }
                match (files.get(&o.src), files.get(&o.dst)) {
                    (Some(src_file), Some(dst_file))
                        if *src_file == path && unchanged(dst_file) =>
                    {
                        calls.push(Call {
                            src: o.src.clone(),
                            dst: o.dst.clone(),
                            dst_file: dst_file.clone(),
                            sites: o.weight,
                        });
                    }
                    _ => stale = true,
                }
            }
            // The other usages, and the calls compacted between files: their ends must still be
            // there, and unchanged, or the verification is out of date (ADR 0022).
            stale |= observations.observations.iter().any(|o| match o.kind {
                EdgeKind::References => o.src != file || !indexed(&o.dst),
                EdgeKind::Implements | EdgeKind::Inherits => {
                    files.get(&o.src) != Some(&path) || !files.contains_key(&o.dst)
                }
                EdgeKind::Calls if o.src == file => !indexed(&o.dst),
                _ => false,
            });
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
            // Compacted, it has its detail to get back (ADR 0018).
            let whole = !compacted
                && [EdgeKind::Calls]
                    .iter()
                    .chain(&USAGE_KINDS)
                    .all(|k| kinds.contains(k));
            self.refinements.verified(&path, Priority::Dependent, whole);
            self.verifications += 1;
            if compacted {
                self.compacted.insert(path);
            } else {
                let verified = Verified {
                    calls,
                    usages,
                    kinds,
                    unverified: None,
                };
                self.detailed.insert(path, (self.verifications, verified));
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

    /// Freezes the head as reviewed (ADR 0024). The review must be the working tree, as before a
    /// commit snapshot: a store behind the indexer catches up first, and what the disk holds
    /// that the watcher has yet to bring is read, a save made just before the click included.
    pub fn mark_reviewed(&mut self) -> Result<Snapshot> {
        self.catch_up()?;
        self.sync_with_disk()?;
        Ok(self.store.freeze(Freeze::ReviewMark)?)
    }

    /// Where git keeps what the watcher follows; `None` outside a repository.
    pub(crate) fn git_places(&self) -> Option<git::Places> {
        git::places(self.root())
    }

    /// When HEAD points at a commit that has no snapshot yet, freezes one that describes the
    /// committed tree exactly: the indexable files that differ from the commit take their
    /// committed version for the time of the freeze (ADR 0015). The working tree always comes
    /// back, even when the freeze fails.
    pub fn consolidate(&mut self) -> Result<Consolidated> {
        let start = Instant::now();
        // A store behind the indexer would be frozen as the commit (review n° 13, m13-2), and
        // stays behind until the next change otherwise: caught up on any change in the git
        // directory, a new commit or not (review n° 16, m16-1).
        let mut head = self
            .catch_up()?
            .map(|report| report.apply)
            .unwrap_or_default();
        let root = self.root().to_path_buf();
        let Some(sha) = git::head(&root) else {
            return Ok(Consolidated {
                snapshot: None,
                head,
            });
        };
        let latest = self
            .store
            .reader()?
            .latest(SnapshotKind::Commit)?
            .and_then(|s| s.commit_sha);
        if latest.as_deref() == Some(sha.as_str()) {
            return Ok(Consolidated {
                snapshot: None,
                head,
            });
        }
        // The commit may be seen before the saves that led to it: what changed since the last
        // commit snapshot is read again first, so that the head is the working tree.
        if let Some(previous) = &latest {
            match git::changed_between(&root, previous, &sha) {
                Some(changed) if !changed.is_empty() => {
                    let paths: Vec<PathBuf> = changed.iter().map(PathBuf::from).collect();
                    head = add(head, self.refresh(&paths)?.apply);
                }
                Some(_) => {}
                None => tracing::warn!(%previous, %sha, "git diff failed: the head may lag behind"),
            }
        }
        // The index as the disk is: what git calls clean is frozen from the index, and a file
        // back to its committed version without the daemon having read it would be frozen as
        // the index holds it (review n° 14, I14-1).
        head = add(head, self.sync_with_disk()?.apply);
        let Some(dirty) = git::dirty(&root) else {
            tracing::warn!(%sha, "git status failed: no commit snapshot this time");
            return Ok(Consolidated {
                snapshot: None,
                head,
            });
        };
        // The manifests too: the commit's packages are those of its manifests (Q23).
        let Some((dirty, manifests)) = self.indexable(&root, dirty) else {
            tracing::warn!(%sha, "git check-ignore failed: no commit snapshot this time");
            return Ok(Consolidated {
                snapshot: None,
                head,
            });
        };
        let (Some(committed), Some(committed_manifests)) = (
            git::committed(&root, &sha, &dirty),
            git::committed(&root, &sha, &manifests),
        ) else {
            tracing::warn!(%sha, "git could not read the commit: no commit snapshot this time");
            return Ok(Consolidated {
                snapshot: None,
                head,
            });
        };
        let mut overlay = committed.clone();
        overlay.extend(committed_manifests);
        let packages = self.indexer.committed_packages(&overlay);
        // What the index holds of them before the freeze: the working tree the LSP followed,
        // and the LSP verification of that content, which the freeze drops. When the commit has
        // other packages, every file is placed again, and every file's verification is kept.
        let held: BTreeMap<String, Option<FileEntry>> = dirty
            .iter()
            .map(|p| (p.clone(), self.indexer.project().file(p).cloned()))
            .collect();
        let held_packages = packages.as_ref().map(|_| self.indexer.packages());
        let kept = if held_packages.is_some() {
            let every: BTreeMap<String, Option<FileEntry>> = self
                .indexer
                .project()
                .files()
                .map(|f| (f.path.clone(), Some(f.clone())))
                .collect();
            self.verifications_of(&every)?
        } else {
            self.verifications_of(&held)?
        };
        let frozen = self.freeze_commit(&sha, &committed, packages);
        if !dirty.is_empty() || held_packages.is_some() {
            // Back to the working tree, in two steps (review n° 13, I13-1, I13-2). First as the
            // index held it, from memory: the state before the freeze comes back whole, its LSP
            // verification included, and nothing is asked (review n° 12, I12-1). Then the disk,
            // read as any change: what changed since the index last read it is followed from
            // what the LSP saw, not from the commit (review n° 11, I11-1).
            match self.restore(&held, &kept, held_packages) {
                Ok(()) => {
                    let paths: Vec<PathBuf> = dirty.iter().map(PathBuf::from).collect();
                    head = add(head, self.refresh(&paths)?.apply);
                }
                // Back from the disk at once, the restart rules judging what stays verified
                // (review n° 14, m14-1).
                Err(error) => {
                    tracing::warn!(%error, "the working tree did not come back: indexing everything again");
                    head = add(head, self.catch_up()?.map(|r| r.apply).unwrap_or_default());
                }
            }
        }
        let snapshot = frozen?;
        Ok(Consolidated {
            snapshot: Some(Consolidation {
                sha,
                seq: snapshot.seq,
                dirty: dirty.len(),
                elapsed: start.elapsed(),
            }),
            head,
        })
    }

    /// The LSP verification the store holds of these files, at the content the index holds:
    /// what the working tree gets back after a commit snapshot.
    fn verifications_of(
        &self,
        files: &BTreeMap<String, Option<FileEntry>>,
    ) -> Result<HashMap<String, SourceObservations>> {
        let wanted: HashMap<&str, ContentHash> = files
            .iter()
            .filter_map(|(path, entry)| Some((path.as_str(), entry.as_ref()?.content_hash)))
            .collect();
        if wanted.is_empty() {
            return Ok(HashMap::new());
        }
        let mut kept = HashMap::new();
        for (scope, observations) in self.store.reader()?.observations(Provenance::Lsp)? {
            let Some(file) = scope.file() else { continue };
            let Some(path) = file.as_str().strip_prefix("file:") else {
                continue;
            };
            if let Some(hash) = wanted.get(path) {
                kept.insert(path.to_owned(), observations.computed_on(*hash));
            }
        }
        Ok(kept)
    }

    /// Reads again the indexed files the disk holds otherwise, and the source files the walk
    /// finds that the index lacks: changes the watcher has yet to bring (ADR 0015, 0022).
    fn sync_with_disk(&mut self) -> Result<Report> {
        let indexed: Vec<String> = self
            .indexer
            .project()
            .files()
            .map(|f| f.path.clone())
            .collect();
        let paths: Vec<PathBuf> = self.differing(&indexed).iter().map(PathBuf::from).collect();
        // And the walk again: files an ignore file now skips go, those it no longer skips come
        // (review n° 15, I15-1).
        self.refresh_with(&paths, true)
    }

    /// Puts the dirty files back as the index held them before a commit snapshot, with their
    /// LSP verification, in one batch: the queue, the detail and the debt are as they were,
    /// since the freeze touched only the store and the indexer's memory. Failed, the next
    /// refresh indexes everything first, and the restart rules judge what stays verified.
    fn restore(
        &mut self,
        held: &BTreeMap<String, Option<FileEntry>>,
        kept: &HashMap<String, SourceObservations>,
        packages: Option<Vec<PackageInfo>>,
    ) -> Result<()> {
        let start = Instant::now();
        let restored = self.indexer.restore(held).and_then(|mut batch| {
            // The working tree's packages, when the commit had others (Q23).
            if let Some(packages) = packages {
                let placed = self.indexer.place_under(packages, true)?;
                batch.updates.extend(placed.updates);
                batch.changed.extend(placed.changed);
            }
            Ok(batch)
        });
        let restored = restored.map(|mut batch| {
            for (path, observations) in kept {
                if let Some(entry) = self.indexer.project().file(path)
                    && observations.computed_on == Some(entry.content_hash)
                    && let Ok(scope) = ScopeKey::of_file(&entry.id)
                {
                    batch
                        .updates
                        .push(ScopeUpdate::new(scope).with_observations(observations.clone()));
                }
            }
            batch
        });
        let recorded = match restored {
            Ok(batch) => self.record(batch, start).map(|_| ()),
            Err(error) => Err(error.into()),
        };
        if recorded.is_err() {
            self.behind = true;
        }
        recorded
    }

    /// The dirty source files and manifests the walk would read (ADR 0013, 0015, 0025): a
    /// source read by a pack, with a valid id; accepted by the walk, or, gone from the working
    /// tree, in no hidden directory and not ignored by git (review n° 2 of Phase 5, m-7, for the
    /// manifests). `None` if git fails.
    fn indexable(&mut self, root: &Path, dirty: Vec<String>) -> Option<(Vec<String>, Vec<String>)> {
        let (manifests, sources): (Vec<String>, Vec<String>) =
            dirty.into_iter().partition(|p| is_manifest(p));
        let sources: Vec<String> = sources
            .into_iter()
            .filter(|p| self.indexer.packs().reads(p) && NodeId::file(p).is_ok())
            .collect();
        if sources.is_empty() && manifests.is_empty() {
            return Some((sources, manifests));
        }
        let (walked_sources, walked_manifests) = self.indexer.walk_files();
        let gone: Vec<String> = sources
            .iter()
            .chain(&manifests)
            .filter(|p| !root.join(p).exists() && !p.split('/').any(|s| s.starts_with('.')))
            .cloned()
            .collect();
        let ignored = git::ignored(root, &gone)?;
        let kept = |walked: &BTreeSet<String>, paths: Vec<String>| -> Vec<String> {
            paths
                .into_iter()
                .filter(|p| walked.contains(p) || (gone.contains(p) && !ignored.contains(p)))
                .collect()
        };
        Some((
            kept(&walked_sources, sources),
            kept(&walked_manifests, manifests),
        ))
    }

    /// The committed versions in, the full re-clustering (spec, ADR 0016), the freeze.
    fn freeze_commit(
        &mut self,
        sha: &str,
        committed: &BTreeMap<String, Option<Vec<u8>>>,
        packages: Option<Vec<PackageInfo>>,
    ) -> Result<timonier_core::Snapshot> {
        if !committed.is_empty() {
            let batch = self.indexer.apply_contents(committed)?;
            if !batch.updates.is_empty() {
                self.store.apply_all(batch.updates)?;
            }
        }
        // The commit's packages (Q23): every file placed again, its ids may change. Its LSP
        // verification would hang from ids the commit has not, and hide tree-sitter's edges: it
        // goes for the freeze, and comes back with the working tree.
        if let Some(packages) = packages {
            let mut batch = self.indexer.place_under(packages, false)?;
            for path in &batch.changed {
                if let Some(entry) = self.indexer.project().file(path)
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
            self.store.apply_all(batch.updates)?;
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
        self.refresh_with(paths, false)
    }

    /// [`Pipeline::refresh`], and the walk again as if an ignore file changed: one outside the
    /// root, git's own or that of a folder above it (ADR 0023).
    pub fn reconcile(&mut self, paths: &[PathBuf]) -> Result<Report> {
        self.refresh_with(paths, true)
    }

    /// [`Pipeline::refresh`]; `reconcile` walks again as if an ignore file changed.
    fn refresh_with(&mut self, paths: &[PathBuf], reconcile: bool) -> Result<Report> {
        let caught = self.catch_up()?;
        match self.refresh_once(paths, reconcile) {
            // What catching up changed is told with it: the views read the head again
            // (review n° 15, m15-3).
            Ok(report) => Ok(match caught {
                Some(caught) => Report {
                    apply: add(caught.apply, report.apply),
                    ..report
                },
                None => report,
            }),
            // The indexer's memory moved ahead of the store: index everything again, from the
            // disk and the cache, to bring both back in line. Failed again, the next refresh
            // tries first.
            Err(error) => {
                tracing::warn!(%error, "an update failed: indexing everything again");
                self.index_everything().map_err(|_| {
                    self.behind = true;
                    error
                })
            }
        }
    }

    /// Whether the store missed a batch the indexer holds, to be indexed again first: for tests
    /// and diagnostics.
    pub fn store_behind(&self) -> bool {
        self.behind
    }

    /// The store missed a batch (an update failed twice, or a restore): everything is indexed
    /// again first, so that nothing is built on a store behind the indexer (review n° 12,
    /// m12-2; n° 13, m13-2).
    fn catch_up(&mut self) -> Result<Option<Report>> {
        if !self.behind {
            return Ok(None);
        }
        let report = self.index_everything()?;
        self.behind = false;
        Ok(Some(report))
    }

    fn refresh_once(&mut self, paths: &[PathBuf], reconcile: bool) -> Result<Report> {
        let start = Instant::now();
        let mut batch = if reconcile {
            self.indexer.reconcile(paths)?
        } else {
            self.indexer.refresh(paths)?
        };
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
        self.refinements.look_up(std::mem::take(&mut batch.lookups));
        // What was held may get other answers now. Nothing changed, nothing to ask again: a file
        // the daemon cannot read is skipped, and would otherwise be asked about without end
        // (review n° 8, I8-1).
        if !batch.changed.is_empty() || !batch.removed.is_empty() || batch.packages_changed {
            self.refinements.release();
        }
        self.record(batch, start)
    }

    /// Changes the budget of symbol → symbol calls (ADR 0018); for tests and measurements.
    pub fn set_symbol_edge_budget(&mut self, budget: usize) {
        self.budget = budget;
    }

    /// Symbol → symbol calls from the LSP held in detail, and files compacted.
    pub fn symbol_edges(&self) -> (usize, usize) {
        (
            self.detailed.values().map(|(_, v)| v.calls.len()).sum(),
            self.compacted.len(),
        )
    }

    /// Asks for the LSP refinement of a file: displayed on the map, or opened in the editor. A
    /// file whose current content is verified for every kind is not asked again.
    pub fn request_refinement(&mut self, path: &str, priority: Priority) {
        self.refinements.ask(path, priority);
    }

    pub fn refinements_waiting(&self) -> usize {
        self.refinements.len()
    }

    /// Files and lookups held until the client's documents are saved (ADR 0022).
    pub fn refinements_held(&self) -> usize {
        self.refinements.held()
    }

    /// The client's documents are saved, or another client relays: what was held waits again.
    pub fn release_held(&mut self) {
        self.refinements.release();
    }

    /// The indexed files among these whose disk does not hold the content indexed: unreadable,
    /// gone, or changed in a way the daemon has yet to see (ADR 0022). A file no longer indexed
    /// is not among them: answers into it make no edge.
    fn differing<'a>(&self, paths: impl IntoIterator<Item = &'a String>) -> Vec<String> {
        let project = self.indexer.project();
        let root = self.indexer.root();
        paths
            .into_iter()
            .filter(|path| {
                project.file(path).is_some_and(|file| {
                    !std::fs::read(root.join(path)).is_ok_and(|bytes| {
                        timonier_index::indexer::content_hash(&bytes) == file.content_hash
                    })
                })
            })
            .cloned()
            .collect()
    }

    /// Whether the disk holds, for each of these files, the content indexed. Otherwise answers
    /// about them came from other content: the files that differ are read again next
    /// (`reread_unseen`).
    fn as_indexed<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) -> bool {
        let unseen = self.differing(paths);
        let as_indexed = unseen.is_empty();
        self.unseen.extend(unseen);
        as_indexed
    }

    /// Reads again the files found on disk with another content than the one indexed, without
    /// waiting for the watcher, which may have missed the change (ADR 0022). What they held is
    /// released when they changed, or match the index now (a change undone); not when they
    /// stay out of reach, unreadable: asked again, they would only be held again. `None` when
    /// there is nothing to read.
    pub fn reread_unseen(&mut self) -> Option<Result<Report>> {
        if self.unseen.is_empty() {
            return None;
        }
        let paths = std::mem::take(&mut self.unseen);
        let root = self.root().to_path_buf();
        let absolute: Vec<PathBuf> = paths.iter().map(|path| root.join(path)).collect();
        let report = self.refresh(&absolute);
        if self.differing(&paths).is_empty() {
            self.refinements.release();
        }
        Some(report)
    }

    /// Refines the next waiting file with `server`, unless `yield_now` says other work waits.
    pub fn refine_next(
        &mut self,
        server: &mut dyn LanguageServer,
        yield_now: impl Fn() -> bool,
    ) -> Result<Option<Refinement>> {
        let step = self.refine_step(server, yield_now)?;
        self.save_debt();
        Ok(step)
    }

    fn refine_step(
        &mut self,
        server: &mut dyn LanguageServer,
        yield_now: impl Fn() -> bool,
    ) -> Result<Option<Refinement>> {
        // Another client answers now: what the previous one refused, it may answer.
        if server.changed_hands() {
            self.refinements.release();
        }
        let (path, priority) = match self.refinements.next_task() {
            None => return Ok(None),
            Some(Task::LookUp(lookup)) => return Ok(Some(self.look_up(lookup, server, yield_now))),
            Some(Task::Refine(path, priority)) => (path, priority),
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
        let project = self.indexer.project();
        let indexed = |file: &str| project.file(file).map(|f| f.id.clone());
        let outcome = lsp::refine(&job, server, locate, indexed, yield_now);
        drop(reader);
        match outcome {
            Ok(Outcome::Yielded) => {
                self.refinements.put_back(&path, priority);
                Ok(Some(Refinement::Yielded { path }))
            }
            Ok(Outcome::Done(refined)) if !self.as_indexed(&refined.read) => {
                // A change the daemon has yet to see: reading it releases the file.
                self.refinements.hold(&path, priority);
                Ok(Some(Refinement::Held {
                    path,
                    reason: DISK.into(),
                }))
            }
            Ok(Outcome::Done(refined)) => {
                let lsp::Refined {
                    update, verified, ..
                } = *refined;
                let count = verified.calls.len();
                let usages = verified.usages.len();
                let unverified = verified.unverified.clone();
                let whole = unverified.is_none();
                let mut apply = self.store.apply(update)?;
                self.refinements.verified(&path, priority, whole);
                self.verifications += 1;
                self.compacted.remove(&path);
                self.detailed
                    .insert(path.clone(), (self.verifications, verified));
                let (compacted, more) = self.compact(&path)?;
                apply = add(apply, more);
                Ok(Some(Refinement::Verified {
                    path,
                    calls: count,
                    usages,
                    unverified,
                    compacted,
                    apply,
                    elapsed: start.elapsed(),
                }))
            }
            Err(LspError::Interrupted) => {
                self.refinements.put_back(&path, priority);
                Ok(Some(Refinement::Yielded { path }))
            }
            Err(LspError::Unsaved) => {
                self.refinements.hold(&path, priority);
                Ok(Some(Refinement::Held {
                    path,
                    reason: UNSAVED.into(),
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

    /// Which files use a changed symbol (ADR 0022): those verified come back as dependents. In a
    /// file changed since the lookup was made, the symbol is found again by its id; gone, its
    /// dependents are those that cite its name, so that none is missed.
    fn look_up(
        &mut self,
        mut lookup: Lookup,
        server: &mut dyn LanguageServer,
        yield_now: impl Fn() -> bool,
    ) -> Refinement {
        let start = Instant::now();
        let path = lookup.path.clone();
        let project = self.indexer.project();
        let current = project.file(&path).map(|f| f.content_hash);
        let at = match current {
            Some(hash) if hash == lookup.content_hash => Some(lookup.at),
            Some(hash) => project.name_at(&path, &lookup.symbol).inspect(|at| {
                lookup.at = *at;
                lookup.content_hash = hash;
            }),
            None => None,
        };
        let (users, by_name): (BTreeSet<String>, bool) = match at {
            None => (lookup.by_name, true),
            Some(at) => match server.references_or_yield(&path, at, &yield_now) {
                Err(LspError::Interrupted) => {
                    self.refinements.put_back_lookup(lookup);
                    return Refinement::Yielded { path };
                }
                Err(LspError::Unsaved) => {
                    self.refinements.hold_lookup(lookup);
                    return Refinement::Held {
                        path,
                        reason: UNSAVED.into(),
                    };
                }
                Ok(_) if !self.as_indexed([&path]) => {
                    self.refinements.hold_lookup(lookup);
                    return Refinement::Held {
                        path,
                        reason: DISK.into(),
                    };
                }
                // Users outside the file, named: those. None: the server may not know yet, or
                // the name is another symbol's elsewhere; by name, no dependent is missed.
                Ok(users) if users.iter().any(|u| *u != path) => {
                    (users.into_iter().filter(|u| *u != path).collect(), false)
                }
                Ok(_) => (lookup.by_name, true),
                Err(LspError::Unavailable(_) | LspError::Failed(_)) => (lookup.by_name, true),
            },
        };
        Refinement::LookedUp {
            dependents: self.refinements.dependents(&users),
            path,
            by_name,
            elapsed: start.elapsed(),
        }
    }

    /// Over the budget, the files verified longest ago keep their calls between files only,
    /// down to 90 % of the budget (ADR 0018). `keep` is the file just verified.
    fn compact(&mut self, keep: &str) -> Result<(usize, ApplyStats)> {
        let mut total: usize = self.detailed.values().map(|(_, v)| v.calls.len()).sum();
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
            let Some((_, verified)) = self.detailed.remove(&path) else {
                continue;
            };
            total -= verified.calls.len();
            let Some(job) = self.indexer.project().refine_job(&path) else {
                continue;
            };
            let file = NodeId::file(&path).map_err(timonier_index::Error::from)?;
            updates.push(lsp::compacted(
                &job.scope,
                &file,
                job.content_hash,
                &verified,
            ));
            self.refinements.incomplete(&path);
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

    /// Records a batch, with what the refinement owes after it in the same transaction: a stop
    /// or a failure never leaves the batch recorded and the debt not (review n° 11, m11-1).
    fn record(&mut self, batch: timonier_index::Batch, start: Instant) -> Result<Report> {
        for problem in &batch.problems {
            tracing::warn!(%problem, "not indexed");
        }
        let debt = self.debt();
        let change = debt.changes_from(&self.owed);
        let apply = if batch.updates.is_empty() && change.is_empty() {
            ApplyStats::default()
        } else {
            let apply = self.store.apply_all_owing(batch.updates, change)?;
            self.owed = debt;
            apply
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
