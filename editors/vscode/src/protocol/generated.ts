// Generated from crates/timonier-core/src/protocol.rs by its test `typescript_is_up_to_date`.
// Do not edit: change the Rust types, then run `TIMONIER_UPDATE_TS=1 cargo test -p timonier-core`.

export const PROTOCOL_VERSION = 6;

export type NodeKind = "repo" | "component" | "package" | "file" | "module" | "type" | "trait" | "function" | "method" | "field" | "constant";

export type Role = "entry_point" | "data_model" | "persistence" | "interface" | "background_task" | "external_libraries" | "utility";

export type EdgeKind = "imports" | "calls" | "references" | "implements" | "inherits" | "depends_on";

export type Provenance = "tree_sitter" | "lsp" | "scip" | "manifest" | "inferred" | "intended";

export type Position = { line: number; column: number };

export type Span = { start: Position; end: Position };

export type InitializeParams = { protocolVersion: number, 
/**
 * Who connects, for the logs: `vscode 1.142.0`, `timonier-cli 0.1.0`.
 */
client: string, 
/**
 * The client relays its IDE's language servers (`lsp/outgoingCalls`, ADR 0014).
 */
lspRelay: boolean, };

export type InitializeResult = { protocolVersion: number, daemonVersion: string, 
/**
 * The repository the daemon indexes, canonical.
 */
root: string, 
/**
 * The file extensions its packs read (`rs`, `ts`…): a relaying client answers only while
 * none of those files has unsaved changes (ADR 0022).
 */
sourceExtensions: Array<string>, };

export type StatusResult = { root: string, daemonVersion: string, 
/**
 * Files of the repository in the graph.
 */
files: number, 
/**
 * The open snapshot every change goes to (ADR 0010).
 */
head: number, 
/**
 * Files waiting for LSP refinement (ADR 0014), and lookups of dependents (ADR 0022).
 */
refinementsWaiting: number, 
/**
 * Those held until the relaying client's documents are saved (ADR 0022).
 */
refinementsHeld: number, clients: number, 
/**
 * Questions the relaying clients answered since the start (ADR 0022).
 */
lspAnswers: LspAnswers, 
/**
 * Lookups of dependents since the start (ADR 0022).
 */
lookups: LookupCounts, 
/**
 * The last review mark; `null` before the first review (ADR 0024).
 */
lastReview: ReviewMark | null, };

export type ReviewMark = { seq: number, 
/**
 * Milliseconds since the Unix epoch.
 */
timestampMs: number, };

export type ReviewChange = "added" | "modified";

export type FanJump = { before: number, after: number, };

export type NodeReview = { change: ReviewChange | null, fanIn: FanJump | null, fanOut: FanJump | null, 
/**
 * In a cycle added since the review.
 */
cycle: boolean, };

export type NodeDebt = { files: number, of: number, untracked: number, };

export type EdgeReview = { 
/**
 * No edge linked these two nodes at the review, and a change of the code explains it.
 */
added: boolean, 
/**
 * Between two boxes of a cycle added since the review.
 */
cycle: boolean, };

export type ReviewBox = { id: string, kind: NodeKind, name: string, role: Role | null, 
/**
 * The name of the box that holds it, to tell apart two of the same name; `null` at the
 * top.
 */
within: string | null, };

export type ReviewDependency = { src: ReviewBox, dst: ReviewBox, };

export type ReviewFan = { item: ReviewBox, fanIn: FanJump | null, fanOut: FanJump | null, };

export type ReviewSummary = { 
/**
 * The review the diff starts from; `null` before the first one: no diff then (Q27).
 */
since: ReviewMark | null, added: Array<ReviewBox>, modified: Array<ReviewBox>, removed: Array<ReviewBox>, dependenciesAdded: Array<ReviewDependency>, dependenciesRemoved: Array<ReviewDependency>, cycles: Array<Array<ReviewBox>>, fans: Array<ReviewFan>, };

export type ViewSpec = { expanded: string[], };

export type SubscribeParams = { view: ViewSpec, };

export type SubscriptionParams = { subscription: number, };

export type ViewNode = { id: string, kind: NodeKind, name: string, 
/**
 * Its parent on the map; `null` at the top.
 */
parent: string | null, role: Role | null, language: string | null, 
/**
 * Children it can be expanded into.
 */
children: number, 
/**
 * The file to open, relative to the root: the file itself, or the symbol's.
 */
file: string | null, span: Span | null, 
/**
 * What changed in it since the last review; `null` without change, and before the first
 * review (ADR 0024).
 */
review: NodeReview | null, 
/**
 * Its review debt: files written since the last review and not read since (ADR 0030);
 * `null` without any.
 */
debt: NodeDebt | null, };

export type ViewEdge = { src: string, dst: string, kind: EdgeKind, weight: number, 
/**
 * How many stored edges it stands for.
 */
edges: number, 
/**
 * The most reliable provenance among them.
 */
best: Provenance, 
/**
 * Tree-sitter alone: drawn differently (spec « Précision affichée »).
 */
approximateOnly: boolean, 
/**
 * What changed since the last review; `null` without change (ADR 0024).
 */
review: EdgeReview | null, };

export type EdgeRef = { src: string, dst: string, kind: EdgeKind, };

export type ViewSnapshot = { subscription: number, revision: number, nodes: Array<ViewNode>, edges: Array<ViewEdge>, review: ReviewSummary, };

export type ViewDelta = { subscription: number, base: number, revision: number, 
/**
 * Nodes added or changed.
 */
nodes: Array<ViewNode>, removedNodes: string[], 
/**
 * Edges added or changed.
 */
edges: Array<ViewEdge>, removedEdges: Array<EdgeRef>, 
/**
 * The review summary, when it changed.
 */
review: ReviewSummary | null, };

export type FilesParams = { paths: Array<string>, };

export type OutgoingCallsParams = { 
/**
 * Relative to the root.
 */
path: string, position: Position, };

export type OutgoingCall = { 
/**
 * Its file, relative to the root; `null` outside the repository.
 */
path: string | null, 
/**
 * The start of its name.
 */
at: Position, 
/**
 * Call sites in the caller.
 */
sites: number, };

export type DefinitionsParams = { 
/**
 * Relative to the root.
 */
path: string, questions: Array<DefinitionQuestion>, };

export type DefinitionQuestion = { position: Position, 
/**
 * Tree-sitter places the definition in another file: an answer « here » means the server
 * does not know yet, and the client may ask again.
 */
elsewhere: boolean, };

export type DefinitionTarget = { 
/**
 * Its file, relative to the root; `null` outside the repository.
 */
path: string | null, 
/**
 * The start of the defined name.
 */
at: Position, };

export type ReferencesParams = { 
/**
 * Relative to the root.
 */
path: string, position: Position, };

export type LspAnswers = { outgoingCalls: number, definitions: number, references: number, 
/**
 * Questions the client could not answer: no server for the file, or none ready.
 */
failed: number, };

export type LookupCounts = { 
/**
 * The language server named the users.
 */
named: number, 
/**
 * By the files citing the name: the server could not say, or the symbol is gone from the
 * file changed since.
 */
byName: number, };

export type ChangedBox = { 
/**
 * Its id in the graph (`pkg:cargo/demo`, `component:dir/src`).
 */
id: string, name: string, 
/**
 * The name of the box that holds it; `null` at the top.
 */
within: string | null, };

export type ChangedLink = { from: ChangedBox, to: ChangedBox, };

export type SessionChanges = { 
/**
 * Files added, removed or changed.
 */
files: number, added: Array<ChangedBox>, modified: Array<ChangedBox>, removed: Array<ChangedBox>, 
/**
 * Dependencies between boxes that the session brought.
 */
linked: Array<ChangedLink>, 
/**
 * Dependencies that it took away.
 */
unlinked: Array<ChangedLink>, };

export type TaskState = "todo" | "inProgress" | "blocked" | "done" | "dropped";

export type NoteView = { 
/**
 * UTC, RFC 3339.
 */
at: string, session: string, text: string, };

export type TaskView = { id: string, title: string, criterion: string | null, status: TaskState, sessions: Array<string>, notes: Array<NoteView>, };

export type MilestoneView = { id: string, title: string, 
/**
 * Tasks done, over tasks not dropped (ADR 0029).
 */
done: number, of: number, tasks: Array<TaskView>, };

export type TodoItemView = { id: string, text: string, task: string | null, done: boolean, };

export type SessionView = { id: string, agent: string, objective: string, tasks: Array<string>, open: boolean, 
/**
 * UTC, RFC 3339.
 */
startedAt: string, endedAt: string | null, summary: string | null, 
/**
 * Its todo: the live one while it is open.
 */
todo: Array<TodoItemView>, 
/**
 * What it changed in the code: since its start while it is open; `null` for a session
 * that ended without it.
 */
changes: SessionChanges | null, };

export type TrackingProblem = { 
/**
 * Relative to `.timonier/`.
 */
file: string, message: string, };

export type LookItem = { "kind": "problem", file: string, } | { "kind": "blocked", task: string, title: string, } | { "kind": "cycle", boxes: Array<ReviewBox>, path: Array<string>, } | { "kind": "debt", item: ReviewBox, debt: NodeDebt, path: Array<string>, };

export type TrackingView = { milestones: Array<MilestoneView>, 
/**
 * The open session's id; `null` between sessions.
 */
open: string | null, todo: Array<TodoItemView>, 
/**
 * The latest sessions first, at most [`JOURNAL_LIMIT`].
 */
sessions: Array<SessionView>, 
/**
 * How many sessions the journal holds in all.
 */
sessionsTotal: number, 
/**
 * The files that do not read: what they hold is not shown.
 */
problems: Array<TrackingProblem>, 
/**
 * The review diff since the last review, for the bridge (ADR 0032).
 */
review: ReviewSummary, 
/**
 * The review debt of the whole project; `null` when nothing is to read again.
 */
debt: NodeDebt | null, 
/**
 * The sessions started since the last review; all of them before the first.
 */
sessionsSinceReview: number, 
/**
 * What to look at first (Q45), at most [`LOOK_LIMIT`].
 */
look: Array<LookItem>, };

export type TrackingSnapshot = { subscription: number, revision: number, tracking: TrackingView, };

export type Pin = { 
/**
 * The id of the container: a role group, an open box, or `@root` for the map itself.
 */
parent: string, x: number, y: number, };

export type Direction = "left" | "right" | "up" | "down";

export type Density = "comfortable" | "compact";

export type DaemonState = "starting" | "connected" | "restarting" | "unavailable";

export type MapCommand = { "name": "fitView" } | { "name": "zoomIn" } | { "name": "zoomOut" } | { "name": "closeAll" } | { "name": "toggle", target: string | null, } | { "name": "openCode", target: string | null, } | { "name": "move", target: string | null, direction: Direction, } | { "name": "resetLayout" } | { "name": "toggleChanges" } | { "name": "toggleReviewDetails" } | { "name": "toggleDebt" } | { "name": "reveal", target: string, path: Array<string>, };

export type HostMessage = { "type": "snapshot", snapshot: ViewSnapshot, } | { "type": "delta", delta: ViewDelta, } | { "type": "daemon", state: DaemonState, 
/**
 * Why, for a person: the error, the program looked for.
 */
detail: string | null, } | { "type": "command", command: MapCommand, } | { "type": "pins", pins: Record<string, Pin>, } | { "type": "density", density: Density, } | { "type": "tracking", revision: number, tracking: TrackingView, };

export type WebviewMessage = { "type": "ready", protocol: number, } | { "type": "subscribe", view: ViewSpec, } | { "type": "resync" } | { "type": "displayed", paths: Array<string>, } | { "type": "openCode", file: string, span: Span | null, } | { "type": "pins", pins: Record<string, Pin>, } | { "type": "markReviewed" } | { "type": "reveal", target: string, path: Array<string>, } | { "type": "openMap" } | { "type": "trackingShown", panel: string, revision: number, } | { "type": "rendered", subscription: number, revision: number, nodes: number, };

export const METHODS = {
  INITIALIZE: "initialize",
  STATUS: "daemon/status",
  SHUTDOWN: "daemon/shutdown",
  SUBSCRIBE: "view/subscribe",
  UPDATE: "view/update",
  RESYNC: "view/resync",
  UNSUBSCRIBE: "view/unsubscribe",
  OPENED: "editor/opened",
  DISPLAYED: "view/displayed",
  SAVED: "editor/saved",
  OUTGOING_CALLS: "lsp/outgoingCalls",
  DEFINITIONS: "lsp/definitions",
  REFERENCES: "lsp/references",
  REVIEW_MARK: "review/mark",
  TRACKING_SUBSCRIBE: "tracking/subscribe",
  TRACKING_UPDATE: "tracking/update",
} as const;

export const ERRORS = {
  REQUEST_FAILED: -32803,
  CONTENT_MODIFIED: -32801,
} as const;
