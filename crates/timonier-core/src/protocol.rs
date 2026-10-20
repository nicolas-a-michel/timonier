//! The protocol between the daemon and its clients (ADR 0019): JSON-RPC 2.0 messages framed by
//! `Content-Length`, whose params and results are the types below. The TypeScript side is
//! generated from them (`webview/src/protocol/generated.ts`, checked by a test).
//!
//! Model types travel as their JSON form: ids as strings, kinds as their names.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AggregatedEdge, EdgeKind, Language, NodeId, NodeKind, Position, Provenance, Role, Snapshot,
    Span,
};

/// Bumped when a change breaks clients; a client with another version is refused.
pub const PROTOCOL_VERSION: u32 = 3;

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
const ROLES: &str = "\"entry_point\" | \"data_model\" | \"persistence\" | \"interface\" | \"background_task\" | \"external_libraries\" | \"utility\"";
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
    /// What changed in it since the last review; `null` without change, and before the first
    /// review (ADR 0024).
    pub review: Option<NodeReview>,
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
    /// Tree-sitter alone: drawn differently (spec « Précision affichée »).
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    },
    /// A command of the palette, for the map (PLAN.md 4.9).
    Command {
        command: MapCommand,
    },
    /// The boxes moved by hand, as the host kept them (ADR 0021, « Boîtes déplacées »).
    Pins {
        #[ts(type = "Record<string, Pin>")]
        pins: BTreeMap<String, Pin>,
    },
    /// How dense the map is drawn, from the settings (spec « Densité réglable », Q32).
    Density {
        density: Density,
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
}

/// From the webview to its host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
        ReviewMark::decl(&cfg),
        ReviewChange::decl(&cfg),
        FanJump::decl(&cfg),
        NodeReview::decl(&cfg),
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
        Pin::decl(&cfg),
        Direction::decl(&cfg),
        Density::decl(&cfg),
        DaemonState::decl(&cfg),
        MapCommand::decl(&cfg),
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
            review: Some(NodeReview {
                change: Some(ReviewChange::Added),
                fan_in: Some(FanJump {
                    before: 1,
                    after: 4,
                }),
                fan_out: None,
                cycle: false,
            }),
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["kind"], "file");
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
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["type"], "daemon");
        assert_eq!(json["state"], "restarting");
    }
}
