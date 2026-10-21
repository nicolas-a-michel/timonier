//! Speeds 1 and 2 (spec « Technical decisions »). Changes on disk are debounced (~300 ms),
//! then the pipeline brings the graph up to date. When nothing else waits, the pipeline refines
//! one file with the language server (ADR 0014), and gives way as soon as a change arrives. A
//! dedicated thread runs the pipeline; the watcher only forwards paths.
//!
//! Only the directories the walk enters are watched, one by one, plus the git directory: a
//! `target/` or `node_modules/` of thousands of directories costs no watch. New directories
//! are added as the walk finds them, and read again once watched.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use notify_debouncer_full::notify::event::{AccessKind, AccessMode, ModifyKind};
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use timonier_core::Snapshot;
use timonier_index::lsp::{LanguageServer, Priority};

use crate::indexing::{Consolidated, Pipeline, Refinement, Report};
use crate::{Error, Result, git};

/// The debounce of the spec (« debounce ~300 ms »).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Commands waiting for the pipeline. A burst beyond this blocks the sender until the pipeline
/// catches up: back-pressure, not loss.
const QUEUE: usize = 64;

type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// What the pipeline thread did.
#[derive(Debug)]
pub enum Event {
    Indexed(Result<Report>),
    Refined(Result<Refinement>),
    /// A change in the git directory was consolidated: a new commit got its snapshot, or the
    /// head changed on the way (ADR 0015). An error may come after the head changed.
    Committed(Result<Consolidated>),
    /// The head was marked as reviewed: the review diff starts again from it (ADR 0024).
    Reviewed(Result<Snapshot>),
    /// The head was frozen for a session's start or end (ADR 0029).
    Frozen(Result<Snapshot>),
}

impl Event {
    /// Whether the views may have to read the graph again.
    pub fn changes_graph(&self) -> bool {
        match self {
            Event::Indexed(Ok(report)) => !report.apply.is_empty(),
            // The head may have changed before the failure: a start that wrote it, then failed
            // to freeze the commit (review n° 17, m17-5).
            Event::Indexed(Err(_)) => true,
            Event::Refined(Ok(Refinement::Verified { apply, .. })) => !apply.is_empty(),
            Event::Committed(Ok(consolidated)) => {
                consolidated.snapshot.is_some() || !consolidated.head.is_empty()
            }
            // The head may have changed before the failure (review n° 16, m16-5).
            Event::Committed(Err(_)) => true,
            // The diff the views show starts from the new review (ADR 0024); and the head may
            // have changed before a failure, the disk read again (review n° 2 of Phase 5, m-3).
            Event::Reviewed(_) => true,
            // A new head opened; a failure may come after the disk was read again.
            Event::Frozen(_) => true,
            _ => false,
        }
    }
}

enum Command {
    Changed(Vec<PathBuf>),
    Refine(String, Priority),
    /// The server may have become ready, or its client's documents saved: what was held waits
    /// again, and the queue is looked at again (ADR 0022).
    Nudge,
    /// Mark the head as reviewed, between two steps of the pipeline: never in the middle of a
    /// commit snapshot (ADR 0024). The answer goes back on the channel.
    Review(SyncSender<std::result::Result<Snapshot, String>>),
    /// Freeze the head as a `working_tree` snapshot for a session, at the same place (ADR 0029).
    Freeze(SyncSender<std::result::Result<Snapshot, String>>),
}

/// A snapshot the pipeline thread was asked for: a review mark, or a session's.
pub struct PendingReview {
    answer: mpsc::Receiver<std::result::Result<Snapshot, String>>,
    failed: fn(String) -> Error,
}

impl PendingReview {
    /// The snapshot, once frozen.
    pub fn wait(self) -> Result<Snapshot> {
        self.answer
            .recv()
            .map_err(|_| Error::Stopped)?
            .map_err(self.failed)
    }
}

/// A running watch. Dropping it stops the watcher and the pipeline thread.
pub struct Watch {
    debouncer: Option<Arc<Mutex<Watcher>>>,
    commands: Option<SyncSender<Command>>,
    pending: Arc<AtomicUsize>,
    /// Files waiting for refinement, as of the pipeline's last step.
    refinements: Arc<AtomicUsize>,
    /// Files and lookups held until the client's documents are saved, as of the last step.
    held: Arc<AtomicUsize>,
    worker: Option<JoinHandle<Pipeline>>,
}

/// The directories under watch. A watch dies with its directory: a directory removed then
/// created again (a branch switch) must be watched again, or its changes are never seen.
struct Watched {
    watcher: Weak<Mutex<Watcher>>,
    dirs: BTreeSet<PathBuf>,
}

impl Watched {
    /// Forgets directories that are gone, watches again those the events name (created again,
    /// or moved in), and watches `wanted` ones not watched yet. Returns the directories watched
    /// anew: what changed in them before their watch must be read again.
    fn sync(&mut self, wanted: Vec<PathBuf>, named: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let Some(watcher) = self.watcher.upgrade() else {
            return Ok(Vec::new());
        };
        let mut watcher = watcher.lock().map_err(|_| Error::Poisoned)?;
        let gone: Vec<PathBuf> = self.dirs.iter().filter(|d| !d.is_dir()).cloned().collect();
        for dir in gone {
            // The watch went with the directory: forgetting it is all there is to do.
            let _ = watcher.unwatch(&dir);
            self.dirs.remove(&dir);
        }
        let mut again = Vec::new();
        for dir in named {
            if self.dirs.remove(dir) {
                let _ = watcher.unwatch(dir);
                again.push(dir.clone());
            }
        }
        let mut added = Vec::new();
        for dir in wanted.into_iter().chain(again) {
            if self.dirs.contains(&dir) || !dir.is_dir() {
                continue;
            }
            match watcher.watch(&dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.dirs.insert(dir.clone());
                    added.push(dir);
                }
                Err(error) if !dir.is_dir() => {
                    tracing::debug!(%error, "directory gone before its watch");
                }
                Err(source) => return Err(Error::Watch { path: dir, source }),
            }
        }
        Ok(added)
    }
}

impl Watch {
    /// Watches the pipeline's repository, then indexes it: every directory the walk enters is
    /// watched first, so that a change made during the indexing is seen. Every debounced batch
    /// goes through the pipeline, and so do refinements when a server is given; `on_event`
    /// hears about each, on the pipeline thread.
    pub fn start<F>(
        mut pipeline: Pipeline,
        debounce: Duration,
        server: Option<Box<dyn LanguageServer + Send>>,
        mut on_event: F,
    ) -> Result<Self>
    where
        F: FnMut(Event) + Send + 'static,
    {
        let root = pipeline.root().to_path_buf();
        let git = pipeline.git_places();
        // The root as given, and as the file system names it: events come under either.
        let roots: Vec<PathBuf> = std::iter::once(root.clone())
            .chain(std::fs::canonicalize(&root).ok())
            .collect();
        let (commands, receive) = mpsc::sync_channel::<Command>(QUEUE);
        let pending = Arc::new(AtomicUsize::new(0));
        let changes = commands.clone();
        let counted = Arc::clone(&pending);
        let debouncer =
            new_debouncer(
                debounce,
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => {
                        let mut paths: Vec<PathBuf> = events
                            .into_iter()
                            .filter(|e| may_change(&e.event.kind))
                            .flat_map(|e| e.event.paths)
                            .collect();
                        paths.sort();
                        paths.dedup();
                        if !paths.is_empty() {
                            counted.fetch_add(1, Ordering::SeqCst);
                            // The pipeline is gone only when the watch stops.
                            let _ = changes.send(Command::Changed(paths));
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::warn!(%error, "file watcher");
                        }
                    }
                },
            )
            .map_err(|source| Error::Watch {
                path: root.clone(),
                source,
            })?;
        let debouncer = Arc::new(Mutex::new(debouncer));
        let mut watched = Watched {
            watcher: Arc::downgrade(&debouncer),
            dirs: BTreeSet::new(),
        };
        // HEAD and its log move in the git directory, inside the repository or not (worktree);
        // `info/exclude`, in the directory worktrees share, says what the walk skips. So do the
        // ignore files of the folders between the top of the working tree and the root
        // (ADR 0023).
        let mut wanted: Vec<PathBuf> = git
            .iter()
            .flat_map(|g| [g.dir.clone(), g.dir.join("logs"), g.common.join("info")])
            .collect();
        if let Some(git) = &git {
            wanted.extend(
                roots
                    .iter()
                    .flat_map(|r| {
                        r.ancestors()
                            .skip(1)
                            .take_while(|d| d.starts_with(&git.top))
                    })
                    .map(Path::to_path_buf),
            );
        }
        wanted.push(root);
        wanted.extend(pipeline.walk_dirs());
        watched.sync(wanted, &[])?;
        let waiting = Arc::clone(&pending);
        let refinements = Arc::new(AtomicUsize::new(0));
        let queued = Arc::clone(&refinements);
        let held = Arc::new(AtomicUsize::new(0));
        let holding = Arc::clone(&held);
        let worker = std::thread::Builder::new()
            .name("timonier-index".into())
            .spawn(move || {
                let mut pipeline = pipeline;
                let mut server = server;
                let mut watched = watched;
                // The watches are in place: index now.
                let report = pipeline.index_all();
                if let Err(error) = &report {
                    tracing::warn!(%error, "indexing failed");
                }
                on_event(Event::Indexed(report));
                follow(&mut pipeline, &mut watched, &[], &mut on_event);
                loop {
                    queued.store(pipeline.refinements_waiting(), Ordering::SeqCst);
                    holding.store(pipeline.refinements_held(), Ordering::SeqCst);
                    let idle_work = server.as_deref().is_some_and(|s| s.ready())
                        && pipeline.refinements_waiting() > 0;
                    let command = if idle_work {
                        match receive.try_recv() {
                            Ok(command) => Some(command),
                            Err(TryRecvError::Empty) => None,
                            Err(TryRecvError::Disconnected) => break,
                        }
                    } else {
                        match receive.recv() {
                            Ok(command) => Some(command),
                            Err(_) => break,
                        }
                    };
                    match command {
                        Some(Command::Changed(paths)) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            let named: Vec<PathBuf> =
                                paths.iter().filter(|p| p.is_dir()).cloned().collect();
                            changed(&mut pipeline, git.as_ref(), &roots, paths, &mut on_event);
                            follow(&mut pipeline, &mut watched, &named, &mut on_event);
                        }
                        Some(Command::Refine(path, priority)) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            pipeline.request_refinement(&path, priority);
                        }
                        Some(Command::Nudge) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            pipeline.release_held();
                        }
                        Some(Command::Review(answer)) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            let reviewed = pipeline.mark_reviewed();
                            match &reviewed {
                                Ok(snapshot) => {
                                    tracing::info!(seq = snapshot.seq, "marked as reviewed");
                                }
                                Err(error) => tracing::warn!(%error, "review mark failed"),
                            }
                            let _ = answer.send(match &reviewed {
                                Ok(snapshot) => Ok(snapshot.clone()),
                                Err(error) => Err(error.to_string()),
                            });
                            on_event(Event::Reviewed(reviewed));
                        }
                        Some(Command::Freeze(answer)) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            let frozen = pipeline.freeze_working_tree();
                            match &frozen {
                                Ok(snapshot) => {
                                    tracing::info!(seq = snapshot.seq, "frozen for a session");
                                }
                                Err(error) => tracing::warn!(%error, "session snapshot failed"),
                            }
                            let _ = answer.send(match &frozen {
                                Ok(snapshot) => Ok(snapshot.clone()),
                                Err(error) => Err(error.to_string()),
                            });
                            on_event(Event::Frozen(frozen));
                        }
                        None => {
                            let Some(server) = server.as_deref_mut() else {
                                continue;
                            };
                            let yield_now = || waiting.load(Ordering::SeqCst) > 0;
                            match pipeline.refine_next(server, yield_now) {
                                Ok(Some(refined)) => {
                                    log_refinement(&refined);
                                    on_event(Event::Refined(Ok(refined)));
                                    // The disk differs from the index: read it now, rather
                                    // than wait for the watcher, which may have missed it.
                                    if let Some(report) = pipeline.reread_unseen() {
                                        if let Err(error) = &report {
                                            tracing::warn!(%error, "indexing failed");
                                        }
                                        on_event(Event::Indexed(report));
                                    }
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    tracing::warn!(%error, "refinement failed");
                                    on_event(Event::Refined(Err(error)));
                                }
                            }
                        }
                    }
                }
                pipeline
            })
            .map_err(Error::Thread)?;
        Ok(Self {
            debouncer: Some(debouncer),
            commands: Some(commands),
            pending,
            refinements,
            held,
            worker: Some(worker),
        })
    }

    /// Asks for the LSP refinement of a file (displayed on the map, opened in the editor).
    pub fn request_refinement(&self, path: &str, priority: Priority) {
        if let Some(commands) = &self.commands {
            self.pending.fetch_add(1, Ordering::SeqCst);
            let _ = commands.send(Command::Refine(path.to_owned(), priority));
        }
    }

    /// The language server may answer now (a relaying client connected, or its documents are
    /// saved): refinement resumes, held work included.
    pub fn nudge(&self) {
        if let Some(commands) = &self.commands {
            self.pending.fetch_add(1, Ordering::SeqCst);
            let _ = commands.send(Command::Nudge);
        }
    }

    /// Asks the pipeline thread to mark the head as reviewed, once the step in progress is
    /// done (ADR 0024). Wait for it with [`PendingReview::wait`], without holding the watch.
    pub fn mark_reviewed(&self) -> Result<PendingReview> {
        let commands = self.commands.as_ref().ok_or(Error::Stopped)?;
        let (answer, answered) = mpsc::sync_channel(1);
        self.pending.fetch_add(1, Ordering::SeqCst);
        commands
            .send(Command::Review(answer))
            .map_err(|_| Error::Stopped)?;
        Ok(PendingReview {
            answer: answered,
            failed: Error::Review,
        })
    }

    /// Asks the pipeline thread to freeze the head for a session (ADR 0029), as
    /// [`Watch::mark_reviewed`] does for a review.
    pub fn freeze_working_tree(&self) -> Result<PendingReview> {
        let commands = self.commands.as_ref().ok_or(Error::Stopped)?;
        let (answer, answered) = mpsc::sync_channel(1);
        self.pending.fetch_add(1, Ordering::SeqCst);
        commands
            .send(Command::Freeze(answer))
            .map_err(|_| Error::Stopped)?;
        Ok(PendingReview {
            answer: answered,
            failed: Error::Freeze,
        })
    }

    /// Files waiting for LSP refinement (ADR 0014).
    pub fn refinements_waiting(&self) -> usize {
        self.refinements.load(Ordering::SeqCst)
    }

    /// Files and lookups held until the client's documents are saved (ADR 0022).
    pub fn refinements_held(&self) -> usize {
        self.held.load(Ordering::SeqCst)
    }

    /// Stops watching, lets the pipeline finish the step in progress, and gives it back.
    pub fn stop(mut self) -> Option<Pipeline> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Option<Pipeline> {
        // Dropping the watcher stops its thread, which drops its sender; dropping ours too
        // ends the pipeline thread's loop.
        self.debouncer.take();
        self.commands.take();
        self.worker.take().and_then(|worker| worker.join().ok())
    }
}

/// One line per refinement step, at debug level (ADR 0014, 0022).
fn log_refinement(refined: &Refinement) {
    match refined {
        Refinement::Verified {
            path,
            calls,
            usages,
            unverified,
            elapsed,
            ..
        } => tracing::debug!(
            file = %path,
            calls,
            usages,
            unverified = unverified.as_deref().unwrap_or("-"),
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            "file refined"
        ),
        Refinement::LookedUp {
            path,
            dependents,
            by_name,
            elapsed,
        } => tracing::debug!(
            file = %path,
            dependents,
            by_name,
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            "dependents looked up"
        ),
        Refinement::Yielded { path } => tracing::debug!(file = %path, "refinement yielded"),
        Refinement::Held { path, reason } => {
            tracing::debug!(file = %path, reason, "refinement held");
        }
        Refinement::Skipped { path, reason } => {
            tracing::debug!(file = %path, reason, "refinement skipped");
        }
    }
}

/// Whether an event may change what is indexed. Opening, reading, closing without writing or
/// changing metadata changes nothing: inotify reports the daemon's own reads, which would
/// otherwise feed it back, pass after pass. A close after writing does count.
fn may_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Any
        | EventKind::Create(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => true,
    }
}

/// Keeps the watches in line with the walk; directories watched anew are read again, since
/// what changed in them before their watch was not seen.
fn follow(
    pipeline: &mut Pipeline,
    watched: &mut Watched,
    named: &[PathBuf],
    on_event: &mut impl FnMut(Event),
) {
    match watched.sync(pipeline.indexer().walked_dirs(), named) {
        Ok(added) if !added.is_empty() => {
            let report = pipeline.refresh(&added);
            if let Err(error) = &report {
                tracing::warn!(%error, "indexing failed");
            }
            on_event(Event::Indexed(report));
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "file watcher"),
    }
}

/// A debounced batch: files go to the pipeline, a change in the git directory may be a commit.
/// An ignore file above the root, or git's own, makes the walk start again.
fn changed(
    pipeline: &mut Pipeline,
    places: Option<&git::Places>,
    roots: &[PathBuf],
    paths: Vec<PathBuf>,
    on_event: &mut impl FnMut(Event),
) {
    let (git, paths): (Vec<PathBuf>, Vec<PathBuf>) = paths
        .into_iter()
        .partition(|p| places.is_some_and(|g| p.starts_with(&g.dir) || p.starts_with(&g.common)));
    let (files, above): (Vec<PathBuf>, Vec<PathBuf>) = paths
        .into_iter()
        .partition(|p| roots.iter().any(|r| p.starts_with(r)));
    // Git's own ignore file, in the git directory, and those of the folders above the root: the
    // walk changes with them (ADR 0023). Nothing else above the root is indexed.
    let excluded = git.iter().any(|p| p.ends_with("info/exclude"))
        || above.iter().any(|p| {
            p.file_name()
                .is_some_and(|n| n == ".gitignore" || n == ".ignore")
        });
    if !files.is_empty() || excluded {
        let report = if excluded {
            pipeline.reconcile(&files)
        } else {
            pipeline.refresh(&files)
        };
        match &report {
            Ok(report) => tracing::debug!(
                files = files.len(),
                parsed = report.index.parsed,
                elapsed_ms = report.elapsed.as_secs_f64() * 1000.0,
                "graph updated"
            ),
            Err(error) => tracing::warn!(%error, "indexing failed"),
        }
        on_event(Event::Indexed(report));
    }
    if !git.is_empty() {
        match pipeline.consolidate() {
            Ok(consolidated) => {
                if let Some(snapshot) = &consolidated.snapshot {
                    tracing::info!(sha = %snapshot.sha, seq = snapshot.seq, "commit snapshot");
                }
                let event = Event::Committed(Ok(consolidated));
                if event.changes_graph() {
                    on_event(event);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "consolidation failed");
                on_event(Event::Committed(Err(error)));
            }
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use timonier_store::ApplyStats;

    use super::*;
    use crate::indexing::{Consolidated, Consolidation};

    /// Review n° 16, m16-5, and n° 17, m17-5: a consolidation tells the views when it froze a
    /// snapshot, when the head changed on the way without one, and when it failed; an indexing
    /// and a review mark tell them when they failed too.
    #[test]
    fn a_consolidation_tells_the_views_what_changed_the_head() {
        assert!(Event::Indexed(Err(Error::Poisoned)).changes_graph());
        let committed = |c: Consolidated| Event::Committed(Ok(c)).changes_graph();
        assert!(!committed(Consolidated::default()));
        assert!(committed(Consolidated {
            snapshot: None,
            head: ApplyStats {
                nodes_added: 1,
                ..ApplyStats::default()
            },
        }));
        assert!(committed(Consolidated {
            snapshot: Some(Consolidation {
                sha: "c0ffee".into(),
                seq: 2,
                dirty: 0,
                elapsed: Duration::ZERO,
            }),
            head: ApplyStats::default(),
        }));
        assert!(Event::Committed(Err(Error::Poisoned)).changes_graph());
        // Review n° 2 of Phase 5, m-3: a review mark that failed may have read the disk first.
        assert!(Event::Reviewed(Err(Error::Poisoned)).changes_graph());
    }
}
