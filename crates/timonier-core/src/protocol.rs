//! The protocol between the daemon and its clients (ADR 0019): JSON-RPC 2.0 messages framed by
//! `Content-Length`, whose params and results are the types below. The TypeScript side is
//! generated from them (`webview/src/protocol/generated.ts`, checked by a test).
//!
//! Model types travel as their JSON form: ids as strings, kinds as their names.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::intent::Intent;
use crate::tracking::{
    ChangedBox, ChangedLink, Plan, Session, SessionChanges, SessionStatus, TaskStatus, Todo,
    TodoItem, TodoStatus,
};
use crate::{
    AggregatedEdge, EdgeKind, Language, NodeId, NodeKind, Position, Provenance, Role, Snapshot,
    Span,
};

/// Bumped when a change breaks clients; a client with another version is refused.
pub const PROTOCOL_VERSION: u32 = 9;

/// Method names (ADR 0019).
pub mod methods {
    pub const INITIALIZE: &str = "initialize";
    pub const STATUS: &str = "daemon/status";
    pub const SHUTDOWN: &str = "daemon/shutdown";
    pub const SUBSCRIBE: &str = "view/subscribe";
    /// A notification from the daemon: one delta.
    pub const UPDATE: &str = "view/update";
    pub const RESYNC: &str = "view/resync";
    pub const UNSUBSCRIBE: &str = "view/unsubscribe";
    /// Notifications from the client: files opened in the editor, files shown on the map.
    pub const OPENED: &str = "editor/opened";
    pub const DISPLAYED: &str = "view/displayed";
    /// A notification from the client that relays: its documents of the repository are what is
    /// on disk again, saved or read again. What it refused meanwhile can be asked (ADR 0022).
    pub const SAVED: &str = "editor/saved";
    /// A request from the daemon to the client that relays the IDE's language servers.
    pub const OUTGOING_CALLS: &str = "lsp/outgoingCalls";
    pub const DEFINITIONS: &str = "lsp/definitions";
    pub const REFERENCES: &str = "lsp/references";
    /// A request from the client: the head is marked as reviewed (ADR 0024).
    pub const REVIEW_MARK: &str = "review/mark";
    /// Requests from a client: the MCP tools, as MCP lists and calls them (ADR 0027).
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";
    /// The plan, the todo and the journal, whole, then again at each change (ADR 0031).
    pub const TRACKING_SUBSCRIBE: &str = "tracking/subscribe";
    pub const TRACKING_UPDATE: &str = "tracking/update";
    /// Whether an agent that wants to stop should be held once, and why (ADR 0033, Q49).
    pub const AGENT_STOP: &str = "agent/stop";
    /// Writes `.timonier/actual.json` from the head (ADR 0033, Q50).
    pub const ACTUAL_WRITE: &str = "actual/write";
    /// Reads `.timonier/intent.json`, the whiteboard (ADR 0039).
    pub const INTENT_READ: &str = "intent/read";
    /// Writes it, after a gesture of the developer on the whiteboard (Q66). The extension's
    /// method, never an agent's: the whiteboard has a single author.
    pub const INTENT_WRITE: &str = "intent/write";
    /// How the code drifts from the whiteboard, for the map (ADR 0042).
    pub const INTENT_DRIFT: &str = "intent/drift";
    /// A run of tests begins: its number, its snapshot, where its traces go (ADR 0047).
    pub const RUN_START: &str = "run/start";
    /// What `timonier test` measured of it, to keep (ADR 0043, 0045).
    pub const RUN_FINISH: &str = "run/finish";
    /// The performance views (ADR 0051): the last runs, one run, the map's layer.
    pub const PERF_RUNS: &str = "perf/runs";
    pub const PERF_RUN: &str = "perf/run";
    pub const PERF_MAP: &str = "perf/map";
    /// The whiteboard's budgets checked on the last runs (ADR 0052); `null` without budgets.
    pub const PERF_BUDGETS: &str = "perf/budgets";
    /// A batch of runs against the main branch (ADR 0053).
    pub const PERF_REGRESSIONS: &str = "perf/regressions";
    /// The technology inventory (ADR 0055): `get_tech_inventory`'s answer.
    pub const INVENTORY: &str = "inventory/get";
    /// The inventory as a CycloneDX 1.5 SBOM, validated (ADR 0055).
    pub const INVENTORY_SBOM: &str = "inventory/sbom";
    /// The development process (ADR 0056): `.timonier/workflow.json`, Solo rapide without one.
    pub const WORKFLOW_GET: &str = "workflow/get";
    /// Its settings, from the panel; checked, the daemon writes the file.
    pub const WORKFLOW_SET: &str = "workflow/set";
}

/// Error codes of the daemon, after JSON-RPC's own (ADR 0019).
pub mod errors {
    pub const PARSE: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    /// `initialize` first.
    pub const NOT_INITIALIZED: i64 = -32000;
    /// Another major version of the protocol.
    pub const VERSION_REFUSED: i64 = -32001;
    pub const UNKNOWN_SUBSCRIPTION: i64 = -32002;
    /// The daemon is shutting down.
    pub const SHUTTING_DOWN: i64 = -32003;
    /// From the relaying client, as in LSP: no server, or none ready.
    pub const REQUEST_FAILED: i64 = -32803;
    /// From the relaying client, as in LSP: its documents are not what is on disk, so its
    /// servers would answer about other content (ADR 0022).
    pub const CONTENT_MODIFIED: i64 = -32801;
}

const NODE_KINDS: &str = "\"repo\" | \"component\" | \"package\" | \"file\" | \"module\" | \"type\" | \"trait\" | \"function\" | \"method\" | \"field\" | \"constant\"";
const ROLES: &str = "\"entry_point\" | \"data_model\" | \"persistence\" | \"interface\" | \"background_task\" | \"external_libraries\" | \"utility\" | \"project\"";
const EDGE_KINDS: &str =
    "\"imports\" | \"calls\" | \"references\" | \"implements\" | \"inherits\" | \"depends_on\"";
const PROVENANCES: &str =
    "\"tree_sitter\" | \"lsp\" | \"scip\" | \"manifest\" | \"inferred\" | \"intended\"";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    /// Who connects, for the logs: `vscode 1.142.0`, `timonier-cli 0.1.0`.
    pub client: String,
    /// The client relays its IDE's language servers (`lsp/outgoingCalls`, ADR 0014).
    #[serde(default)]
    pub lsp_relay: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub daemon_version: String,
    /// The repository the daemon indexes, canonical.
    pub root: String,
    /// The file extensions its packs read (`rs`, `ts`…): a relaying client answers only while
    /// none of those files has unsaved changes (ADR 0022).
    pub source_extensions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StatusResult {
    pub root: String,
    pub daemon_version: String,
    /// Files of the repository in the graph.
    pub files: u32,
    /// The first indexing is done: the graph holds the whole repository (ADR 0033).
    pub indexed: bool,
    /// The open snapshot every change goes to (ADR 0010).
    #[ts(type = "number")]
    pub head: u64,
    /// Files waiting for LSP refinement (ADR 0014), and lookups of dependents (ADR 0022).
    pub refinements_waiting: u32,
    /// Those held until the relaying client's documents are saved (ADR 0022).
    pub refinements_held: u32,
    pub clients: u32,
    /// Questions the relaying clients answered since the start (ADR 0022).
    pub lsp_answers: LspAnswers,
    /// Lookups of dependents since the start (ADR 0022).
    pub lookups: LookupCounts,
    /// The last review mark; `null` before the first review (ADR 0024).
    pub last_review: Option<ReviewMark>,
    /// How far the first indexing is, until `indexed` (T-115): for the start's screens.
    pub progress: Option<IndexProgress>,
}

/// An agent `timonier init` configures, as `init --detect` says it (T-118).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AgentChoice {
    pub key: String,
    pub label: String,
    pub found: bool,
    pub why: Option<String>,
}

/// The files the first indexing reads, and those read so far (T-115).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub done: u32,
    pub total: u32,
}

/// A review mark: the snapshot the review diff starts from (ADR 0024).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMark {
    #[ts(type = "number")]
    pub seq: u64,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub timestamp_ms: i64,
}

impl From<&Snapshot> for ReviewMark {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            seq: snapshot.seq,
            timestamp_ms: snapshot.timestamp_ms,
        }
    }
}

/// Lookups of the dependents of changed signatures, by how they ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LookupCounts {
    /// The language server named the users.
    pub named: u32,
    /// By the files citing the name: the server could not say, or the symbol is gone from the
    /// file changed since.
    pub by_name: u32,
}

/// Questions answered by the language servers of relaying clients, by method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LspAnswers {
    pub outgoing_calls: u32,
    pub definitions: u32,
    pub references: u32,
    /// Questions the client could not answer: no server for the file, or none ready.
    pub failed: u32,
}

/// What a view shows: the top of the map, and the inside of each expanded node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewSpec {
    #[ts(type = "string[]")]
    pub expanded: Vec<NodeId>,
}

/// `tools/call` (ADR 0027): a tool and its arguments, as MCP gives them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeParams {
    pub view: ViewSpec,
}

/// A subscription, for `view/resync` and `view/unsubscribe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionParams {
    #[ts(type = "number")]
    pub subscription: u64,
}

/// A node on the map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    #[ts(type = "string")]
    pub id: NodeId,
    #[ts(type = "NodeKind")]
    pub kind: NodeKind,
    pub name: String,
    /// Its parent on the map; `null` at the top.
    #[ts(type = "string | null")]
    pub parent: Option<NodeId>,
    #[ts(type = "Role | null")]
    pub role: Option<Role>,
    #[ts(type = "string | null")]
    pub language: Option<Language>,
    /// Children it can be expanded into.
    pub children: u32,
    /// The file to open, relative to the root: the file itself, or the symbol's.
    pub file: Option<String>,
    #[ts(type = "Span | null")]
    pub span: Option<Span>,
    /// A symbol's normalised signature, as its language pack writes it: a function's
    /// parameters and result, a field's or constant's type; `null` for the others (T-114).
    pub signature: Option<String>,
    /// What changed in it since the last review; `null` without change, and before the first
    /// review (ADR 0024).
    pub review: Option<NodeReview>,
    /// Its review debt: files written since the last review and not read since (ADR 0030);
    /// `null` without any.
    pub debt: Option<NodeDebt>,
}

/// The review debt of a box or a file (ADR 0030): of its `of` files, `files` changed since the
/// last review, in sessions or not, and `untracked` of them changed with no session open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NodeDebt {
    pub files: u32,
    pub of: u32,
    pub untracked: u32,
}

/// What changed in a box or a file since the last review (ADR 0024).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ReviewChange {
    Added,
    Modified,
}

/// A fan-in or a fan-out that jumped: at the review, and now (ADR 0024).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FanJump {
    pub before: u32,
    pub after: u32,
}

/// The review diff on a node of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NodeReview {
    pub change: Option<ReviewChange>,
    pub fan_in: Option<FanJump>,
    pub fan_out: Option<FanJump>,
    /// In a cycle added since the review.
    pub cycle: bool,
}

/// The review diff on an edge of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EdgeReview {
    /// No edge linked these two nodes at the review, and a change of the code explains it.
    pub added: bool,
    /// Between two boxes of a cycle added since the review.
    pub cycle: bool,
}

/// A box, as the review summary names it: one gone from the map too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBox {
    #[ts(type = "string")]
    pub id: NodeId,
    #[ts(type = "NodeKind")]
    pub kind: NodeKind,
    pub name: String,
    #[ts(type = "Role | null")]
    pub role: Option<Role>,
    /// The name of the box that holds it, to tell apart two of the same name; `null` at the
    /// top.
    pub within: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDependency {
    pub src: ReviewBox,
    pub dst: ReviewBox,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFan {
    pub item: ReviewBox,
    pub fan_in: Option<FanJump>,
    pub fan_out: Option<FanJump>,
}

/// The review diff, whatever the zoom (ADR 0024): what the summary lists. What is gone from the
/// map is only here (Q26).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    /// The review the diff starts from; `null` before the first one: no diff then (Q27).
    pub since: Option<ReviewMark>,
    pub added: Vec<ReviewBox>,
    pub modified: Vec<ReviewBox>,
    pub removed: Vec<ReviewBox>,
    pub dependencies_added: Vec<ReviewDependency>,
    pub dependencies_removed: Vec<ReviewDependency>,
    pub cycles: Vec<Vec<ReviewBox>>,
    pub fans: Vec<ReviewFan>,
}

/// An edge between two nodes of the map, standing for every edge between their contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewEdge {
    #[ts(type = "string")]
    pub src: NodeId,
    #[ts(type = "string")]
    pub dst: NodeId,
    #[ts(type = "EdgeKind")]
    pub kind: EdgeKind,
    #[ts(type = "number")]
    pub weight: u64,
    /// How many stored edges it stands for.
    pub edges: u32,
    /// The most reliable provenance among them.
    #[ts(type = "Provenance")]
    pub best: Provenance,
    /// Tree-sitter alone: drawn differently (spec « Displayed precision »).
    pub approximate_only: bool,
    /// What changed since the last review; `null` without change (ADR 0024).
    pub review: Option<EdgeReview>,
}

impl From<AggregatedEdge> for ViewEdge {
    fn from(edge: AggregatedEdge) -> Self {
        Self {
            src: edge.src,
            dst: edge.dst,
            kind: edge.kind,
            weight: edge.weight,
            edges: edge.edges,
            best: edge.best,
            approximate_only: edge.approximate_only,
            review: None,
        }
    }
}

impl ViewEdge {
    pub fn key(&self) -> EdgeRef {
        EdgeRef {
            src: self.src.clone(),
            dst: self.dst.clone(),
            kind: self.kind,
        }
    }
}

/// An edge of the map, by its endpoints and kind.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRef {
    #[ts(type = "string")]
    pub src: NodeId,
    #[ts(type = "string")]
    pub dst: NodeId,
    #[ts(type = "EdgeKind")]
    pub kind: EdgeKind,
}

/// A whole view at a revision: what `view/subscribe` and `view/resync` answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewSnapshot {
    #[ts(type = "number")]
    pub subscription: u64,
    #[ts(type = "number")]
    pub revision: u64,
    pub nodes: Vec<ViewNode>,
    pub edges: Vec<ViewEdge>,
    pub review: ReviewSummary,
}

/// From revision `base` to `revision` (`base + 1`). A client whose last revision is not `base`
/// missed a delta: it asks for `view/resync`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewDelta {
    #[ts(type = "number")]
    pub subscription: u64,
    #[ts(type = "number")]
    pub base: u64,
    #[ts(type = "number")]
    pub revision: u64,
    /// Nodes added or changed.
    pub nodes: Vec<ViewNode>,
    #[ts(type = "string[]")]
    pub removed_nodes: Vec<NodeId>,
    /// Edges added or changed.
    pub edges: Vec<ViewEdge>,
    pub removed_edges: Vec<EdgeRef>,
    /// The review summary, when it changed.
    pub review: Option<ReviewSummary>,
}

impl ViewDelta {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.removed_nodes.is_empty()
            && self.edges.is_empty()
            && self.removed_edges.is_empty()
            && self.review.is_none()
    }
}

/// The project's tracking as the panels show it (ADR 0031): what `tracking/subscribe` answers
/// and `tracking/update` sends again, whole, at each change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TrackingSnapshot {
    #[ts(type = "number")]
    pub subscription: u64,
    #[ts(type = "number")]
    pub revision: u64,
    pub tracking: TrackingView,
}

/// The plan, the todo of the open session and the journal, read in `.timonier/`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TrackingView {
    pub milestones: Vec<MilestoneView>,
    /// The open session's id; `null` between sessions.
    pub open: Option<String>,
    pub todo: Vec<TodoItemView>,
    /// The latest sessions first, at most [`JOURNAL_LIMIT`].
    pub sessions: Vec<SessionView>,
    /// How many sessions the journal holds in all.
    pub sessions_total: u32,
    /// The files that do not read: what they hold is not shown.
    pub problems: Vec<TrackingProblem>,
    /// The review diff since the last review, for the bridge (ADR 0032).
    pub review: ReviewSummary,
    /// The review debt of the whole project; `null` when nothing is to read again.
    pub debt: Option<NodeDebt>,
    /// The sessions started since the last review; all of them before the first.
    pub sessions_since_review: u32,
    /// What to look at first (Q45), at most [`LOOK_LIMIT`].
    pub look: Vec<LookItem>,
    /// `timonier init` set the project up (`.timonier/init.json`); else the bridge offers it
    /// (Q52).
    pub configured: bool,
}

/// `agent/stop`'s params: who stops (ADR 0058). Both optional: an older hook sends none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStopParams {
    /// The agent, as its hook names it (`claude`, `copilot`, `codex`).
    #[serde(default)]
    pub agent: Option<String>,
    /// The agent's own session (Claude Code's `session_id`).
    #[serde(default)]
    pub agent_session: Option<String>,
}

/// `agent/stop`: a reason to hold the agent once, or none (ADR 0033, Q49).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentStop {
    pub reason: Option<String>,
}

/// `actual/write`: where the file is, and whether it changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActualWritten {
    pub path: String,
    pub changed: bool,
}

/// What `intent/read` finds: the whiteboard, none yet, or the reason it does not read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IntentRead {
    pub intent: Option<Intent>,
    pub problem: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IntentWriteParams {
    pub intent: Intent,
}

/// What `intent/write` wrote: the whiteboard in its one order, and whether the file changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IntentWritten {
    pub intent: Intent,
    pub changed: bool,
}

/// How many things the bridge proposes to look at (Q45).
pub const LOOK_LIMIT: usize = 5;

/// Something to look at, as the bridge ranks them (ADR 0032, Q45). A box carries its path from
/// the top of the map (its ancestors, the repository left out), for the map to reveal it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LookItem {
    /// A file of `.timonier/` that does not read.
    Problem { file: String },
    /// A task of the plan, blocked.
    Blocked { task: String, title: String },
    /// A cycle added since the review; the path leads to its first box.
    Cycle {
        boxes: Vec<ReviewBox>,
        path: Vec<String>,
    },
    /// A box with files to read again.
    Debt {
        item: ReviewBox,
        debt: NodeDebt,
        path: Vec<String>,
    },
}

/// How many sessions the panels receive (ADR 0031).
pub const JOURNAL_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneView {
    pub id: String,
    pub title: String,
    /// Tasks done, over tasks not dropped (ADR 0029).
    pub done: u32,
    pub of: u32,
    pub tasks: Vec<TaskView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub criterion: Option<String>,
    pub status: TaskState,
    pub sessions: Vec<String>,
    pub notes: Vec<NoteView>,
    /// The tokens of its sessions; `null`: not measured (ADR 0058).
    pub usage: Option<TaskUsage>,
}

/// A task's tokens (ADR 0058): the sum of its sessions'.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskUsage {
    pub tokens: crate::usage::TokenUsage,
    /// A session of it served other tasks too: their tokens are in each one's sum.
    pub shared: bool,
}

/// The states of a task (ADR 0029).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Todo,
    InProgress,
    Blocked,
    Done,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    /// UTC, RFC 3339.
    pub at: String,
    pub session: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TodoItemView {
    pub id: String,
    pub text: String,
    pub task: Option<String>,
    pub done: bool,
}

/// A session of the journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub agent: String,
    pub objective: String,
    pub tasks: Vec<String>,
    pub open: bool,
    /// UTC, RFC 3339.
    pub started_at: String,
    pub ended_at: Option<String>,
    pub summary: Option<String>,
    /// Its todo: the live one while it is open.
    pub todo: Vec<TodoItemView>,
    /// What it changed in the code: since its start while it is open; `null` for a session
    /// that ended without it.
    pub changes: Option<SessionChanges>,
    /// The budgets the last runs exceeded when it ended (ADR 0052).
    pub budgets: Vec<String>,
    /// The tokens its agent spent; `null`: not measured (ADR 0058).
    pub usage: Option<crate::usage::TokenUsage>,
}

/// A tracking file that does not read (ADR 0029).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TrackingProblem {
    /// Relative to `.timonier/`.
    pub file: String,
    pub message: String,
}

impl TrackingView {
    /// The view of what the files hold. `sessions` in any order; `live` is the open session's
    /// changes since its start.
    pub fn new(
        plan: Option<&Plan>,
        todo: Option<&Todo>,
        mut sessions: Vec<Session>,
        live: Option<SessionChanges>,
        problems: Vec<TrackingProblem>,
    ) -> Self {
        let item = |item: &TodoItem| TodoItemView {
            id: item.id.clone(),
            text: item.text.clone(),
            task: item.task.clone(),
            done: item.status == TodoStatus::Done,
        };
        let todo_items: Vec<TodoItemView> =
            todo.map_or_else(Vec::new, |t| t.items.iter().map(item).collect());
        let open = sessions
            .iter()
            .find(|s| {
                s.status == SessionStatus::Open
                    && todo.and_then(|t| t.session.as_ref()) == Some(&s.id)
            })
            .map(|s| s.id.clone());
        // The latest first: ids sort by date, then by number.
        sessions.sort_by(|a, b| session_order(b).cmp(&session_order(a)));
        let total = u32::try_from(sessions.len()).unwrap_or(u32::MAX);
        // Each task's tokens, from every session of the journal (ADR 0058).
        let mut task_usage: std::collections::HashMap<&str, TaskUsage> =
            std::collections::HashMap::new();
        for session in &sessions {
            let Some(usage) = &session.usage else {
                continue;
            };
            for task in &session.tasks {
                let entry = task_usage
                    .entry(task.as_str())
                    .or_insert_with(|| TaskUsage {
                        tokens: crate::usage::TokenUsage::default(),
                        shared: false,
                    });
                entry.tokens.add(usage);
                entry.shared |= session.tasks.len() > 1;
            }
        }
        let task_usage: std::collections::HashMap<String, TaskUsage> = task_usage
            .into_iter()
            .map(|(task, usage)| (task.to_owned(), usage))
            .collect();
        sessions.truncate(JOURNAL_LIMIT);
        let mut live = live;
        let sessions = sessions
            .into_iter()
            .map(|session| {
                let is_open = open.as_deref() == Some(session.id.as_str());
                SessionView {
                    todo: if is_open {
                        todo_items.clone()
                    } else {
                        session.todo.iter().map(item).collect()
                    },
                    changes: if is_open {
                        live.take()
                    } else {
                        session.changes
                    },
                    id: session.id,
                    agent: session.agent,
                    objective: session.objective,
                    tasks: session.tasks,
                    open: is_open,
                    started_at: session.started_at,
                    ended_at: session.ended_at,
                    summary: session.summary,
                    budgets: session.budgets,
                    usage: session.usage,
                }
            })
            .collect();
        let milestones = plan.map_or_else(Vec::new, |plan| {
            plan.milestones
                .iter()
                .map(|milestone| {
                    let (done, of) = milestone.progress();
                    MilestoneView {
                        id: milestone.id.clone(),
                        title: milestone.title.clone(),
                        done,
                        of,
                        tasks: milestone
                            .tasks
                            .iter()
                            .map(|task| TaskView {
                                id: task.id.clone(),
                                title: task.title.clone(),
                                criterion: task.criterion.clone(),
                                status: TaskState::from(task.status),
                                sessions: task.sessions.clone(),
                                usage: task_usage.get(&task.id).cloned(),
                                notes: task
                                    .notes
                                    .iter()
                                    .map(|note| NoteView {
                                        at: note.at.clone(),
                                        session: note.session.clone(),
                                        text: note.text.clone(),
                                    })
                                    .collect(),
                            })
                            .collect(),
                    }
                })
                .collect()
        });
        Self {
            milestones,
            todo: if open.is_some() {
                todo_items
            } else {
                Vec::new()
            },
            open,
            sessions,
            sessions_total: total,
            problems,
            review: ReviewSummary::default(),
            debt: None,
            sessions_since_review: 0,
            look: Vec::new(),
            configured: false,
        }
    }
}

/// `S-2026-10-20-12` after `S-2026-10-20-9`: by date, then by number.
fn session_order(session: &Session) -> (&str, u64) {
    let id = session.id.as_str();
    match id.rsplit_once('-') {
        Some((day, n)) => (day, n.parse().unwrap_or(0)),
        None => (id, 0),
    }
}

impl From<TaskStatus> for TaskState {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Todo => TaskState::Todo,
            TaskStatus::InProgress => TaskState::InProgress,
            TaskStatus::Blocked => TaskState::Blocked,
            TaskStatus::Done => TaskState::Done,
            TaskStatus::Dropped => TaskState::Dropped,
        }
    }
}

/// Files, relative to the root: opened in the editor, or shown on the map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilesParams {
    pub paths: Vec<String>,
}

/// `lsp/outgoingCalls`, from the daemon to the client that relays its IDE's language servers
/// (ADR 0014): the calls of the function whose name starts at `position`. Positions are the
/// model's: 0-based lines, UTF-8 byte columns; the client converts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingCallsParams {
    /// Relative to the root.
    pub path: String,
    #[ts(type = "Position")]
    pub position: Position,
}

/// One callee, in the answer to `lsp/outgoingCalls`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingCall {
    /// Its file, relative to the root; `null` outside the repository.
    pub path: Option<String>,
    /// The start of its name.
    #[ts(type = "Position")]
    pub at: Position,
    /// Call sites in the caller.
    pub sites: u32,
}

/// `lsp/definitions`, from the daemon to the client that relays (ADR 0022): where the names at
/// these positions of a file are defined, in one batch. The answer is one list of
/// [`DefinitionTarget`] per question, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionsParams {
    /// Relative to the root.
    pub path: String,
    pub questions: Vec<DefinitionQuestion>,
}

/// One name of a batch of `lsp/definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionQuestion {
    #[ts(type = "Position")]
    pub position: Position,
    /// Tree-sitter places the definition in another file: an answer « here » means the server
    /// does not know yet, and the client may ask again.
    pub elsewhere: bool,
}

/// One definition, in the answer to `lsp/definitions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionTarget {
    /// Its file, relative to the root; `null` outside the repository.
    pub path: Option<String>,
    /// The start of the defined name.
    #[ts(type = "Position")]
    pub at: Position,
}

/// `lsp/references`, from the daemon to the client that relays (ADR 0022): the files that use
/// the symbol named at `position`. The answer is their paths, relative to the root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReferencesParams {
    /// Relative to the root.
    pub path: String,
    #[ts(type = "Position")]
    pub position: Position,
}

/// The daemon as a front-end host sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DaemonState {
    /// Launched, not answering yet.
    Starting,
    Connected,
    /// Lost: launched again after a delay (ADR 0019).
    Restarting,
    /// Cannot be launched: no folder open, no program found.
    Unavailable,
}

/// From a front-end host (the VSCode extension) to its webview, by `postMessage`. The webview
/// never talks to the daemon (ADR 0019).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HostMessage {
    /// The whole view: after a subscription, a resync, or a restart of the daemon.
    Snapshot {
        snapshot: ViewSnapshot,
    },
    Delta {
        delta: ViewDelta,
    },
    Daemon {
        state: DaemonState,
        /// Why, for a person: the error, the program looked for.
        detail: Option<String>,
        /// How far the start is (T-115): the files the first indexing read, of how many.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        progress: Option<IndexProgress>,
    },
    /// A command of the palette, for the map (PLAN.md 4.9).
    Command {
        command: MapCommand,
    },
    /// The agents `timonier init` configures, and those found on this machine (T-118).
    Agents {
        agents: Vec<AgentChoice>,
    },
    /// The development process, for its settings panel (ADR 0056); `problem` when the daemon
    /// refused a setting or cannot answer.
    Workflow {
        view: Option<crate::workflow::WorkflowView>,
        problem: Option<String>,
    },
    /// How `timonier init` went, from the bridge (T-118): `ok`, else the end of what it said.
    Configured {
        ok: bool,
        detail: Option<String>,
    },
    /// The boxes moved by hand, as the host kept them (ADR 0021, « Boîtes déplacées »).
    Pins {
        #[ts(type = "Record<string, Pin>")]
        pins: BTreeMap<String, Pin>,
    },
    /// How dense the map is drawn, from the settings (spec « Adjustable density », Q32).
    Density {
        density: Density,
    },
    /// The plan, the todo and the journal, whole, at a revision (ADR 0031).
    Tracking {
        #[ts(type = "number")]
        revision: u64,
        tracking: TrackingView,
    },
    /// A command of the palette, for the whiteboard (ADR 0037).
    Whiteboard {
        command: WhiteboardCommand,
    },
    /// The whiteboard as `.timonier/intent.json` holds it: when the page opens, and when the
    /// file changes outside it (ADR 0039). `null` before anything was drawn.
    Intent {
        intent: Option<Intent>,
        /// Why the file does not read; the page then waits for the developer to start over.
        problem: Option<String>,
    },
    /// Whether the last gestures reached `intent.json` (Q66).
    IntentSaved {
        saved: bool,
        detail: Option<String>,
    },
    /// How the code drifts from the whiteboard, while the map compares them (ADR 0042); `null`
    /// when it cannot be had.
    Drift {
        drift: Option<crate::drift::DriftView>,
    },
    /// The map's Performances layer, while it is on (ADR 0051); `null` when it cannot be had.
    PerfMap {
        perf: Option<crate::perf::PerfMap>,
    },
    /// The Performances panel: the last runs, again when a run ends (ADR 0051).
    PerfRuns {
        runs: crate::perf::PerfRuns,
    },
    /// The run the panel shows; `null` when it is gone or the daemon cannot answer.
    PerfRun {
        run: Option<crate::perf::PerfRun>,
    },
    /// The regressions of the latest batch, for the panel (ADR 0053); `null` without daemon.
    PerfRegressions {
        report: Option<crate::perf::RegressionReport>,
    },
    /// The whiteboard's budgets checked on the last runs, for the map, always (Q74, ADR 0052);
    /// `null` without budgets or daemon.
    Budgets {
        budgets: Option<crate::budgets::BudgetsView>,
    },
}

/// The density of the map: the sizes of its boxes and the room between them (Q32).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Where a box moved by hand stays (ADR 0021): in pixels, relative to its container. A pin
/// counts only in that container.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// The id of the container: a role group, an open box, or `@root` for the map itself.
    pub parent: String,
    pub x: i32,
    pub y: i32,
}

/// Which way the palette moves a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// What the palette asks the map. Without a target, the item that has the focus or is selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "name", rename_all = "camelCase")]
pub enum MapCommand {
    FitView,
    ZoomIn,
    ZoomOut,
    CloseAll,
    /// Open or close a role group, expand or fold a box.
    Toggle {
        #[ts(type = "string | null")]
        target: Option<String>,
    },
    OpenCode {
        #[ts(type = "string | null")]
        target: Option<String>,
    },
    /// Move a box by one cell of the board, and pin it there (ADR 0021).
    Move {
        #[ts(type = "string | null")]
        target: Option<String>,
        direction: Direction,
    },
    /// Forget the boxes moved by hand: the layout is computed again.
    ResetLayout,
    /// Quiet what did not change since the last review, or no longer (ADR 0024).
    ToggleChanges,
    /// Show or hide the lists of the review summary.
    ToggleReviewDetails,
    /// Show or hide the review debt on the boxes (ADR 0030, Q44).
    ToggleDebt,
    /// Compare the map with the whiteboard, or no longer (ADR 0042).
    ToggleIntent,
    /// Show the Performances layer of the last run, or no longer (ADR 0051).
    TogglePerformance,
    /// Open what leads to a box, then center and select it (ADR 0032, Q46): `path` are its
    /// ancestors from the top of the map.
    Reveal {
        target: String,
        path: Vec<String>,
    },
}

/// What the pointer does on the whiteboard (ADR 0036).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WhiteboardTool {
    /// Select, move and delete what is drawn; drag the board.
    Select,
    /// Draw freehand.
    Pen,
    /// Erase the strokes the pointer goes over.
    Eraser,
    /// Put a typed shape where the pointer clicks (ADR 0038).
    Box,
    Zone,
    Note,
    Budget,
    /// Draw an arrow from a box or zone to another: two clicks.
    Arrow,
}

/// A typed shape the palette adds in the middle of the view (ADR 0038).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WhiteboardShape {
    Box,
    Zone,
    Note,
    Budget,
}

/// What the palette asks the whiteboard (ADR 0037).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "name", rename_all = "camelCase")]
pub enum WhiteboardCommand {
    Undo,
    Redo,
    Tool {
        tool: WhiteboardTool,
    },
    /// Add a shape in the middle of the view, selected, its properties open.
    Add {
        shape: WhiteboardShape,
    },
    /// Delete what is selected.
    DeleteSelection,
    FitView,
    ZoomIn,
    ZoomOut,
}

/// From the webview to its host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WebviewMessage {
    /// Rendered for the first time: the bundle runs under the CSP.
    Ready { protocol: u32 },
    /// Show this view, in place of the previous one.
    Subscribe { view: ViewSpec },
    /// A delta went missing: the whole view again.
    Resync,
    /// The files the map shows now, relative to the root: they come first for refinement
    /// (`view/displayed`, ADR 0014).
    Displayed { paths: Vec<String> },
    /// Show this code in the editor: a file relative to the root, at a span of the model.
    OpenCode {
        file: String,
        #[ts(type = "Span | null")]
        span: Option<Span>,
    },
    /// The boxes moved by hand: the host keeps them for the next pages (ADR 0021).
    Pins {
        #[ts(type = "Record<string, Pin>")]
        pins: BTreeMap<String, Pin>,
    },
    /// The map's « Mark as reviewed » button: the host runs the command (ADR 0024).
    MarkReviewed,
    /// The bridge asks the map to show a box (ADR 0032, Q46).
    Reveal { target: String, path: Vec<String> },
    /// The bridge's « Open Map » button.
    OpenMap,
    /// The bridge's « Set up » button: `timonier init` for the agents chosen, without a terminal
    /// (T-118, in place of Q53); the palette's command gives none, and init finds them.
    Configure {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        agents: Option<Vec<String>>,
    },
    /// The bridge, not set up: which agents `timonier init` would configure (T-118).
    DetectAgents,
    /// The settings panel asks for the process (ADR 0056).
    AskWorkflow,
    /// A setting changed in the panel: the whole process, for the daemon to check and write.
    SetWorkflow { workflow: crate::workflow::Workflow },
    /// A panel of the tracking shows this revision now (ADR 0031).
    TrackingShown {
        panel: String,
        #[ts(type = "number")]
        revision: u64,
    },
    /// The whiteboard after a gesture of the developer: the host writes it (Q66, ADR 0039).
    Intent { intent: Intent },
    /// The map compares with the whiteboard, or no longer (ADR 0042).
    CompareIntent { on: bool },
    /// The map's « Open the whiteboard » button.
    OpenWhiteboard,
    /// The map shows its Performances layer, or no longer (ADR 0051).
    ShowPerformance { on: bool },
    /// « Open the Performances panel », from the map's layer.
    OpenPerformance,
    /// The Performances panel shows this run (`R-…`).
    ChooseRun { run: String },
    /// The Performances panel's « Run the tests »: the host shows the commands, then runs
    /// `timonier test` in a terminal once approved.
    RunTests,
    /// What the webview shows now, once a snapshot or a delta is applied.
    Rendered {
        #[ts(type = "number")]
        subscription: u64,
        #[ts(type = "number")]
        revision: u64,
        nodes: u32,
    },
}

/// The TypeScript of the protocol, as one module (ADR 0019).
pub fn typescript() -> String {
    let cfg = ts_rs::Config::new().with_large_int("number");
    let mut out = String::from(
        "// Generated from crates/timonier-core/src/protocol.rs by its test `typescript_is_up_to_date`.\n\
         // Do not edit: change the Rust types, then run `TIMONIER_UPDATE_TS=1 cargo test -p timonier-core`.\n\n",
    );
    out.push_str(&format!(
        "export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n\n"
    ));
    for (name, union) in [
        ("NodeKind", NODE_KINDS),
        ("Role", ROLES),
        ("EdgeKind", EDGE_KINDS),
        ("Provenance", PROVENANCES),
    ] {
        out.push_str(&format!("export type {name} = {union};\n\n"));
    }
    out.push_str("export type Position = { line: number; column: number };\n\n");
    out.push_str("export type Span = { start: Position; end: Position };\n\n");
    for decl in [
        InitializeParams::decl(&cfg),
        InitializeResult::decl(&cfg),
        StatusResult::decl(&cfg),
        IndexProgress::decl(&cfg),
        AgentChoice::decl(&cfg),
        crate::workflow::Preset::decl(&cfg),
        crate::workflow::TicketProvider::decl(&cfg),
        crate::workflow::CommitConvention::decl(&cfg),
        crate::workflow::MergeStrategy::decl(&cfg),
        crate::workflow::CiProvider::decl(&cfg),
        crate::workflow::Tickets::decl(&cfg),
        crate::workflow::Branches::decl(&cfg),
        crate::workflow::Commits::decl(&cfg),
        crate::workflow::PullRequests::decl(&cfg),
        crate::workflow::Merge::decl(&cfg),
        crate::workflow::Ci::decl(&cfg),
        crate::workflow::Workflow::decl(&cfg),
        crate::workflow::WorkflowView::decl(&cfg),
        ReviewMark::decl(&cfg),
        ReviewChange::decl(&cfg),
        FanJump::decl(&cfg),
        NodeReview::decl(&cfg),
        NodeDebt::decl(&cfg),
        EdgeReview::decl(&cfg),
        ReviewBox::decl(&cfg),
        ReviewDependency::decl(&cfg),
        ReviewFan::decl(&cfg),
        ReviewSummary::decl(&cfg),
        ViewSpec::decl(&cfg),
        SubscribeParams::decl(&cfg),
        SubscriptionParams::decl(&cfg),
        ViewNode::decl(&cfg),
        ViewEdge::decl(&cfg),
        EdgeRef::decl(&cfg),
        ViewSnapshot::decl(&cfg),
        ViewDelta::decl(&cfg),
        FilesParams::decl(&cfg),
        OutgoingCallsParams::decl(&cfg),
        OutgoingCall::decl(&cfg),
        DefinitionsParams::decl(&cfg),
        DefinitionQuestion::decl(&cfg),
        DefinitionTarget::decl(&cfg),
        ReferencesParams::decl(&cfg),
        LspAnswers::decl(&cfg),
        LookupCounts::decl(&cfg),
        ChangedBox::decl(&cfg),
        ChangedLink::decl(&cfg),
        SessionChanges::decl(&cfg),
        TaskState::decl(&cfg),
        NoteView::decl(&cfg),
        TaskView::decl(&cfg),
        MilestoneView::decl(&cfg),
        TodoItemView::decl(&cfg),
        SessionView::decl(&cfg),
        crate::usage::TokenUsage::decl(&cfg),
        TaskUsage::decl(&cfg),
        TrackingProblem::decl(&cfg),
        LookItem::decl(&cfg),
        TrackingView::decl(&cfg),
        TrackingSnapshot::decl(&cfg),
        Pin::decl(&cfg),
        Direction::decl(&cfg),
        Density::decl(&cfg),
        DaemonState::decl(&cfg),
        MapCommand::decl(&cfg),
        WhiteboardTool::decl(&cfg),
        WhiteboardShape::decl(&cfg),
        WhiteboardCommand::decl(&cfg),
        crate::intent::Intent::decl(&cfg),
        crate::intent::IntentLayer::decl(&cfg),
        crate::intent::IntentComponent::decl(&cfg),
        crate::intent::Policy::decl(&cfg),
        crate::intent::IntentEdgeKind::decl(&cfg),
        crate::intent::IntentEdge::decl(&cfg),
        crate::intent::IntentNote::decl(&cfg),
        crate::intent::IntentBudget::decl(&cfg),
        crate::intent::Sketch::decl(&cfg),
        crate::intent::Place::decl(&cfg),
        crate::intent::SketchStroke::decl(&cfg),
        crate::actual::ActualDependency::decl(&cfg),
        crate::drift::Anchor::decl(&cfg),
        crate::drift::Crossing::decl(&cfg),
        crate::drift::Unchecked::decl(&cfg),
        crate::drift::Drift::decl(&cfg),
        crate::drift::DriftView::decl(&cfg),
        crate::perf::CpuSource::decl(&cfg),
        crate::perf::TestStatus::decl(&cfg),
        crate::perf::SpanStatus::decl(&cfg),
        crate::perf::Sample::decl(&cfg),
        crate::perf::RunSummary::decl(&cfg),
        crate::perf::PerfRuns::decl(&cfg),
        crate::perf::PerfRunParams::decl(&cfg),
        crate::perf::TestView::decl(&cfg),
        crate::perf::SpanView::decl(&cfg),
        crate::perf::PerfRun::decl(&cfg),
        crate::perf::NodePerf::decl(&cfg),
        crate::perf::LinkPerf::decl(&cfg),
        crate::perf::PerfMap::decl(&cfg),
        crate::regression::Verdict::decl(&cfg),
        crate::perf::RegressionStatus::decl(&cfg),
        crate::perf::RegressionNote::decl(&cfg),
        crate::perf::Regression::decl(&cfg),
        crate::perf::RegressionReport::decl(&cfg),
        crate::perf::RegressionParams::decl(&cfg),
        crate::budgets::LimitKind::decl(&cfg),
        crate::budgets::LimitCheck::decl(&cfg),
        crate::budgets::BudgetState::decl(&cfg),
        crate::budgets::BudgetCheck::decl(&cfg),
        crate::budgets::BudgetsView::decl(&cfg),
        IntentRead::decl(&cfg),
        IntentWriteParams::decl(&cfg),
        IntentWritten::decl(&cfg),
        HostMessage::decl(&cfg),
        WebviewMessage::decl(&cfg),
    ] {
        out.push_str("export ");
        out.push_str(&decl);
        out.push_str("\n\n");
    }
    let methods = [
        ("INITIALIZE", methods::INITIALIZE),
        ("STATUS", methods::STATUS),
        ("SHUTDOWN", methods::SHUTDOWN),
        ("SUBSCRIBE", methods::SUBSCRIBE),
        ("UPDATE", methods::UPDATE),
        ("RESYNC", methods::RESYNC),
        ("UNSUBSCRIBE", methods::UNSUBSCRIBE),
        ("OPENED", methods::OPENED),
        ("DISPLAYED", methods::DISPLAYED),
        ("SAVED", methods::SAVED),
        ("OUTGOING_CALLS", methods::OUTGOING_CALLS),
        ("DEFINITIONS", methods::DEFINITIONS),
        ("REFERENCES", methods::REFERENCES),
        ("REVIEW_MARK", methods::REVIEW_MARK),
        ("TRACKING_SUBSCRIBE", methods::TRACKING_SUBSCRIBE),
        ("TRACKING_UPDATE", methods::TRACKING_UPDATE),
        ("INTENT_READ", methods::INTENT_READ),
        ("INTENT_WRITE", methods::INTENT_WRITE),
        ("INTENT_DRIFT", methods::INTENT_DRIFT),
        ("PERF_RUNS", methods::PERF_RUNS),
        ("PERF_RUN", methods::PERF_RUN),
        ("PERF_MAP", methods::PERF_MAP),
        ("PERF_BUDGETS", methods::PERF_BUDGETS),
        ("PERF_REGRESSIONS", methods::PERF_REGRESSIONS),
        ("INVENTORY", methods::INVENTORY),
        ("INVENTORY_SBOM", methods::INVENTORY_SBOM),
        ("WORKFLOW_GET", methods::WORKFLOW_GET),
        ("WORKFLOW_SET", methods::WORKFLOW_SET),
    ];
    out.push_str("export const METHODS = {\n");
    for (name, value) in methods {
        out.push_str(&format!("  {name}: \"{value}\",\n"));
    }
    out.push_str("} as const;\n\n");
    // The codes the relaying client answers with.
    out.push_str("export const ERRORS = {\n");
    for (name, value) in [
        ("REQUEST_FAILED", errors::REQUEST_FAILED),
        ("CONTENT_MODIFIED", errors::CONTENT_MODIFIED),
    ] {
        out.push_str(&format!("  {name}: {value},\n"));
    }
    out.push_str("} as const;\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::{Milestone, SessionSnapshots, Stamp, Task};

    fn session(id: &str, status: SessionStatus) -> Session {
        Session {
            version: 1,
            id: id.to_owned(),
            agent: "agent".into(),
            objective: format!("objective of {id}"),
            tasks: Vec::new(),
            status,
            started_at: "2026-10-20T08:00:00Z".into(),
            ended_at: None,
            snapshots: SessionSnapshots {
                start: 1,
                end: None,
            },
            summary: None,
            todo: vec![TodoItem {
                id: "D-1".into(),
                text: "kept".into(),
                task: None,
                status: TodoStatus::Done,
            }],
            changes: Some(SessionChanges {
                files: 3,
                ..SessionChanges::default()
            }),
            budgets: Vec::new(),
            agent_sessions: Vec::new(),
            usage: None,
        }
    }

    fn task(id: &str, status: TaskStatus) -> Task {
        Task {
            id: id.into(),
            title: id.into(),
            criterion: None,
            status,
            components: Vec::new(),
            sessions: Vec::new(),
            created: Stamp {
                at: "2026-10-20T08:00:00Z".into(),
                session: "S-2026-10-20-1".into(),
            },
            notes: Vec::new(),
            ticket: None,
        }
    }

    #[test]
    fn the_tracking_view_puts_the_latest_sessions_first_and_the_open_one_live() {
        let plan = Plan {
            version: 1,
            milestones: vec![Milestone {
                id: "M-1".into(),
                title: "First".into(),
                tasks: vec![
                    task("T-1", TaskStatus::Done),
                    task("T-2", TaskStatus::Dropped),
                    task("T-3", TaskStatus::InProgress),
                ],
            }],
        };
        let todo = Todo {
            version: 1,
            session: Some("S-2026-10-20-10".into()),
            items: vec![TodoItem {
                id: "D-1".into(),
                text: "live".into(),
                task: Some("T-3".into()),
                status: TodoStatus::Open,
            }],
        };
        let mut sessions: Vec<Session> = (1..=9)
            .map(|n| session(&format!("S-2026-10-20-{n}"), SessionStatus::Ended))
            .collect();
        sessions.push(session("S-2026-10-19-4", SessionStatus::Ended));
        sessions.insert(0, session("S-2026-10-20-10", SessionStatus::Open));
        let live = SessionChanges {
            files: 1,
            ..SessionChanges::default()
        };
        let view = TrackingView::new(Some(&plan), Some(&todo), sessions, Some(live), Vec::new());
        let milestone = &view.milestones[0];
        assert_eq!(
            (milestone.done, milestone.of),
            (1, 2),
            "dropped tasks do not count"
        );
        assert_eq!(milestone.tasks[2].status, TaskState::InProgress);
        assert_eq!(view.open.as_deref(), Some("S-2026-10-20-10"));
        let ids: Vec<&str> = view.sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids[..3],
            ["S-2026-10-20-10", "S-2026-10-20-9", "S-2026-10-20-8"]
        );
        assert_eq!(ids.last(), Some(&"S-2026-10-19-4"));
        assert_eq!(view.sessions_total, 11);
        let open = &view.sessions[0];
        assert!(open.open);
        assert_eq!(open.todo[0].text, "live");
        assert_eq!(open.changes.as_ref().map(|c| c.files), Some(1));
        assert_eq!(view.todo, open.todo);
        assert_eq!(view.sessions[1].todo[0].text, "kept");
        assert_eq!(view.sessions[1].changes.as_ref().map(|c| c.files), Some(3));
        // Between sessions: no todo, none open.
        let mut closed = todo.clone();
        closed.session = None;
        let between = TrackingView::new(Some(&plan), Some(&closed), Vec::new(), None, Vec::new());
        assert!(between.open.is_none() && between.todo.is_empty());
    }

    #[test]
    fn the_journal_is_capped() {
        let sessions = (1..=60)
            .map(|n| session(&format!("S-2026-10-20-{n}"), SessionStatus::Ended))
            .collect();
        let view = TrackingView::new(None, None, sessions, None, Vec::new());
        assert_eq!(view.sessions.len(), JOURNAL_LIMIT);
        assert_eq!(view.sessions_total, 60);
        assert_eq!(view.sessions[0].id, "S-2026-10-20-60");
    }

    fn names(union: &str) -> Vec<String> {
        union
            .split(" | ")
            .map(|n| n.trim_matches('"').to_owned())
            .collect()
    }

    #[test]
    fn the_unions_name_every_kind_of_the_model() {
        let kinds: Vec<&str> = NodeKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(names(NODE_KINDS), kinds);
        let roles: Vec<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(names(ROLES), roles);
        let edges: Vec<&str> = EdgeKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(names(EDGE_KINDS), edges);
        let provenances: Vec<&str> = Provenance::ALL.iter().map(|p| p.as_str()).collect();
        assert_eq!(names(PROVENANCES), provenances);
    }

    #[test]
    fn messages_use_camel_case_json() {
        let node = ViewNode {
            id: NodeId::file("src/a.rs").unwrap(),
            kind: NodeKind::File,
            name: "a.rs".into(),
            parent: Some(NodeId::package("cargo", "a").unwrap()),
            role: None,
            language: Some(Language::new("rust").unwrap()),
            children: 2,
            file: Some("src/a.rs".into()),
            span: None,
            signature: None,
            review: Some(NodeReview {
                change: Some(ReviewChange::Added),
                fan_in: Some(FanJump {
                    before: 1,
                    after: 4,
                }),
                fan_out: None,
                cycle: false,
            }),
            debt: Some(NodeDebt {
                files: 1,
                of: 1,
                untracked: 0,
            }),
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["kind"], "file");
        assert_eq!(
            json["debt"],
            serde_json::json!({"files": 1, "of": 1, "untracked": 0})
        );
        assert_eq!(json["review"]["change"], "added");
        assert_eq!(json["review"]["fanIn"]["after"], 4);
        assert_eq!(json["parent"], "pkg:cargo/a");
        assert_eq!(serde_json::from_value::<ViewNode>(json).unwrap(), node);
        let delta = ViewDelta {
            subscription: 1,
            base: 3,
            revision: 4,
            nodes: Vec::new(),
            removed_nodes: vec![NodeId::file("src/b.rs").unwrap()],
            edges: Vec::new(),
            removed_edges: Vec::new(),
            review: Some(ReviewSummary::default()),
        };
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["removedNodes"][0], "file:src/b.rs");
        assert_eq!(json["review"]["dependenciesAdded"], serde_json::json!([]));
        assert_eq!(json["review"]["since"], serde_json::Value::Null);
    }

    #[test]
    fn whiteboard_commands_say_their_name_and_tool() {
        let message = HostMessage::Whiteboard {
            command: WhiteboardCommand::Tool {
                tool: WhiteboardTool::Pen,
            },
        };
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "whiteboard", "command": {"name": "tool", "tool": "pen"}})
        );
        assert_eq!(
            serde_json::from_value::<HostMessage>(json).unwrap(),
            message
        );
        let undo = serde_json::to_value(WhiteboardCommand::DeleteSelection).unwrap();
        assert_eq!(undo, serde_json::json!({"name": "deleteSelection"}));
    }

    /// The TypeScript in the repository is what the Rust types say (ADR 0019).
    #[test]
    fn typescript_is_up_to_date() {
        let generated = typescript();
        // The webview and the extension each hold a copy: neither builds from the other's sources.
        for file in [
            "webview/src/protocol/generated.ts",
            "editors/vscode/src/protocol/generated.ts",
        ] {
            let path = format!("{}/../../{file}", env!("CARGO_MANIFEST_DIR"));
            if std::env::var_os("TIMONIER_UPDATE_TS").is_some() {
                std::fs::write(&path, &generated).unwrap();
            }
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                committed == generated,
                "{file} is out of date: run `TIMONIER_UPDATE_TS=1 cargo test -p timonier-core`"
            );
        }
    }

    #[test]
    fn relay_messages_are_tagged_by_type() {
        let ready: WebviewMessage =
            serde_json::from_str(r#"{"type":"ready","protocol":1}"#).unwrap();
        assert_eq!(ready, WebviewMessage::Ready { protocol: 1 });
        let resync: WebviewMessage = serde_json::from_str(r#"{"type":"resync"}"#).unwrap();
        assert_eq!(resync, WebviewMessage::Resync);
        let state = HostMessage::Daemon {
            state: DaemonState::Restarting,
            detail: None,
            progress: None,
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["type"], "daemon");
        assert_eq!(json["state"], "restarting");
    }
}
