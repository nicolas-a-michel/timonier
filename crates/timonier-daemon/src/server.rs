//! The daemon's JSON-RPC 2.0 server on its Unix socket (ADR 0019).
//!
//! The pipeline keeps its thread (ADR 0013), the views theirs (`subscriptions`); the tokio tasks
//! only read and write the clients' sockets. One task per client reads its frames and answers;
//! another writes its queue, so that a slow reader never holds the daemon.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use timonier_core::SnapshotKind;
use timonier_core::protocol::{
    FilesParams, InitializeParams, InitializeResult, LookupCounts, PROTOCOL_VERSION, ReviewMark,
    StatusResult, SubscribeParams, SubscriptionParams, ToolCallParams, errors, methods,
};
use timonier_core::rpc::{Decoder, ErrorObject, Id, InvalidMessage, Message};
use timonier_index::lsp::Priority;
use timonier_store::Store;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Notify, mpsc, watch};
use tokio::task::JoinSet;

use crate::relay::Relay;
use crate::runtime::{self, Lock, Paths};
use crate::subscriptions::{self, Command, Outbox};
use crate::watch::{DEBOUNCE, Event, Watch};
use crate::{Error, Pipeline, Refinement, Result};

/// Without any client for this long, the daemon stops (ADR 0019).
pub const IDLE: Duration = Duration::from_secs(15 * 60);

/// Frames waiting for one client. Beyond, deltas are dropped (the client resyncs).
const OUTBOX: usize = 1024;

/// Commands waiting for the views thread.
const VIEW_COMMANDS: usize = 256;

/// Bytes read from a socket at once.
const READ_CHUNK: usize = 64 << 10;

/// How a daemon runs.
#[derive(Clone, Debug)]
pub struct Options {
    /// The repository.
    pub root: std::path::PathBuf,
    /// Where the socket and the lock go; `None`: [`runtime::runtime_dir`].
    pub runtime: Option<std::path::PathBuf>,
    pub idle: Duration,
    pub debounce: Duration,
    /// Stop on SIGTERM and SIGINT. Off in tests, which share one process.
    pub signals: bool,
}

impl Options {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            root: root.into(),
            runtime: None,
            idle: IDLE,
            debounce: DEBOUNCE,
            signals: true,
        }
    }
}

/// A daemon that holds its project's lock and listens on its socket, before it serves.
pub struct Daemon {
    options: Options,
    paths: Paths,
    listener: std::os::unix::net::UnixListener,
    pipeline: Pipeline,
    store: Store,
    // Dropped last: held as long as the socket is served.
    _lock: Lock,
}

impl Daemon {
    /// Takes the project's lock, then listens on its socket and opens its database. `None`:
    /// another daemon serves this project.
    pub fn bind(options: Options) -> Result<Option<Self>> {
        let runtime = options.runtime.clone().unwrap_or_else(runtime::runtime_dir);
        let paths = Paths::in_runtime(&options.root, &runtime)?;
        let Some(lock) = Lock::acquire(&paths)? else {
            return Ok(None);
        };
        // The lock is ours: a socket left there belongs to a daemon that died.
        match std::fs::remove_file(&paths.socket) {
            Ok(()) => tracing::info!(socket = %paths.socket.display(), "stale socket replaced"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    path: paths.socket.clone(),
                    source,
                });
            }
        }
        let io = |source| Error::Io {
            path: paths.socket.clone(),
            source,
        };
        let listener = std::os::unix::net::UnixListener::bind(&paths.socket).map_err(io)?;
        listener.set_nonblocking(true).map_err(io)?;
        runtime::prepare_repository(&paths)?;
        let store = Store::open(&paths.database)?;
        let pipeline = Pipeline::new(&paths.root, store.clone())?;
        Ok(Some(Self {
            options,
            paths,
            listener,
            pipeline,
            store,
            _lock: lock,
        }))
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// [`Daemon::serve`] on a runtime of its own, until the daemon stops: what
    /// `timonier daemon run` does.
    pub fn run(self) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("timonier-rpc")
            .build()
            .map_err(Error::Runtime)?;
        runtime.block_on(self.serve())
    }

    /// Indexes, watches and serves until `daemon/shutdown`, a signal, or [`Options::idle`]
    /// without any client. Then stops everything, removes the socket and releases the lock.
    pub async fn serve(self) -> Result<()> {
        let Self {
            options,
            paths,
            listener,
            pipeline,
            store,
            _lock,
        } = self;
        let socket_error = |source| Error::Io {
            path: paths.socket.clone(),
            source,
        };
        let listener = UnixListener::from_std(listener).map_err(socket_error)?;
        let (shutdown, _) = watch::channel(false);
        let changed = Arc::new(AtomicBool::new(false));
        let (views, commands) = mpsc::channel(VIEW_COMMANDS);
        let watching = Arc::new(AtomicBool::new(false));
        let views_thread = subscriptions::spawn(
            store.clone(),
            paths.root.clone(),
            Arc::clone(&changed),
            Arc::clone(&watching),
            commands,
        )
        .map_err(Error::Thread)?;
        let files = Arc::new(AtomicUsize::new(0));
        let indexed = Arc::new(AtomicBool::new(false));
        let lookups: Arc<[AtomicU32; 2]> = Arc::new([AtomicU32::new(0), AtomicU32::new(0)]);
        let on_event = {
            let views = views.clone();
            let files = Arc::clone(&files);
            let indexed = Arc::clone(&indexed);
            let lookups = Arc::clone(&lookups);
            move |event: Event| {
                if let Event::Refined(Ok(Refinement::LookedUp { by_name, .. })) = &event {
                    lookups[usize::from(*by_name)].fetch_add(1, Ordering::SeqCst);
                }
                if let Event::Indexed(Ok(report)) = &event {
                    files.store(report.index.files, Ordering::SeqCst);
                    indexed.store(true, Ordering::SeqCst);
                }
                if event.changes_graph() {
                    changed.store(true, Ordering::SeqCst);
                    // Full: the views thread is busy and sees the flag afterwards.
                    let _ = views.try_send(Command::Changed);
                }
            }
        };
        let debounce = options.debounce;
        let source_extensions = pipeline.indexer().packs().extensions();
        // Refinement asks the client that relays its IDE's language servers (ADR 0014).
        let relay = Relay::default();
        let server = Box::new(relay.server());
        let watch = tokio::task::spawn_blocking(move || {
            Watch::start(pipeline, debounce, Some(server), on_event)
        })
        .await
        .map_err(|e| Error::Join(e.to_string()))??;
        tracing::info!(root = %paths.root.display(), socket = %paths.socket.display(), "daemon serving");

        let shared = Arc::new(Shared {
            root: paths.root.display().to_string(),
            root_path: paths.root.clone(),
            store,
            views,
            watch: Mutex::new(Some(watch)),
            files,
            indexed,
            clients: AtomicUsize::new(0),
            idle_since: Mutex::new(Some(Instant::now())),
            shutdown,
            next_connection: AtomicU64::new(1),
            relay,
            lookups,
            source_extensions,
            tracking: Mutex::new(()),
        });
        let mut background = JoinSet::new();
        background.spawn(idle(Arc::clone(&shared), options.idle));
        background.spawn(watch_tracking(Arc::clone(&shared), watching));
        if options.signals {
            background.spawn(signals(Arc::clone(&shared)));
        }
        let mut connections = JoinSet::new();
        let mut stop = shared.shutdown.subscribe();
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        connections.spawn(connection(Arc::clone(&shared), stream));
                    }
                    Err(error) => tracing::warn!(%error, "cannot accept a client"),
                },
                Some(_) = connections.join_next(), if !connections.is_empty() => {}
                () = stopping(&mut stop) => break,
            }
        }

        tracing::info!("daemon stopping");
        drop(listener);
        let _ = std::fs::remove_file(&paths.socket);
        background.abort_all();
        // Clients see the shutdown and finish writing what they were sent.
        let drained = tokio::time::timeout(Duration::from_secs(2), async {
            while connections.join_next().await.is_some() {}
        });
        if drained.await.is_err() {
            connections.shutdown().await;
        }
        let watch = shared.watch.lock().map_err(|_| Error::Poisoned)?.take();
        let views = shared.views.clone();
        drop(shared);
        tokio::task::spawn_blocking(move || {
            // The pipeline finishes its step; then the views thread stops.
            if let Some(watch) = watch {
                watch.stop();
            }
            let _ = views.blocking_send(Command::Stop);
            let _ = views_thread.join();
        })
        .await
        .map_err(|e| Error::Join(e.to_string()))?;
        tracing::info!("daemon stopped");
        Ok(())
    }
}

struct Shared {
    root: String,
    /// The same, as a path: where `.timonier/` is.
    root_path: std::path::PathBuf,
    store: Store,
    views: mpsc::Sender<Command>,
    watch: Mutex<Option<Watch>>,
    files: Arc<AtomicUsize>,
    /// The first indexing is done.
    indexed: Arc<AtomicBool>,
    clients: AtomicUsize,
    /// Since when no client is connected.
    idle_since: Mutex<Option<Instant>>,
    shutdown: watch::Sender<bool>,
    next_connection: AtomicU64,
    relay: Relay,
    /// Lookups of dependents: named by the server, then by name.
    lookups: Arc<[AtomicU32; 2]>,
    /// The file extensions the packs read.
    source_extensions: Vec<String>,
    /// One tool call of the tracking at a time: the daemon is the only writer (ADR 0029).
    tracking: Mutex<()>,
}

impl Shared {
    fn joined(&self) {
        self.clients.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut since) = self.idle_since.lock() {
            *since = None;
        }
    }

    fn left(&self) {
        if self.clients.fetch_sub(1, Ordering::SeqCst) == 1
            && let Ok(mut since) = self.idle_since.lock()
        {
            *since = Some(Instant::now());
        }
    }

    fn idle_for(&self) -> Duration {
        match self.idle_since.lock() {
            Ok(since) => since.map_or(Duration::ZERO, |s| s.elapsed()),
            Err(_) => Duration::ZERO,
        }
    }

    fn stop(&self) {
        self.shutdown.send_replace(true);
    }
}

/// Until the daemon stops. The value's guard does not live on: it would make the task's future
/// unsendable.
async fn stopping(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopping| *stopping).await;
}

async fn idle(shared: Arc<Shared>, idle: Duration) {
    let tick = (idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(30));
    loop {
        tokio::time::sleep(tick).await;
        if shared.idle_for() >= idle {
            tracing::info!(idle_s = idle.as_secs(), "no client: stopping");
            shared.stop();
            return;
        }
    }
}

/// While a tracking view is open, looks at `.timonier/` every second: a change made outside the
/// daemon (by hand, by git) reaches the panels too (ADR 0031). The watcher of the code skips it.
async fn watch_tracking(shared: Arc<Shared>, watching: Arc<AtomicBool>) {
    let mut last = None;
    loop {
        tokio::time::sleep(TRACKING_POLL).await;
        if !watching.load(Ordering::SeqCst) {
            last = None;
            continue;
        }
        let dir = shared.root_path.join(".timonier");
        let Ok(now) = tokio::task::spawn_blocking(move || tracking_files(&dir)).await else {
            continue;
        };
        // The first look may come after a change: the views thread sends only what differs.
        if last.as_ref() != Some(&now) {
            // Full: the views thread is busy, and looks again at the next tick.
            if shared.views.try_send(Command::Tracked).is_err() {
                continue;
            }
        }
        last = Some(now);
    }
}

/// A tool that could not run because the daemon failed: an `internal` error, with the header.
fn infrastructure_failure(error: &dyn std::fmt::Display) -> Value {
    timonier_core::tools::failure(
        timonier_core::tools::Mailbox::default(),
        &timonier_core::tracking::Refusal::new(
            "internal",
            format!("the daemon failed ({error}): try again, or run `timonier doctor`"),
        ),
    )
}

/// How often `.timonier/` is looked at while a tracking view is open.
const TRACKING_POLL: Duration = Duration::from_secs(1);

/// The size and time of each tracking file: what says one changed.
fn tracking_files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64, Option<SystemTime>)> {
    let stat = |path: std::path::PathBuf| {
        let meta = std::fs::metadata(&path).ok();
        let (len, modified) = meta.map_or((0, None), |m| (m.len(), m.modified().ok()));
        (path, len, modified)
    };
    // `init.json`: the bridge offers `timonier init` until it is there (Q52).
    let mut files: Vec<_> = ["plan.json", "todo.json", "init.json"]
        .iter()
        .map(|name| stat(dir.join(name)))
        .collect();
    let mut sessions: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    sessions.sort();
    files.extend(sessions.into_iter().map(stat));
    files
}

async fn signals(shared: Arc<Shared>) {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut terminate), Ok(mut interrupt)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) else {
        tracing::warn!("cannot listen to signals");
        return;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
    tracing::info!("signal received: stopping");
    shared.stop();
}

/// One client, from its connection to its end.
async fn connection(shared: Arc<Shared>, stream: UnixStream) {
    let id = shared.next_connection.fetch_add(1, Ordering::SeqCst);
    shared.joined();
    tracing::debug!(connection = id, "client connected");
    let (mut read, mut write) = stream.into_split();
    let (outbox, mut queue) = mpsc::channel::<Vec<u8>>(OUTBOX);
    // Set by the views thread when a delta did not fit: once the queue drains, it hears so.
    let congested = Arc::new(AtomicBool::new(false));
    let writer = {
        let congested = Arc::clone(&congested);
        let views = shared.views.clone();
        tokio::spawn(async move {
            while let Some(frame) = queue.recv().await {
                if write.write_all(&frame).await.is_err() {
                    break;
                }
                // Waits for room in the views thread's queue: without a frame to come, a
                // signal dropped here would never be sent again. A views thread gone means
                // the daemon stops: nothing to signal then.
                if queue.is_empty() && congested.swap(false, Ordering::SeqCst) {
                    let _ = views.send(Command::Drained { connection: id }).await;
                }
            }
            let _ = write.shutdown().await;
        })
    };
    let close = Arc::new(Notify::new());
    let open = Command::Open {
        connection: id,
        outbox: outbox.clone(),
        close: Arc::clone(&close),
        congested,
    };
    if shared.views.send(open).await.is_ok() {
        let mut session = Session {
            shared: Arc::clone(&shared),
            connection: id,
            outbox,
            initialized: false,
            client: String::new(),
        };
        let mut stop = shared.shutdown.subscribe();
        let mut decoder = Decoder::default();
        let mut buffer = vec![0; READ_CHUNK];
        'read: loop {
            tokio::select! {
                read = read.read(&mut buffer) => {
                    let n = match read {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    decoder.push(&buffer[..n]);
                    loop {
                        match decoder.next_body() {
                            Ok(Some(body)) => session.handle(&body).await,
                            Ok(None) => break,
                            Err(error) => {
                                tracing::warn!(connection = id, %error, "unreadable frames: client closed");
                                break 'read;
                            }
                        }
                    }
                }
                _ = close.notified() => break,
                () = stopping(&mut stop) => break,
            }
        }
        if shared.relay.detach(id) {
            // Another client relays now: what this one held back may be asked of it.
            nudge(&shared);
        }
        let _ = shared.views.send(Command::Closed { connection: id }).await;
        drop(session);
    } else {
        // No views thread: the queue goes now, or the writer would wait for it forever.
        drop(outbox);
    }
    // The views thread dropped its copy of the queue: the writer ends with the last frame.
    let _ = writer.await;
    shared.left();
    tracing::debug!(connection = id, "client gone");
}

struct Session {
    shared: Arc<Shared>,
    connection: u64,
    outbox: Outbox,
    initialized: bool,
    /// Who connected: the agent of the sessions it opens (ADR 0029).
    client: String,
}

impl Session {
    async fn handle(&mut self, body: &[u8]) {
        match Message::parse(body) {
            Ok(Message::Request { id, method, params }) => self.request(id, &method, params).await,
            Ok(Message::Notification { method, params }) => {
                self.notification(&method, params).await
            }
            Ok(Message::Response { id, outcome }) => {
                let answered = id
                    .as_ref()
                    .is_some_and(|id| self.shared.relay.answer(self.connection, id, outcome));
                if !answered {
                    tracing::debug!(
                        connection = self.connection,
                        ?id,
                        "unexpected response ignored"
                    );
                }
            }
            Err(InvalidMessage::Json(error)) => {
                let error = ErrorObject::new(errors::PARSE, error);
                self.send(Message::error(None, error)).await;
            }
            Err(InvalidMessage::Shape(error)) => {
                let error = ErrorObject::new(errors::INVALID_REQUEST, error);
                self.send(Message::error(Message::id_of(body), error)).await;
            }
        }
    }

    async fn send(&self, message: Message) {
        // Closed: the client is gone, and its reader ends too.
        let _ = self.outbox.send(message.frame()).await;
    }

    async fn request(&mut self, id: Id, method: &str, params: Value) {
        if !self.initialized && method != methods::INITIALIZE {
            let error = ErrorObject::new(errors::NOT_INITIALIZED, "`initialize` first");
            return self.send(Message::error(Some(id), error)).await;
        }
        let connection = self.connection;
        let outcome = match method {
            methods::INITIALIZE => match parse::<InitializeParams>(params) {
                Ok(params) => self.initialize(params),
                Err(error) => Err(error),
            },
            methods::STATUS => self.status().await,
            methods::REVIEW_MARK => self.mark_reviewed().await,
            methods::TOOLS_LIST => {
                Ok(serde_json::json!({ "tools": timonier_core::tools::catalogue() }))
            }
            methods::AGENT_STOP => self.agent_stop().await,
            methods::ACTUAL_WRITE => self.write_actual().await,
            methods::TOOLS_CALL => match parse::<ToolCallParams>(params) {
                Ok(params) => self.call_tool(params).await,
                Err(error) => Err(error),
            },
            methods::SHUTDOWN => {
                self.send(Message::result(id, Value::Null)).await;
                tracing::info!(connection, "shutdown asked");
                self.shared.stop();
                return;
            }
            methods::SUBSCRIBE => match parse::<SubscribeParams>(params) {
                Ok(params) => {
                    let command = Command::Subscribe {
                        connection,
                        id: id.clone(),
                        spec: params.view,
                    };
                    // The views thread answers, in order with the deltas.
                    if self.to_views(command).await {
                        return;
                    }
                    Err(shutting_down())
                }
                Err(error) => Err(error),
            },
            methods::TRACKING_SUBSCRIBE => {
                let command = Command::SubscribeTracking {
                    connection,
                    id: id.clone(),
                };
                if self.to_views(command).await {
                    return;
                }
                Err(shutting_down())
            }
            methods::RESYNC | methods::UNSUBSCRIBE => match parse::<SubscriptionParams>(params) {
                Ok(SubscriptionParams { subscription }) => {
                    let id = id.clone();
                    let command = if method == methods::RESYNC {
                        Command::Resync {
                            connection,
                            id,
                            subscription,
                        }
                    } else {
                        Command::Unsubscribe {
                            connection,
                            id,
                            subscription,
                        }
                    };
                    if self.to_views(command).await {
                        return;
                    }
                    Err(shutting_down())
                }
                Err(error) => Err(error),
            },
            _ => Err(ErrorObject::new(
                errors::METHOD_NOT_FOUND,
                format!("no method `{method}`"),
            )),
        };
        let message = match outcome {
            Ok(result) => Message::result(id, result),
            Err(error) => Message::error(Some(id), error),
        };
        self.send(message).await;
    }

    async fn to_views(&self, command: Command) -> bool {
        self.shared.views.send(command).await.is_ok()
    }

    fn initialize(&mut self, params: InitializeParams) -> Result<Value, ErrorObject> {
        if params.protocol_version != PROTOCOL_VERSION {
            let error = ErrorObject::new(
                errors::VERSION_REFUSED,
                format!(
                    "this daemon speaks protocol {PROTOCOL_VERSION}, the client {}",
                    params.protocol_version
                ),
            )
            .with_data(serde_json::json!({
                "protocolVersion": PROTOCOL_VERSION,
                "daemonVersion": timonier_core::VERSION,
            }));
            return Err(error);
        }
        self.initialized = true;
        self.client = params.client.clone();
        tracing::info!(connection = self.connection, client = %params.client, relay = params.lsp_relay, "client initialized");
        if params.lsp_relay {
            self.shared
                .relay
                .attach(self.connection, self.outbox.clone());
            // Refinement waited for a server: it can go on.
            nudge(&self.shared);
        }
        Ok(json(&InitializeResult {
            protocol_version: PROTOCOL_VERSION,
            daemon_version: timonier_core::VERSION.to_owned(),
            root: self.shared.root.clone(),
            source_extensions: self.shared.source_extensions.clone(),
        }))
    }

    async fn status(&self) -> Result<Value, ErrorObject> {
        let shared = Arc::clone(&self.shared);
        let read = tokio::task::spawn_blocking(move || -> Result<StatusResult> {
            let reader = shared.store.reader()?;
            let head = reader.head()?.seq;
            let last_review = reader.latest(SnapshotKind::ReviewMark)?;
            let (refinements, held) = shared
                .watch
                .lock()
                .map_err(|_| Error::Poisoned)?
                .as_ref()
                .map_or((0, 0), |w| (w.refinements_waiting(), w.refinements_held()));
            Ok(StatusResult {
                root: shared.root.clone(),
                daemon_version: timonier_core::VERSION.to_owned(),
                files: count(shared.files.load(Ordering::SeqCst)),
                indexed: shared.indexed.load(Ordering::SeqCst),
                head,
                refinements_waiting: count(refinements),
                refinements_held: count(held),
                clients: count(shared.clients.load(Ordering::SeqCst)),
                lsp_answers: shared.relay.answers(),
                lookups: LookupCounts {
                    named: shared.lookups[0].load(Ordering::SeqCst),
                    by_name: shared.lookups[1].load(Ordering::SeqCst),
                },
                last_review: last_review.as_ref().map(ReviewMark::from),
            })
        });
        match read.await {
            Ok(Ok(status)) => Ok(json(&status)),
            Ok(Err(error)) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
            Err(error) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
        }
    }

    /// `review/mark`: the pipeline thread freezes the head between two of its steps (ADR 0024).
    async fn mark_reviewed(&self) -> Result<Value, ErrorObject> {
        let shared = Arc::clone(&self.shared);
        let marked = tokio::task::spawn_blocking(move || -> Result<ReviewMark> {
            // Asked with the watch held, waited for without it.
            let pending = shared
                .watch
                .lock()
                .map_err(|_| Error::Poisoned)?
                .as_ref()
                .ok_or(Error::Stopped)?
                .mark_reviewed()?;
            Ok(ReviewMark::from(&pending.wait()?))
        });
        match marked.await {
            Ok(Ok(mark)) => Ok(json(&mark)),
            Ok(Err(error)) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
            Err(error) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
        }
    }

    /// A tool of the tracking (ADR 0027, 0029), run off the runtime: it reads and writes
    /// `.timonier/`, and a session's start or end waits for a snapshot.
    /// Why the agent that wants to stop should be held once, if it should (ADR 0033, Q49). A
    /// tracking file that does not read holds nobody: the tools say what is wrong.
    async fn agent_stop(&self) -> Result<Value, ErrorObject> {
        let shared = Arc::clone(&self.shared);
        let answered = tokio::task::spawn_blocking(move || {
            let tracker = crate::tracking::Tracker::new(&shared.root_path);
            let changed = |from: Option<u64>| -> Result<Vec<String>> {
                let mut reader = shared.store.reader()?;
                let head = reader.head()?.seq;
                crate::tracking::changed_files(&mut reader, from, head)
            };
            let reason = tracker.stop_reason(&changed).unwrap_or_else(|refusal| {
                tracing::warn!(message = %refusal.message, "cannot tell whether to hold the agent");
                None
            });
            timonier_core::protocol::AgentStop { reason }
        })
        .await;
        match answered {
            Ok(stop) => Ok(serde_json::to_value(stop).unwrap_or(Value::Null)),
            Err(error) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
        }
    }

    /// Writes `.timonier/actual.json` from the head, when it changed (ADR 0033).
    async fn write_actual(&self) -> Result<Value, ErrorObject> {
        let shared = Arc::clone(&self.shared);
        let written = tokio::task::spawn_blocking(move || -> Result<_> {
            let mut reader = shared.store.reader()?;
            let graph = crate::view::load(&mut reader)?;
            let actual = timonier_core::actual::actual(&graph);
            let path = shared.root_path.join(".timonier").join("actual.json");
            let mut text = serde_json::to_string_pretty(&actual).unwrap_or_default();
            text.push('\n');
            let changed = std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str());
            if changed {
                let _writing = shared.tracking.lock().map_err(|_| Error::Poisoned)?;
                crate::tracking::Tracker::new(&shared.root_path)
                    .write("actual.json", &actual)
                    .map_err(|refusal| Error::Tracking(refusal.message))?;
            }
            Ok(timonier_core::protocol::ActualWritten {
                path: path.display().to_string(),
                changed,
            })
        })
        .await;
        match written {
            Ok(Ok(written)) => Ok(serde_json::to_value(written).unwrap_or(Value::Null)),
            Ok(Err(error)) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
            Err(error) => Err(ErrorObject::new(errors::INTERNAL, error.to_string())),
        }
    }

    async fn call_tool(&self, params: ToolCallParams) -> Result<Value, ErrorObject> {
        let shared = Arc::clone(&self.shared);
        let agent = self.client.clone();
        let called = tokio::task::spawn_blocking(move || {
            let freeze = || -> Result<timonier_core::Snapshot> {
                let pending = shared
                    .watch
                    .lock()
                    .map_err(|_| Error::Poisoned)?
                    .as_ref()
                    .ok_or(Error::Stopped)?
                    .freeze_working_tree()?;
                pending.wait()
            };
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
            let _writing = shared.tracking.lock().map_err(|_| Error::Poisoned)?;
            let tracker = crate::tracking::Tracker::new(&shared.root_path);
            Ok::<_, Error>(tracker.call(
                &params.name,
                params.arguments,
                &crate::tracking::Context {
                    agent: &agent,
                    now_ms,
                    freeze: &freeze,
                    changed_files: &|from, to| {
                        let mut reader = shared.store.reader()?;
                        crate::tracking::changed_files(&mut reader, from, to)
                    },
                    changes: &|start, end| {
                        let mut reader = shared.store.reader()?;
                        crate::journal::between(&mut reader, start, end)
                    },
                    forget_contents: &|| Ok(shared.store.forget_contents()?),
                },
            ))
        });
        let called = called.await;
        // The panels show what the tool did (ADR 0031); full, the views thread is busy and
        // the watch of `.timonier/` catches up within a second.
        let _ = self.shared.views.try_send(Command::Tracked);
        // A failure of the daemon itself is a tool error too, with the mailbox header (ADR 0028):
        // the agent reads the same shape whatever went wrong.
        Ok(match called {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => infrastructure_failure(&error),
            Err(error) => infrastructure_failure(&error),
        })
    }

    async fn notification(&self, method: &str, params: Value) {
        if !self.initialized {
            return;
        }
        let priority = match method {
            methods::OPENED => Priority::Opened,
            methods::DISPLAYED => Priority::Displayed,
            // What waited for saved documents can be asked again (ADR 0022).
            methods::SAVED => {
                nudge(&self.shared);
                return;
            }
            // `$/cancelRequest` and the like: nothing here takes long enough to cancel.
            _ => return,
        };
        let Ok(FilesParams { paths }) = parse::<FilesParams>(params) else {
            tracing::debug!(
                connection = self.connection,
                method,
                "invalid params ignored"
            );
            return;
        };
        let shared = Arc::clone(&self.shared);
        // The pipeline's queue may be full for a moment: wait off the runtime.
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(watch) = shared.watch.lock()
                && let Some(watch) = watch.as_ref()
            {
                for path in &paths {
                    watch.request_refinement(path, priority);
                }
            }
        })
        .await;
    }
}

/// Refinement may go on: a server is ready, or what was held may be asked again (ADR 0022). Off
/// the runtime, as the pipeline's queue may be full for a moment.
fn nudge(shared: &Arc<Shared>) {
    let shared = Arc::clone(shared);
    tokio::task::spawn_blocking(move || {
        if let Ok(watch) = shared.watch.lock()
            && let Some(watch) = watch.as_ref()
        {
            watch.nudge();
        }
    });
}

fn parse<T: DeserializeOwned>(params: Value) -> Result<T, ErrorObject> {
    // No params at all reads as `{}`, for methods whose params are all optional.
    let params = if params.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        params
    };
    serde_json::from_value(params)
        .map_err(|e| ErrorObject::new(errors::INVALID_PARAMS, e.to_string()))
}

fn json(value: &impl Serialize) -> Value {
    // The protocol types are plain data: their JSON cannot fail.
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn shutting_down() -> ErrorObject {
    ErrorObject::new(errors::SHUTTING_DOWN, "the daemon is shutting down")
}

/// Whether a daemon answers on the project's socket.
pub fn is_serving(paths: &Paths) -> bool {
    std::os::unix::net::UnixStream::connect(&paths.socket).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review n° 1 of Phase 6, m2: the daemon's own failure keeps the mailbox header.
    #[test]
    fn a_failure_of_the_daemon_is_a_tool_error_with_the_header() {
        let result = infrastructure_failure(&Error::Poisoned);
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["mailbox"]["unread"], 0);
        assert_eq!(result["structuredContent"]["error"]["code"], "internal");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("Mailbox: 0 unread.\n"), "{text}");
    }
}
