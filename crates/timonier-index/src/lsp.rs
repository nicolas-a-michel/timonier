//! Speed 2 (ADR 0014, 0022): the language-server interface, the refinement queue, and the
//! turning of call hierarchies and definitions into verified observations.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

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

/// Where a name is defined, as the language server says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    /// Its file, relative to the repository root; `None` outside the repository.
    pub path: Option<String>,
    /// The start of the defined name.
    pub at: Position,
}

/// A usage of a file to verify (ADR 0022): a cited path, asked about once at its first site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usage {
    /// `References`, `Implements` or `Inherits`.
    pub kind: EdgeKind,
    /// The file for `References`; the symbol that implements or inherits otherwise.
    pub source: NodeId,
    pub at: Position,
    pub sites: u32,
    /// Tree-sitter places its definition in another file of the repository.
    pub elsewhere: bool,
}

/// A name the language server is asked about (ADR 0022): where it is, and whether tree-sitter
/// places its definition in another file. A server that answers « here » for such a name may
/// not know yet: VSCode's TypeScript syntax server, while the project loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Question {
    pub at: Position,
    pub elsewhere: bool,
}

/// A symbol whose signature changed, or that appeared: the files that use it are its dependents,
/// and the language server says which they are (ADR 0022).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub path: String,
    /// Which symbol: what tells two lookups apart, whatever their positions.
    pub symbol: NodeId,
    /// Where its name starts.
    pub at: Position,
    /// The content the position was read in: once the file changed, the symbol is found again.
    pub content_hash: ContentHash,
    /// The files that cite its name: its dependents when the server cannot say.
    pub by_name: BTreeSet<String>,
}

/// What the refinement queue serves next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task {
    Refine(String, Priority),
    LookUp(Lookup),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LspError {
    /// No server reads this file: the job is dropped.
    #[error("no language server for `{0}`")]
    Unavailable(String),
    #[error("language server: {0}")]
    Failed(String),
    /// Other work came first while the server was answering: the job waits again.
    #[error("interrupted: other work came first")]
    Interrupted,
    /// The client's documents are not what is on disk (unsaved changes, or not read again yet):
    /// its servers would answer about other content. The job is held until they are
    /// (ADR 0022).
    #[error("the client's documents differ from the disk")]
    Unsaved,
}

/// What refinement asks a language server (ADR 0014). Positions are those of the model:
/// 0-based lines, UTF-8 byte columns; the implementation converts.
pub trait LanguageServer {
    /// `textDocument/prepareCallHierarchy` at `at`, then `callHierarchy/outgoingCalls`.
    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError>;

    /// Whether it can answer now. A relay without its client cannot: refinement then waits,
    /// and keeps its queue.
    fn ready(&self) -> bool {
        true
    }

    /// Whether another client answers since the last call (a relay): what one could not answer
    /// about saved content, the next may (ADR 0022).
    fn changed_hands(&mut self) -> bool {
        false
    }

    /// [`LanguageServer::outgoing_calls`], giving way to other work while it waits: a server
    /// that answers slowly returns [`LspError::Interrupted`] once `yield_now` says so.
    fn outgoing_calls_or_yield(
        &mut self,
        path: &str,
        at: Position,
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<CallTarget>, LspError> {
        let _ = yield_now;
        self.outgoing_calls(path, at)
    }

    /// `textDocument/definition` at each position of `path`, in one batch: where each name is
    /// defined, in the same order (ADR 0022). A server without it is `Unavailable`, and
    /// refinement verifies the calls alone.
    fn definitions(
        &mut self,
        path: &str,
        questions: &[Question],
    ) -> Result<Vec<Vec<Definition>>, LspError> {
        let _ = (path, questions);
        Err(LspError::Unavailable(
            "no definitions from this server".into(),
        ))
    }

    /// [`LanguageServer::definitions`], giving way to other work while it waits.
    fn definitions_or_yield(
        &mut self,
        path: &str,
        questions: &[Question],
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<Definition>>, LspError> {
        let _ = yield_now;
        self.definitions(path, questions)
    }

    /// `textDocument/references` at `at`: the files, relative to the repository root, that use
    /// the symbol named there (ADR 0022).
    fn references(&mut self, path: &str, at: Position) -> Result<Vec<String>, LspError> {
        let _ = (path, at);
        Err(LspError::Unavailable(
            "no references from this server".into(),
        ))
    }

    /// [`LanguageServer::references`], giving way to other work while it waits.
    fn references_or_yield(
        &mut self,
        path: &str,
        at: Position,
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<String>, LspError> {
        let _ = yield_now;
        self.references(path, at)
    }
}

/// Refinement priorities, served from the highest (ADR 0014).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Dependent,
    Opened,
    Displayed,
}

/// One file to refine: the content it was read with, its functions with the position of their
/// name, and its other usages (ADR 0022).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefineJob {
    pub path: String,
    pub scope: ScopeKey,
    pub content_hash: ContentHash,
    pub functions: Vec<(NodeId, Position)>,
    pub usages: Vec<Usage>,
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
            usages: self.usages(file),
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

/// What the language server verified in a file (ADR 0014, 0022).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verified {
    pub calls: Vec<Call>,
    /// Its `References`, `Implements` and `Inherits`, as observed: compaction keeps them.
    pub usages: Vec<Observation>,
    /// The kinds verified: `Calls`, and the others when the definitions answered.
    pub kinds: Vec<EdgeKind>,
    /// Why the other usages stay those of tree-sitter, when they do.
    pub unverified: Option<String>,
}

/// The kinds a file's definitions verify, besides its calls.
pub const USAGE_KINDS: [EdgeKind; 3] = [
    EdgeKind::References,
    EdgeKind::Implements,
    EdgeKind::Inherits,
];

/// A file refined: the update for the store, and what was verified, in detail.
#[derive(Debug, PartialEq)]
pub struct Refined {
    pub update: ScopeUpdate,
    pub verified: Verified,
    /// The file and those the answers point into: the answers hold only while the disk has the
    /// content indexed for each (ADR 0022). A file the daemon does not index (a dependency
    /// under the root, an ignored file) makes no edge, and the check leaves it out.
    pub read: BTreeSet<String>,
}

/// How a refinement ended.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Done(Box<Refined>),
    /// Other work came first; the job starts again later.
    Yielded,
}

/// The update that keeps a verified file's calls between files only (ADR 0018): same
/// provenance, same verification, same content.
pub fn compacted(
    scope: &ScopeKey,
    file: &NodeId,
    content_hash: ContentHash,
    verified: &Verified,
) -> ScopeUpdate {
    let mut weights: BTreeMap<String, u32> = BTreeMap::new();
    for call in &verified.calls {
        *weights.entry(call.dst_file.clone()).or_default() += call.sites.max(1);
    }
    let observations = weights
        .into_iter()
        .filter_map(|(dst, weight)| {
            let dst = NodeId::file(&dst).ok()?;
            (dst != *file)
                .then(|| Observation::new(file.clone(), dst, EdgeKind::Calls).with_weight(weight))
        })
        .chain(verified.usages.iter().cloned())
        .collect();
    ScopeUpdate::new(scope.clone()).with_observations(
        SourceObservations::new(Provenance::Lsp, observations)
            .verifying(verified.kinds.iter().copied())
            .computed_on(content_hash),
    )
}

/// Asks the server about each function of the job, then about its usages in one batch
/// (ADR 0022). `locate` maps a definition (file, position) to the symbol that contains it,
/// `indexed` says whether a file is in the model; `yield_now` is checked between questions.
pub fn refine(
    job: &RefineJob,
    server: &mut dyn LanguageServer,
    mut locate: impl FnMut(&str, Position) -> Option<NodeId>,
    indexed: impl Fn(&str) -> Option<NodeId>,
    yield_now: impl Fn() -> bool,
) -> Result<Outcome, LspError> {
    let mut calls: BTreeMap<(NodeId, NodeId), (String, u32)> = BTreeMap::new();
    let mut read = BTreeSet::from([job.path.clone()]);
    for (function, at) in &job.functions {
        if yield_now() {
            return Ok(Outcome::Yielded);
        }
        let targets = match server.outgoing_calls_or_yield(&job.path, *at, &yield_now) {
            Err(LspError::Interrupted) => return Ok(Outcome::Yielded),
            other => other?,
        };
        for target in targets {
            let Some(path) = &target.path else { continue };
            read.insert(path.clone());
            let Some(callee) = locate(path, target.at) else {
                continue;
            };
            let entry = calls
                .entry((function.clone(), callee))
                .or_insert_with(|| (path.clone(), 0));
            entry.1 += target.sites.max(1);
        }
    }
    let mut kinds = vec![EdgeKind::Calls];
    let mut unverified = None;
    let mut usages: BTreeMap<(NodeId, NodeId, EdgeKind), u32> = BTreeMap::new();
    if job.usages.is_empty() {
        // Nothing to ask: no usage of this file makes an edge to another one.
        kinds.extend(USAGE_KINDS);
    } else {
        if yield_now() {
            return Ok(Outcome::Yielded);
        }
        let questions: Vec<Question> = job
            .usages
            .iter()
            .map(|u| Question {
                at: u.at,
                elsewhere: u.elsewhere,
            })
            .collect();
        // A usage tree-sitter places in another file, that the server places in this one or
        // nowhere: the server is not sure yet. Tree-sitter keeps the usages of the file rather
        // than lose a right edge on a doubtful answer.
        let doubtful = |found: &[Vec<Definition>]| {
            job.usages.iter().zip(found).any(|(usage, definitions)| {
                usage.elsewhere
                    && definitions
                        .iter()
                        .all(|d| d.path.as_deref() == Some(job.path.as_str()))
            })
        };
        match server.definitions_or_yield(&job.path, &questions, &yield_now) {
            Err(LspError::Interrupted) => return Ok(Outcome::Yielded),
            Err(LspError::Unsaved) => return Err(LspError::Unsaved),
            // The calls alone, as before ADR 0022: tree-sitter keeps the other usages.
            Err(error @ (LspError::Unavailable(_) | LspError::Failed(_))) => {
                unverified = Some(error.to_string());
            }
            Ok(found) if found.len() != job.usages.len() => {
                unverified = Some(format!(
                    "{} definitions for {} usages",
                    found.len(),
                    job.usages.len()
                ));
            }
            Ok(found) if doubtful(&found) => {
                unverified = Some(
                    "a usage tree-sitter places in another file is placed nowhere else".into(),
                );
            }
            Ok(found) => {
                kinds.extend(USAGE_KINDS);
                read.extend(found.iter().flatten().filter_map(|d| d.path.clone()));
                for (usage, definitions) in job.usages.iter().zip(&found) {
                    let target = definitions.iter().find_map(|d| {
                        let path = d.path.as_deref()?;
                        match usage.kind {
                            // Between files, as tree-sitter sees them.
                            EdgeKind::References if path == job.path => None,
                            EdgeKind::References => indexed(path),
                            _ => locate(path, d.at),
                        }
                    });
                    if let Some(dst) = target
                        && dst != usage.source
                    {
                        *usages
                            .entry((usage.source.clone(), dst, usage.kind))
                            .or_default() += usage.sites.max(1);
                    }
                }
            }
        }
    }
    let usages: Vec<Observation> = usages
        .into_iter()
        .map(|((src, dst, kind), sites)| Observation::new(src, dst, kind).with_weight(sites))
        .collect();
    let observations = calls
        .iter()
        .map(|((src, dst), (_, sites))| {
            Observation::new(src.clone(), dst.clone(), EdgeKind::Calls).with_weight(*sites)
        })
        .chain(usages.iter().cloned())
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
    Ok(Outcome::Done(Box::new(Refined {
        update: ScopeUpdate::new(job.scope.clone()).with_observations(
            SourceObservations::new(Provenance::Lsp, observations)
                .verifying(kinds.iter().copied())
                .computed_on(job.content_hash),
        ),
        verified: Verified {
            calls,
            usages,
            kinds,
            unverified,
        },
        read,
    })))
}

/// Files waiting for refinement, one entry per file, the files already verified with the
/// priority they were asked at, and the lookups of dependents (ADR 0022). What the client could
/// not answer about saved content is held apart, until it can.
#[derive(Debug, Default)]
pub struct Queue {
    waiting: HashMap<String, (Priority, u64)>,
    order: u64,
    verified: HashMap<String, Priority>,
    /// Verified files whose current content holds nothing more to verify.
    whole: HashSet<String>,
    lookups: VecDeque<Lookup>,
    held: HashMap<String, Priority>,
    held_lookups: Vec<Lookup>,
    /// Verified files a changed signature made stale, not verified again yet: with the lookups,
    /// what a restart must not forget (review n° 9, I9-2).
    stale: BTreeSet<String>,
}

impl Queue {
    /// A client asks for a file (displayed, opened): nothing to do if its current content is
    /// verified for every kind. Returns whether it waits.
    pub fn ask(&mut self, path: &str, priority: Priority) -> bool {
        if self.whole.contains(path) {
            // Asked again later, it comes back at this priority.
            self.verified(path, priority, true);
            return false;
        }
        self.request(path, priority);
        true
    }

    /// Asks for a file; a file already waiting keeps its place and takes the higher priority.
    pub fn request(&mut self, path: &str, priority: Priority) {
        // Held, it waits again: asked now, it is no longer held (review n° 9, m9-1).
        let priority = match self.held.remove(path) {
            Some(held) => priority.max(held),
            None => priority,
        };
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

    /// Symbols whose dependents the language server should name (ADR 0022). Only a verified
    /// file that cites the name can come back: with none, there is nothing to look up. The
    /// lookup then only tells, among those, the ones that do not use this symbol.
    pub fn look_up(&mut self, lookups: impl IntoIterator<Item = Lookup>) {
        for lookup in lookups {
            if lookup
                .by_name
                .iter()
                .any(|path| self.verified.contains_key(path))
            {
                self.enqueue(lookup);
            }
        }
    }

    /// The same symbol again, at any position: the newer content, and the citers of both. One
    /// held for the same symbol waits again with it.
    fn enqueue(&mut self, mut lookup: Lookup) {
        let same = |l: &Lookup| l.path == lookup.path && l.symbol == lookup.symbol;
        if let Some(i) = self.held_lookups.iter().position(same) {
            let held = self.held_lookups.remove(i);
            lookup.by_name.extend(held.by_name);
        }
        match self
            .lookups
            .iter_mut()
            .find(|l| l.path == lookup.path && l.symbol == lookup.symbol)
        {
            Some(waiting) => {
                let mut by_name = std::mem::take(&mut waiting.by_name);
                by_name.extend(lookup.by_name.iter().cloned());
                *waiting = Lookup { by_name, ..lookup };
            }
            None => self.lookups.push_back(lookup),
        }
    }

    /// A lookup that yielded comes back first.
    pub fn put_back_lookup(&mut self, lookup: Lookup) {
        self.lookups.push_front(lookup);
    }

    /// A file the client could not answer about saved content: it waits apart (ADR 0022).
    pub fn hold(&mut self, path: &str, priority: Priority) {
        let entry = self.held.entry(path.to_owned()).or_insert(priority);
        *entry = (*entry).max(priority);
    }

    pub fn hold_lookup(&mut self, lookup: Lookup) {
        self.held_lookups.push(lookup);
    }

    /// The client's documents are saved again, another client relays, or files changed on
    /// disk: what was held waits again.
    pub fn release(&mut self) {
        for (path, priority) in std::mem::take(&mut self.held) {
            self.request(&path, priority);
        }
        for lookup in std::mem::take(&mut self.held_lookups) {
            self.enqueue(lookup);
        }
    }

    /// Files and lookups held until the client's documents are saved.
    pub fn held(&self) -> usize {
        self.held.len() + self.held_lookups.len()
    }

    /// The next task: what is displayed or opened, then the lookups of dependents, then the
    /// dependents themselves. A lookup goes before them, since it says which ones to verify.
    pub fn next_task(&mut self) -> Option<Task> {
        let urgent = self
            .waiting
            .values()
            .any(|(priority, _)| *priority > Priority::Dependent);
        if !urgent && let Some(lookup) = self.lookups.pop_front() {
            return Some(Task::LookUp(lookup));
        }
        self.pop()
            .map(|(path, priority)| Task::Refine(path, priority))
    }

    /// A file verified at `priority`; `whole` when every kind is, its other usages included.
    pub fn verified(&mut self, path: &str, priority: Priority, whole: bool) {
        self.stale.remove(path);
        self.held.remove(path);
        let entry = self.verified.entry(path.to_owned()).or_insert(priority);
        *entry = (*entry).max(priority);
        if whole {
            self.whole.insert(path.to_owned());
        } else {
            self.whole.remove(path);
        }
    }

    /// A verified file with something left to verify: its detail, lost to compaction
    /// (ADR 0018), or its verification, dropped. A client asking for it gets it back.
    pub fn incomplete(&mut self, path: &str) {
        self.whole.remove(path);
    }

    /// Files whose content changed: their verification is stale, ask again those that had one.
    pub fn changed<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) {
        for path in paths {
            self.whole.remove(path);
            // Its verification went with its content: nothing stale is left to show.
            self.stale.remove(path);
            if let Some(priority) = self.verified.get(path).copied() {
                self.request(path, priority);
            }
        }
    }

    /// Dependents of a signature change: verified ones come back at the lowest priority. Returns
    /// how many came back.
    pub fn dependents<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) -> usize {
        let mut back = 0;
        for path in paths {
            if self.verified.contains_key(path) {
                self.whole.remove(path);
                self.stale.insert(path.clone());
                self.request(path, Priority::Dependent);
                back += 1;
            }
        }
        back
    }

    /// Forgets what was verified, keeping the files that wait: the store is read again.
    pub fn reset_verified(&mut self) {
        self.verified.clear();
        self.whole.clear();
        self.stale.clear();
    }

    /// The verified files made stale, taken out: the store is read again, and they are owed
    /// again after (review n° 12, m12-1).
    pub fn take_stale(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.stale)
    }

    /// What the refinement owes (ADR 0022): the verified files made stale, and the lookups
    /// waiting or held, sorted.
    pub fn debt(&self) -> (Vec<String>, Vec<Lookup>) {
        let mut lookups: Vec<Lookup> = self
            .lookups
            .iter()
            .chain(&self.held_lookups)
            .cloned()
            .collect();
        lookups.sort_by(|a, b| (&a.path, &a.symbol).cmp(&(&b.path, &b.symbol)));
        (self.stale.iter().cloned().collect(), lookups)
    }

    /// What a previous run owed, after a restart: the files still verified come back as
    /// dependents, and the lookups wait again.
    pub fn owe(&mut self, dependents: impl IntoIterator<Item = String>, lookups: Vec<Lookup>) {
        let stale: Vec<String> = dependents
            .into_iter()
            .filter(|path| self.verified.contains_key(path))
            .collect();
        self.dependents(&stale);
        self.look_up(lookups);
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
        self.whole.remove(path);
        self.held.remove(path);
        self.stale.remove(path);
        self.lookups.retain(|l| l.path != path);
        self.held_lookups.retain(|l| l.path != path);
    }

    /// Files waiting for refinement, and lookups of dependents; not those held.
    pub fn len(&self) -> usize {
        self.waiting.len() + self.lookups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty() && self.lookups.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(symbol: &str, line: u32, hash: u8, by_name: &[&str]) -> Lookup {
        Lookup {
            path: "a.rs".into(),
            symbol: NodeId::parse(format!("rust:a::{symbol}")).unwrap(),
            at: Position::new(line, 7),
            content_hash: ContentHash::from_bytes([hash; 32]),
            by_name: by_name.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    /// ADR 0022: a lookup only when a verified file cites the name; the same symbol asked twice
    /// is one lookup, with the newer content and the citers of both (review n° 4, I-A). Another
    /// symbol is another lookup, even at the same position (review n° 5, I-β).
    #[test]
    fn lookups_wait_only_for_what_can_come_back() {
        let mut queue = Queue::default();
        queue.look_up([lookup("helper", 0, 1, &["b.rs"])]);
        assert!(queue.is_empty(), "no verified file cites the name");
        queue.verified("b.rs", Priority::Opened, true);
        queue.verified("c.rs", Priority::Opened, true);
        queue.look_up([lookup("helper", 0, 1, &["b.rs"])]);
        queue.look_up([
            lookup("helper", 1, 2, &["c.rs"]),
            lookup("other", 3, 2, &["x.rs"]),
            // Where `helper` was, in the newer content.
            lookup("assist", 0, 2, &["c.rs"]),
        ]);
        let mut tasks = Vec::new();
        while let Some(Task::LookUp(lookup)) = queue.next_task() {
            tasks.push(lookup);
        }
        assert_eq!(tasks.len(), 2, "{tasks:?}");
        assert_eq!(tasks[0].symbol.as_str(), "rust:a::helper");
        assert_eq!(tasks[0].at, Position::new(1, 7));
        assert_eq!(tasks[0].content_hash, ContentHash::from_bytes([2; 32]));
        assert_eq!(
            tasks[0].by_name,
            ["b.rs".to_owned(), "c.rs".to_owned()].into()
        );
        assert_eq!(tasks[1].symbol.as_str(), "rust:a::assist");
    }

    /// ADR 0022: what the client could not answer about saved content waits apart, outside the
    /// count that keeps the pipeline busy, until it is released.
    #[test]
    fn held_tasks_wait_apart_until_released() {
        let mut queue = Queue::default();
        queue.verified("b.rs", Priority::Opened, true);
        queue.hold("a.rs", Priority::Dependent);
        queue.hold("a.rs", Priority::Displayed);
        queue.hold_lookup(lookup("helper", 0, 1, &["b.rs"]));
        assert!(queue.is_empty());
        assert_eq!(queue.held(), 2);
        assert_eq!(queue.next_task(), None);
        queue.release();
        assert_eq!(queue.held(), 0);
        assert_eq!(
            queue.next_task(),
            Some(Task::Refine("a.rs".into(), Priority::Displayed))
        );
        assert!(matches!(queue.next_task(), Some(Task::LookUp(_))));
        queue.hold("gone.rs", Priority::Opened);
        queue.forget("gone.rs");
        assert_eq!(queue.held(), 0);
    }

    /// Review n° 9, m9-1: a file asked for, or verified, while held is held no longer; a lookup
    /// of the same symbol takes along the one held.
    #[test]
    fn what_is_asked_again_is_held_no_longer() {
        let mut queue = Queue::default();
        queue.verified("b.rs", Priority::Opened, true);
        queue.hold("a.rs", Priority::Displayed);
        queue.request("a.rs", Priority::Dependent);
        assert_eq!(queue.held(), 0);
        assert_eq!(queue.pop(), Some(("a.rs".into(), Priority::Displayed)));
        queue.hold("a.rs", Priority::Opened);
        queue.verified("a.rs", Priority::Opened, true);
        assert_eq!(queue.held(), 0);
        // Each cites a file the other does not: the merged lookup cites them all (D16).
        queue.hold_lookup(lookup("helper", 0, 1, &["b.rs", "x.rs"]));
        queue.look_up([lookup("helper", 1, 2, &["b.rs", "c.rs"])]);
        assert_eq!(queue.held(), 0);
        match queue.next_task() {
            Some(Task::LookUp(merged)) => {
                assert_eq!(merged.at, Position::new(1, 7));
                assert_eq!(
                    merged.by_name,
                    ["b.rs".to_owned(), "c.rs".to_owned(), "x.rs".to_owned()].into()
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// Review n° 5, m-β: a client asking for a file verified whole, unchanged since, asks for
    /// nothing; a file whose usages stayed tree-sitter's, changed, or depending on a changed
    /// signature, is refined again.
    #[test]
    fn a_file_verified_whole_is_not_asked_again() {
        let mut queue = Queue::default();
        queue.verified("whole.rs", Priority::Dependent, true);
        queue.verified("calls.rs", Priority::Opened, false);
        assert!(!queue.ask("whole.rs", Priority::Displayed));
        assert!(queue.ask("calls.rs", Priority::Displayed));
        assert_eq!(queue.pop(), Some(("calls.rs".into(), Priority::Displayed)));
        assert!(queue.is_empty());
        // Asked at a higher priority, it comes back at that one when it changes.
        queue.changed(&["whole.rs".to_owned()]);
        assert_eq!(queue.pop(), Some(("whole.rs".into(), Priority::Displayed)));
        assert!(
            queue.ask("whole.rs", Priority::Opened),
            "changed: not whole"
        );
        queue.pop();
        queue.verified("whole.rs", Priority::Opened, true);
        queue.dependents(&["whole.rs".to_owned()]);
        queue.pop();
        assert!(
            queue.ask("whole.rs", Priority::Opened),
            "a dependent: not whole"
        );
    }

    /// ADR 0018, 0022: compaction reduces the calls to calls between files, and keeps the other
    /// usages and the kinds verified as they are.
    #[test]
    fn compaction_keeps_the_usages_and_what_was_verified() {
        let id = |s: &str| NodeId::parse(s).unwrap();
        let file = id("file:src/a.rs");
        let scope = ScopeKey::of_file(&file).unwrap();
        let usage = Observation::new(file.clone(), id("file:src/c.rs"), EdgeKind::References)
            .with_weight(3);
        let verified = Verified {
            calls: vec![Call {
                src: id("rust:a::f"),
                dst: id("rust:b::g"),
                dst_file: "src/b.rs".into(),
                sites: 2,
            }],
            usages: vec![usage.clone()],
            kinds: vec![EdgeKind::Calls, EdgeKind::References],
            unverified: None,
        };
        let update = compacted(&scope, &file, ContentHash::from_bytes([7; 32]), &verified);
        let observations = update.observations.unwrap();
        assert_eq!(
            observations.verified,
            [EdgeKind::Calls, EdgeKind::References]
        );
        assert_eq!(
            observations.observations,
            [
                Observation::new(file, id("file:src/b.rs"), EdgeKind::Calls).with_weight(2),
                usage
            ]
        );
    }

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
        queue.verified("open.rs", Priority::Opened, true);
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
        queue.verified("a.rs", Priority::Opened, true);
        queue.request("shown.rs", Priority::Displayed);
        queue.reset_verified();
        assert!(queue.ask("a.rs", Priority::Opened));
        assert_eq!(queue.verified_paths().count(), 0);
        assert_eq!(queue.pop(), Some(("shown.rs".into(), Priority::Displayed)));
        assert_eq!(queue.pop(), Some(("a.rs".into(), Priority::Opened)));
    }
}
