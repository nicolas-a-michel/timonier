// Generated from crates/timonier-core/src/protocol.rs by its test `typescript_is_up_to_date`.
// Do not edit: change the Rust types, then run `TIMONIER_UPDATE_TS=1 cargo test -p timonier-core`.

export const PROTOCOL_VERSION = 2;

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
lookups: LookupCounts, };

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
file: string | null, span: Span | null, };

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
approximateOnly: boolean, };

export type EdgeRef = { src: string, dst: string, kind: EdgeKind, };

export type ViewSnapshot = { subscription: number, revision: number, nodes: Array<ViewNode>, edges: Array<ViewEdge>, };

export type ViewDelta = { subscription: number, base: number, revision: number, 
/**
 * Nodes added or changed.
 */
nodes: Array<ViewNode>, removedNodes: string[], 
/**
 * Edges added or changed.
 */
edges: Array<ViewEdge>, removedEdges: Array<EdgeRef>, };

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

export type Pin = { 
/**
 * The id of the container: a role group, an open box, or `@root` for the map itself.
 */
parent: string, x: number, y: number, };

export type Direction = "left" | "right" | "up" | "down";

export type DaemonState = "starting" | "connected" | "restarting" | "unavailable";

export type MapCommand = { "name": "fitView" } | { "name": "zoomIn" } | { "name": "zoomOut" } | { "name": "closeAll" } | { "name": "toggle", target: string | null, } | { "name": "openCode", target: string | null, } | { "name": "move", target: string | null, direction: Direction, } | { "name": "resetLayout" };

export type HostMessage = { "type": "snapshot", snapshot: ViewSnapshot, } | { "type": "delta", delta: ViewDelta, } | { "type": "daemon", state: DaemonState, 
/**
 * Why, for a person: the error, the program looked for.
 */
detail: string | null, } | { "type": "command", command: MapCommand, } | { "type": "pins", pins: Record<string, Pin>, };

export type WebviewMessage = { "type": "ready", protocol: number, } | { "type": "subscribe", view: ViewSpec, } | { "type": "resync" } | { "type": "displayed", paths: Array<string>, } | { "type": "openCode", file: string, span: Span | null, } | { "type": "pins", pins: Record<string, Pin>, } | { "type": "rendered", subscription: number, revision: number, nodes: number, };

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
} as const;

export const ERRORS = {
  REQUEST_FAILED: -32803,
  CONTENT_MODIFIED: -32801,
} as const;
