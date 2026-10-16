//! Speed 2 (ADR 0014): the language-server interface, the refinement queue, and the turning of
//! call hierarchies into verified `Calls` observations.

use std::collections::{BTreeMap, HashMap};

use timonier_core::{
    ContentHash, EdgeKind, NodeId, Observation, Position, Provenance, ScopeKey, ScopeUpdate,
    SourceObservations, SymbolKind,
};

use crate::project::Project;

/// A callee found by the language server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallTarget {
    /// The callee's file, relative to the repository root; `None` outside the repository.
    pub path: Option<String>,
    /// The start of the callee's name (`selectionRange.start`).
    pub at: Position,
    /// The number of call sites (`fromRanges`).
    pub sites: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LspError {
    /// No server reads this file: the job is dropped.
    #[error("no language server for `{0}`")]
    Unavailable(String),
    #[error("language server: {0}")]
    Failed(String),
}

/// What refinement asks a language server (ADR 0014). Positions are those of the model:
/// 0-based lines, UTF-8 byte columns; the implementation converts.
pub trait LanguageServer {
    /// `textDocument/prepareCallHierarchy` at `at`, then `callHierarchy/outgoingCalls`.
    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError>;
}

/// Refinement priorities, served from the highest (ADR 0014).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Dependent,
    Opened,
    Displayed,
}

/// One file to refine: the content it was read with, and its functions with the position of
/// their name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefineJob {
    pub path: String,
    pub scope: ScopeKey,
    pub content_hash: ContentHash,
    pub functions: Vec<(NodeId, Position)>,
}

impl Project {
    /// The job that refines a file, if the file is indexed.
    pub fn refine_job(&self, path: &str) -> Option<RefineJob> {
        let file = self.file(path)?;
        let functions = file
            .facts
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
            .filter_map(|(i, s)| Some((self.symbol_id(file, i)?, s.name_at)))
            .collect();
        Some(RefineJob {
            path: path.to_owned(),
            scope: ScopeKey::of_file(&file.id).ok()?,
            content_hash: file.content_hash,
            functions,
        })
    }
}

/// A verified call, with the file of its callee: what compaction needs (ADR 0018).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub src: NodeId,
    pub dst: NodeId,
    pub dst_file: String,
    pub sites: u32,
}

/// How a refinement ended.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The verified `Calls` of the file, for the store, and the calls themselves.
    Done(ScopeUpdate, Vec<Call>),
    /// Other work came first; the job starts again later.
    Yielded,
}

/// The update that keeps a verified file's calls between files only (ADR 0018): same
/// provenance, same verification, same content.
pub fn compacted(
    scope: &ScopeKey,
    file: &NodeId,
    content_hash: ContentHash,
    calls: &[Call],
) -> ScopeUpdate {
    let mut weights: BTreeMap<String, u32> = BTreeMap::new();
    for call in calls {
        *weights.entry(call.dst_file.clone()).or_default() += call.sites.max(1);
    }
    let observations = weights
        .into_iter()
        .filter_map(|(dst, weight)| {
            let dst = NodeId::file(&dst).ok()?;
            (dst != *file)
                .then(|| Observation::new(file.clone(), dst, EdgeKind::Calls).with_weight(weight))
        })
        .collect();
    ScopeUpdate::new(scope.clone()).with_observations(
        SourceObservations::new(Provenance::Lsp, observations)
            .verifying([EdgeKind::Calls])
            .computed_on(content_hash),
    )
}

/// Asks the server about each function of the job. `locate` maps a callee (file, position) to
/// the symbol that contains it; `yield_now` is checked between two functions.
pub fn refine(
    job: &RefineJob,
    server: &mut dyn LanguageServer,
    mut locate: impl FnMut(&str, Position) -> Option<NodeId>,
    yield_now: impl Fn() -> bool,
) -> Result<Outcome, LspError> {
    let mut calls: BTreeMap<(NodeId, NodeId), (String, u32)> = BTreeMap::new();
    for (function, at) in &job.functions {
        if yield_now() {
            return Ok(Outcome::Yielded);
        }
        for target in server.outgoing_calls(&job.path, *at)? {
            let Some(path) = &target.path else { continue };
            let Some(callee) = locate(path, target.at) else {
                continue;
            };
            let entry = calls
                .entry((function.clone(), callee))
                .or_insert_with(|| (path.clone(), 0));
            entry.1 += target.sites.max(1);
        }
    }
    let observations = calls
        .iter()
        .map(|((src, dst), (_, sites))| {
            Observation::new(src.clone(), dst.clone(), EdgeKind::Calls).with_weight(*sites)
        })
        .collect();
    let calls = calls
        .into_iter()
        .map(|((src, dst), (dst_file, sites))| Call {
            src,
            dst,
            dst_file,
            sites,
        })
        .collect();
    Ok(Outcome::Done(
        ScopeUpdate::new(job.scope.clone()).with_observations(
            SourceObservations::new(Provenance::Lsp, observations)
                .verifying([EdgeKind::Calls])
                .computed_on(job.content_hash),
        ),
        calls,
    ))
}

/// Files waiting for refinement, one entry per file, and the files already verified with the
/// priority they were asked at.
#[derive(Debug, Default)]
pub struct Queue {
    waiting: HashMap<String, (Priority, u64)>,
    order: u64,
    verified: HashMap<String, Priority>,
}

impl Queue {
    /// Asks for a file; a file already waiting keeps its place and takes the higher priority.
    pub fn request(&mut self, path: &str, priority: Priority) {
        self.order += 1;
        let order = self.order;
        self.waiting
            .entry(path.to_owned())
            .and_modify(|(p, _)| *p = (*p).max(priority))
            .or_insert((priority, order));
    }

    /// The next file: the highest priority, then the oldest request.
    pub fn pop(&mut self) -> Option<(String, Priority)> {
        let (path, (priority, _)) = self
            .waiting
            .iter()
            .max_by(|(_, (pa, oa)), (_, (pb, ob))| pa.cmp(pb).then(ob.cmp(oa)))
            .map(|(path, entry)| (path.clone(), *entry))?;
        self.waiting.remove(&path);
        Some((path, priority))
    }

    /// A job that yielded goes back first in its priority.
    pub fn put_back(&mut self, path: &str, priority: Priority) {
        self.waiting.insert(path.to_owned(), (priority, 0));
    }

    pub fn verified(&mut self, path: &str, priority: Priority) {
        let entry = self.verified.entry(path.to_owned()).or_insert(priority);
        *entry = (*entry).max(priority);
    }

    /// Files whose content changed: their verification is stale, ask again those that had one.
    pub fn changed<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) {
        for path in paths {
            if let Some(priority) = self.verified.get(path).copied() {
                self.request(path, priority);
            }
        }
    }

    /// Dependents of a signature change: verified ones come back at the lowest priority.
    pub fn dependents<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) {
        for path in paths {
            if self.verified.contains_key(path) {
                self.request(path, Priority::Dependent);
            }
        }
    }

    /// Forgets what was verified, keeping the files that wait: the store is read again.
    pub fn reset_verified(&mut self) {
        self.verified.clear();
    }

    /// Files verified so far, with the priority they were asked at.
    pub fn verified_paths(&self) -> impl Iterator<Item = (&String, Priority)> {
        self.verified
            .iter()
            .map(|(path, priority)| (path, *priority))
    }

    pub fn forget(&mut self, path: &str) {
        self.waiting.remove(path);
        self.verified.remove(path);
    }

    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_serves_priorities_then_age() {
        let mut queue = Queue::default();
        queue.request("dep.rs", Priority::Dependent);
        queue.request("open.rs", Priority::Opened);
        queue.request("shown.rs", Priority::Displayed);
        queue.request("open2.rs", Priority::Opened);
        // Asking again raises the priority but keeps the place.
        queue.request("dep.rs", Priority::Opened);
        queue.request("open.rs", Priority::Dependent);
        let order: Vec<String> = std::iter::from_fn(|| queue.pop().map(|(p, _)| p)).collect();
        assert_eq!(order, ["shown.rs", "dep.rs", "open.rs", "open2.rs"]);
        queue.put_back("a.rs", Priority::Opened);
        queue.request("b.rs", Priority::Opened);
        assert_eq!(queue.pop().unwrap().0, "a.rs");
    }

    #[test]
    fn only_verified_files_come_back() {
        let mut queue = Queue::default();
        queue.verified("open.rs", Priority::Opened);
        let changed = ["open.rs".to_owned(), "never.rs".to_owned()];
        queue.changed(&changed);
        queue.dependents(&changed);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop(), Some(("open.rs".into(), Priority::Opened)));
        queue.dependents(&changed);
        assert_eq!(queue.pop(), Some(("open.rs".into(), Priority::Dependent)));
        queue.forget("open.rs");
        queue.changed(&changed);
        assert!(queue.is_empty());
        // Reading the store again forgets the verified, not what waits.
        queue.verified("a.rs", Priority::Opened);
        queue.request("shown.rs", Priority::Displayed);
        queue.reset_verified();
        assert_eq!(queue.verified_paths().count(), 0);
        assert_eq!(queue.pop(), Some(("shown.rs".into(), Priority::Displayed)));
    }
}
