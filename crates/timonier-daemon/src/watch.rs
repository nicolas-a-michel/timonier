//! Speeds 1 and 2 (spec « Décisions techniques »). Changes on disk are debounced (~300 ms),
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
use timonier_index::lsp::{LanguageServer, Priority};

use crate::indexing::{Consolidation, Pipeline, Refinement, Report};
use crate::{Error, Result};

/// The debounce of the spec (« anti-rebond ~300 ms »).
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
    /// A new commit got its snapshot (ADR 0015).
    Committed(Result<Consolidation>),
}

enum Command {
    Changed(Vec<PathBuf>),
    Refine(String, Priority),
}

/// A running watch. Dropping it stops the watcher and the pipeline thread.
pub struct Watch {
    debouncer: Option<Arc<Mutex<Watcher>>>,
    commands: Option<SyncSender<Command>>,
    pending: Arc<AtomicUsize>,
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
        let git_dir = pipeline.git_dir();
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
        // HEAD and its log move in the git directory, inside the repository or not (worktree).
        let mut wanted: Vec<PathBuf> = git_dir
            .iter()
            .flat_map(|d| [d.clone(), d.join("logs")])
            .collect();
        wanted.push(root);
        wanted.extend(pipeline.walk_dirs());
        watched.sync(wanted, &[])?;
        let waiting = Arc::clone(&pending);
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
                    let idle_work = server.is_some() && pipeline.refinements_waiting() > 0;
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
                            changed(&mut pipeline, git_dir.as_deref(), paths, &mut on_event);
                            follow(&mut pipeline, &mut watched, &named, &mut on_event);
                        }
                        Some(Command::Refine(path, priority)) => {
                            waiting.fetch_sub(1, Ordering::SeqCst);
                            pipeline.request_refinement(&path, priority);
                        }
                        None => {
                            let Some(server) = server.as_deref_mut() else {
                                continue;
                            };
                            let yield_now = || waiting.load(Ordering::SeqCst) > 0;
                            match pipeline.refine_next(server, yield_now) {
                                Ok(Some(refined)) => on_event(Event::Refined(Ok(refined))),
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
fn changed(
    pipeline: &mut Pipeline,
    git_dir: Option<&Path>,
    paths: Vec<PathBuf>,
    on_event: &mut impl FnMut(Event),
) {
    let (git, files): (Vec<PathBuf>, Vec<PathBuf>) = paths
        .into_iter()
        .partition(|p| git_dir.is_some_and(|d| p.starts_with(d)));
    if !files.is_empty() {
        let report = pipeline.refresh(&files);
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
            Ok(Some(consolidation)) => {
                tracing::info!(sha = %consolidation.sha, seq = consolidation.seq, "commit snapshot");
                on_event(Event::Committed(Ok(consolidation)));
            }
            Ok(None) => {}
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
