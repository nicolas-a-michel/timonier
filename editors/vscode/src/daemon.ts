import { spawn, type ChildProcess } from "node:child_process";
import { createConnection, type Socket } from "node:net";
import { createInterface } from "node:readline";

import * as vscode from "vscode";
import {
  createMessageConnection,
  SocketMessageReader,
  SocketMessageWriter,
  type CancellationToken,
  type MessageConnection,
} from "vscode-jsonrpc/node";

import { backoff, parseAnnounce } from "./launch";
import { definitions, openedFiles, outgoingCalls, references } from "./lspRelay";
import { SavedDocuments } from "./savedDocuments";
import {
  METHODS,
  PROTOCOL_VERSION,
  type DaemonState,
  type IndexProgress,
  type StatusResult,
  type DefinitionsParams,
  type InitializeResult,
  type OutgoingCallsParams,
  type ReferencesParams,
  type TrackingSnapshot,
  type ViewDelta,
} from "./protocol/generated";

/** A connection that lived this long was not a failure: the next loss waits 1 s again. */
const STABLE_MS = 30_000;

export interface DaemonStatus {
  state: DaemonState;
  detail: string | null;
  /** How far the first indexing is, while it runs (T-115). */
  progress?: IndexProgress;
}

/** How often the start's progress is asked, while the first indexing runs. */
const PROGRESS_MS = 300;

/**
 * Launches the project's daemon, connects to it, and launches it again when it is lost. The
 * daemon outlives the window: other clients (the CLI, an agent's MCP proxy) may use it, and it
 * stops alone once nobody is connected (ADR 0019).
 */
/** How long the extension waits for the named pipe of a daemon that announced itself (T-125). */
const PIPE_WAIT_MS = 5_000;

export class DaemonSupervisor implements vscode.Disposable {
  private readonly stateEmitter = new vscode.EventEmitter<DaemonStatus>();
  private readonly connectEmitter = new vscode.EventEmitter<MessageConnection>();
  private readonly deltaEmitter = new vscode.EventEmitter<ViewDelta>();
  private readonly trackingEmitter = new vscode.EventEmitter<TrackingSnapshot>();
  /** The state, then each change. */
  readonly onDidChangeState = this.stateEmitter.event;
  /** Each connection, once initialized: subscriptions are made again on it. */
  readonly onDidConnect = this.connectEmitter.event;
  readonly onDidReceiveDelta = this.deltaEmitter.event;
  /** The plan, the todo and the journal, whole, at each change (ADR 0031). */
  readonly onDidReceiveTracking = this.trackingEmitter.event;

  private current: DaemonStatus = { state: "starting", detail: null };
  private connection: MessageConnection | undefined;
  private socket: Socket | undefined;
  private child: ChildProcess | undefined;
  private failures = 0;
  private retry: NodeJS.Timeout | undefined;
  private stable: NodeJS.Timeout | undefined;
  private daemonPid: number | undefined;
  private progressTimer: NodeJS.Timeout | undefined;
  private disposed = false;
  private attempt = 0;
  /** The relay answers about saved content only (ADR 0022). */
  private readonly saved: SavedDocuments;

  constructor(
    private readonly program: string,
    private readonly root: string,
    private readonly log: vscode.LogOutputChannel,
    private readonly env: NodeJS.ProcessEnv = process.env,
  ) {
    // What the daemon held while documents differed from the disk can be asked again.
    this.saved = new SavedDocuments(root, () => {
      void this.client?.sendNotification(METHODS.SAVED, {}).catch(() => undefined);
    });
  }

  get status(): DaemonStatus {
    return this.current;
  }

  /** The process that serves, when this window launched it or learnt it. */
  get pid(): number | undefined {
    return this.daemonPid;
  }

  get client(): MessageConnection | undefined {
    return this.current.state === "connected" ? this.connection : undefined;
  }

  start(): void {
    this.launch();
  }

  /** Stops supervising and asks the daemon to stop: for the tests' cleanup. */
  async stop(): Promise<void> {
    const connection = this.client;
    this.disposed = true;
    clearTimeout(this.retry);
    if (connection) {
      await connection.sendRequest(METHODS.SHUTDOWN).catch(() => undefined);
    }
    this.dispose();
  }

  /** Launches again now, without waiting for the delay. */
  restart(): void {
    if (this.disposed) return;
    this.failures = 0;
    this.drop();
    this.launch();
  }

  dispose(): void {
    this.disposed = true;
    clearTimeout(this.retry);
    clearTimeout(this.stable);
    clearTimeout(this.progressTimer);
    this.drop();
    this.saved.dispose();
    this.stateEmitter.dispose();
    this.connectEmitter.dispose();
    this.deltaEmitter.dispose();
    this.trackingEmitter.dispose();
  }

  /** The files open in the editor come first in the refinement queue (ADR 0014). */
  announceEditors(): void {
    const paths = openedFiles(this.root);
    if (paths.length === 0) return;
    void this.client?.sendNotification(METHODS.OPENED, { paths }).catch(() => undefined);
  }

  private setState(state: DaemonState, detail: string | null = null): void {
    if (this.disposed) return;
    clearTimeout(this.progressTimer);
    this.current = { state, detail };
    this.stateEmitter.fire(this.current);
  }

  /**
   * Until the first indexing is done, its progress, a few times a second (T-115): the pages
   * show a bar instead of an empty screen. Ends with the connection.
   */
  private followIndexing(connection: MessageConnection, attempt: number): void {
    const ask = () => {
      void connection
        .sendRequest<StatusResult>(METHODS.STATUS)
        .then((status) => {
          if (attempt !== this.attempt || this.disposed || this.current.state !== "connected") return;
          const progress = status.progress ?? undefined;
          if (progress?.done !== this.current.progress?.done || progress?.total !== this.current.progress?.total) {
            this.current = { ...this.current, progress };
            this.stateEmitter.fire(this.current);
          }
          if (progress) this.progressTimer = setTimeout(ask, PROGRESS_MS);
        })
        .catch(() => undefined);
    };
    ask();
  }

  private launch(): void {
    if (this.disposed) return;
    clearTimeout(this.retry);
    const attempt = ++this.attempt;
    this.setState(this.failures === 0 ? "starting" : "restarting");
    this.log.info(`Launching ${this.program} daemon run --root ${this.root}`);
    let child: ChildProcess;
    try {
      // Detached (ADR 0019): in its own process group, the daemon outlives the window, and other
      // clients keep it; it stops on its own once nobody uses it.
      child = spawn(this.program, ["daemon", "run", "--root", this.root], {
        env: this.env,
        stdio: ["ignore", "pipe", "pipe"],
        detached: true,
      });
      child.unref();
    } catch (error) {
      this.lost(attempt, `cannot launch ${this.program}: ${message(error)}`, true);
      return;
    }
    this.child = child;
    let announced = false;
    child.on("error", (error: NodeJS.ErrnoException) => {
      const missing = error.code === "ENOENT";
      const detail = missing
        ? vscode.l10n.t("The program {0} was not found. Set timonier.daemon.path.", this.program)
        : message(error);
      this.lost(attempt, detail, missing);
    });
    if (child.stderr) {
      createInterface({ input: child.stderr }).on("line", (line) => this.log.info(`[daemon] ${line}`));
    }
    if (child.stdout) {
      createInterface({ input: child.stdout }).once("line", (line) => {
        announced = true;
        const announce = parseAnnounce(line);
        if (!announce) {
          this.lost(attempt, `unexpected output from the daemon: ${line}`);
          return;
        }
        this.daemonPid = announce.pid ?? undefined;
        this.connect(attempt, announce.socket);
      });
    }
    child.on("exit", (code, signal) => {
      if (this.child === child) this.child = undefined;
      // `run` leaves at once when another process serves: that is not a loss.
      if (announced && code === 0) return;
      if (!announced) this.lost(attempt, `the daemon stopped before listening (${signal ?? code})`);
    });
  }

  private connect(attempt: number, path: string, since = Date.now()): void {
    const socket = createConnection(path);
    this.socket = socket;
    socket.once("error", (error) => {
      // Windows: the daemon makes its named pipe right after it announces itself (T-125).
      const notYet = (error as NodeJS.ErrnoException).code === "ENOENT" && process.platform === "win32";
      if (notYet && Date.now() - since < PIPE_WAIT_MS && attempt === this.attempt) {
        setTimeout(() => this.connect(attempt, path, since), 50);
        return;
      }
      this.lost(attempt, `cannot reach the daemon: ${message(error)}`);
    });
    socket.once("connect", () => {
      const connection = createMessageConnection(new SocketMessageReader(socket), new SocketMessageWriter(socket));
      this.connection = connection;
      connection.onNotification(METHODS.UPDATE, (delta: ViewDelta) => this.deltaEmitter.fire(delta));
      connection.onNotification(METHODS.TRACKING_UPDATE, (update: TrackingSnapshot) =>
        this.trackingEmitter.fire(update),
      );
      // The IDE's language servers, relayed to the daemon's refinement (ADR 0014).
      // Their answers hold only for the content on disk: see `SavedDocuments`.
      connection.onRequest(METHODS.OUTGOING_CALLS, (params: OutgoingCallsParams) =>
        this.saved.answer((documents) => outgoingCalls(this.root, params, documents)),
      );
      connection.onRequest(METHODS.DEFINITIONS, (params: DefinitionsParams, token: CancellationToken) =>
        this.saved.answer((documents) => definitions(this.root, params, documents, token)),
      );
      connection.onRequest(METHODS.REFERENCES, (params: ReferencesParams) =>
        this.saved.answer((documents) => references(this.root, params, documents)),
      );
      connection.onClose(() => this.lost(attempt, "the connection to the daemon closed"));
      connection.listen();
      connection
        .sendRequest<InitializeResult>(METHODS.INITIALIZE, {
          protocolVersion: PROTOCOL_VERSION,
          client: `vscode ${vscode.version}`,
          lspRelay: true,
        })
        .then((result) => {
          if (attempt !== this.attempt || this.disposed) return;
          this.log.info(`Connected to the daemon ${result.daemonVersion} for ${result.root}`);
          this.saved.setSources(result.sourceExtensions);
          this.setState("connected");
          clearTimeout(this.stable);
          this.stable = setTimeout(() => (this.failures = 0), STABLE_MS);
          this.connectEmitter.fire(connection);
          this.announceEditors();
          this.followIndexing(connection, attempt);
        })
        .catch((error: unknown) => this.lost(attempt, `the daemon refused the connection: ${message(error)}`));
    });
  }

  /** Attempt `attempt` failed or its daemon is gone: launch again after the delay. */
  private lost(attempt: number, detail: string, unavailable = false): void {
    if (this.disposed || attempt !== this.attempt) return;
    this.attempt++;
    clearTimeout(this.stable);
    this.drop();
    this.failures++;
    const delay = backoff(this.failures);
    this.log.warn(`${detail}; next launch in ${delay / 1000} s`);
    this.setState(unavailable ? "unavailable" : "restarting", detail);
    this.retry = setTimeout(() => this.launch(), delay);
  }

  private drop(): void {
    this.connection?.dispose();
    this.connection = undefined;
    this.socket?.destroy();
    this.socket = undefined;
    // A daemon launched here keeps running for the other clients: only its pipes go.
    this.child?.stdout?.destroy();
    this.child?.stderr?.destroy();
    this.child = undefined;
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
