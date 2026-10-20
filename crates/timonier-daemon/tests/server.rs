//! The daemon ↔ client boundary (ADR 0019): a real daemon on a temporary socket, serving a
//! temporary project, and the blocking Rust client.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use timonier_core::NodeId;
use timonier_core::protocol::{
    ActualWritten, AgentStop, FilesParams, InitializeParams, InitializeResult, LookItem,
    PROTOCOL_VERSION, ReviewChange, ReviewMark, ReviewSummary, StatusResult, SubscribeParams,
    SubscriptionParams, TrackingSnapshot, TrackingView, ViewDelta, ViewSnapshot, ViewSpec, errors,
    methods,
};
use timonier_core::rpc::{Id, Message};
use timonier_daemon::view::ViewState;
use timonier_daemon::{Client, ClientError, Daemon, Lock, Options, Paths};

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create dirs");
    std::fs::write(path, content).expect("write file");
}

/// A crate of two files.
fn project() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp dir");
    write(repo.path(), "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub fn top() { a::f(); }\n",
    );
    write(repo.path(), "src/a.rs", "pub fn f() {}\n");
    repo
}

struct Running {
    paths: Paths,
    thread: Option<JoinHandle<timonier_daemon::Result<()>>>,
}

impl Running {
    fn start(root: &Path, runtime: &Path, idle: Duration) -> Self {
        let mut options = Options::new(root);
        options.runtime = Some(run_dir(runtime));
        options.idle = idle;
        options.debounce = Duration::from_millis(100);
        options.signals = false;
        let daemon = Daemon::bind(options)
            .expect("the daemon starts")
            .expect("no other daemon serves this project");
        let paths = daemon.paths().clone();
        Self {
            paths,
            thread: Some(std::thread::spawn(move || daemon.run())),
        }
    }

    fn client(&self) -> Client {
        let mut client = Client::connect(&self.paths.socket).expect("the daemon listens");
        client.set_timeout(Duration::from_secs(20));
        client
    }

    /// Waits for the daemon to stop on its own.
    fn stopped(&mut self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if self.thread.as_ref().is_none_or(JoinHandle::is_finished) {
                if let Some(thread) = self.thread.take() {
                    thread
                        .join()
                        .expect("the daemon thread")
                        .expect("the daemon stops cleanly");
                }
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.thread.is_some()
            && let Ok(mut client) = Client::connect(&self.paths.socket)
        {
            let _ = client.initialize("test");
            let _ = client.request::<Value>(methods::SHUTDOWN, &Value::Null);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The runtime directory, inside a temporary directory open to others: the daemon creates it,
/// closed (ADR 0019).
fn run_dir(temp: &Path) -> PathBuf {
    temp.join("run")
}

fn status(client: &mut Client) -> StatusResult {
    client
        .request(methods::STATUS, &Value::Null)
        .expect("status")
}

/// Until the first indexing has recorded `files` files.
fn indexed(client: &mut Client, files: u32) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while status(client).files != files {
        assert!(
            Instant::now() < deadline,
            "not indexed: {:?}",
            status(client)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn subscribe(client: &mut Client, expanded: &[&str]) -> ViewSnapshot {
    let view = ViewSpec {
        expanded: expanded
            .iter()
            .map(|i| NodeId::parse(*i).expect("id"))
            .collect(),
    };
    client
        .request(methods::SUBSCRIBE, &SubscribeParams { view })
        .expect("subscribe")
}

fn rpc_code(error: ClientError) -> i64 {
    match error {
        ClientError::Rpc(error) => error.code,
        other => panic!("not an error of the daemon: {other}"),
    }
}

/// What a client keeps of one view, and what it does with each delta (ADR 0019).
struct Mirror {
    subscription: u64,
    revision: u64,
    state: ViewState,
}

#[derive(Debug, PartialEq)]
enum Received {
    Applied,
    /// Older than the client's revision: a delta sent before the snapshot it holds.
    Stale,
    /// A delta went missing: `view/resync`.
    Gap,
}

impl Mirror {
    fn new(snapshot: &ViewSnapshot) -> Self {
        Self {
            subscription: snapshot.subscription,
            revision: snapshot.revision,
            state: ViewState::from_snapshot(snapshot),
        }
    }

    fn receive(&mut self, delta: &ViewDelta) -> Received {
        assert_eq!(delta.subscription, self.subscription);
        assert_eq!(delta.revision, delta.base + 1);
        if delta.base == self.revision {
            self.state.apply(delta);
            self.revision = delta.revision;
            Received::Applied
        } else if delta.base < self.revision {
            Received::Stale
        } else {
            Received::Gap
        }
    }

    fn resync(&mut self, client: &mut Client) {
        let snapshot: ViewSnapshot = client
            .request(
                methods::RESYNC,
                &SubscriptionParams {
                    subscription: self.subscription,
                },
            )
            .expect("resync");
        assert!(snapshot.revision >= self.revision);
        *self = Self::new(&snapshot);
    }
}

/// The next delta of `subscription`, within `timeout`.
fn next_delta(client: &mut Client, subscription: u64, timeout: Duration) -> Option<ViewDelta> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let incoming = client.next_incoming(left).expect("read")?;
        assert_eq!(incoming.method, methods::UPDATE);
        let delta: ViewDelta = serde_json::from_value(incoming.params).expect("a delta");
        if delta.subscription == subscription {
            return Some(delta);
        }
    }
}

/// Applies deltas until none comes for a while; resyncs on a gap. Returns the resyncs.
fn follow(client: &mut Client, mirror: &mut Mirror) -> usize {
    let mut resyncs = 0;
    while let Some(delta) = next_delta(client, mirror.subscription, Duration::from_millis(1500)) {
        if mirror.receive(&delta) == Received::Gap {
            mirror.resync(client);
            resyncs += 1;
        }
    }
    resyncs
}

/// What a new subscription to the same view shows now.
fn fresh(client: &mut Client, expanded: &[&str]) -> ViewState {
    let snapshot = subscribe(client, expanded);
    let state = ViewState::from_snapshot(&snapshot);
    let _: Value = client
        .request(
            methods::UNSUBSCRIBE,
            &SubscriptionParams {
                subscription: snapshot.subscription,
            },
        )
        .expect("unsubscribe");
    state
}

fn names(state: &ViewState) -> Vec<String> {
    state.nodes().map(|n| n.id.as_str().to_owned()).collect()
}

#[test]
fn a_client_initializes_then_reads_the_status() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let mut daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();

    let before = client.request::<StatusResult>(methods::STATUS, &Value::Null);
    assert_eq!(rpc_code(before.unwrap_err()), errors::NOT_INITIALIZED);
    let refused = client.request::<InitializeResult>(
        methods::INITIALIZE,
        &InitializeParams {
            protocol_version: PROTOCOL_VERSION + 1,
            client: "future".into(),
            lsp_relay: false,
        },
    );
    match refused.unwrap_err() {
        ClientError::Rpc(error) => {
            assert_eq!(error.code, errors::VERSION_REFUSED);
            assert_eq!(error.data.unwrap()["protocolVersion"], PROTOCOL_VERSION);
        }
        other => panic!("{other}"),
    }
    let init = client.initialize("test").unwrap();
    assert_eq!(init.protocol_version, PROTOCOL_VERSION);
    assert_eq!(init.daemon_version, timonier_core::VERSION);
    assert_eq!(
        PathBuf::from(&init.root),
        repo.path().canonicalize().unwrap()
    );

    indexed(&mut client, 2);
    let now = status(&mut client);
    assert_eq!((now.clients, now.refinements_waiting), (1, 0));
    assert!(repo.path().join(".timonier/graph.sqlite").is_file());
    assert!(repo.path().join(".timonier/.gitignore").is_file());

    // Errors keep the connection usable.
    let unknown = client.request::<Value>("view/nope", &Value::Null);
    assert_eq!(rpc_code(unknown.unwrap_err()), errors::METHOD_NOT_FOUND);
    let invalid = client.request::<Value>(methods::SUBSCRIBE, &json!({"view": 3}));
    assert_eq!(rpc_code(invalid.unwrap_err()), errors::INVALID_PARAMS);
    client
        .send_raw(&timonier_core::rpc::frame(b"{nope"))
        .unwrap();
    match client.read_raw(Duration::from_secs(5)).unwrap() {
        Some(Message::Response {
            id: None,
            outcome: Err(error),
        }) => assert_eq!(error.code, errors::PARSE),
        other => panic!("{other:?}"),
    }
    let body = br#"{"jsonrpc":"2.0","id":"x","method":"daemon/status","params":3}"#;
    client.send_raw(&timonier_core::rpc::frame(body)).unwrap();
    match client.read_raw(Duration::from_secs(5)).unwrap() {
        Some(Message::Response {
            id,
            outcome: Err(error),
        }) => {
            assert_eq!(
                (id, error.code),
                (Some(Id::String("x".into())), errors::INVALID_REQUEST)
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(status(&mut client).files, 2);

    // Opened in the editor: the file waits for refinement (ADR 0014; no server here).
    client
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/a.rs".into()],
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&mut client).refinements_waiting != 1 {
        assert!(Instant::now() < deadline, "the refinement was not queued");
        std::thread::sleep(Duration::from_millis(20));
    }

    let _: Value = client.request(methods::SHUTDOWN, &Value::Null).unwrap();
    assert!(
        daemon.stopped(Duration::from_secs(20)),
        "the daemon did not stop"
    );
    assert!(!daemon.paths.socket.exists());
    assert!(!Lock::is_held(&daemon.paths).unwrap());
}

/// Applies deltas until `done` holds, within 20 s.
fn follow_until(client: &mut Client, mirror: &mut Mirror, done: impl Fn(&ViewState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(&mirror.state) {
        let left = deadline.saturating_duration_since(Instant::now());
        let delta = next_delta(client, mirror.subscription, left)
            .unwrap_or_else(|| panic!("not reached: {:?}", mirror.state.review()));
        if mirror.receive(&delta) == Received::Gap {
            mirror.resync(client);
        }
    }
}

/// The review debt of a box in a view (ADR 0030).
fn debt_of(state: &ViewState, id: &str) -> Option<timonier_core::protocol::NodeDebt> {
    state
        .nodes()
        .find(|n| n.id.as_str() == id)
        .and_then(|n| n.debt)
}

/// A tool of the tracking, through the daemon's `tools/call` (ADR 0027): its structured content.
fn tool(client: &mut Client, name: &str, arguments: Value) -> Value {
    let result: Value = client
        .request(
            methods::TOOLS_CALL,
            &serde_json::json!({ "name": name, "arguments": arguments }),
        )
        .expect("the tool answers");
    assert_eq!(result["isError"], false, "{result}");
    result["structuredContent"].clone()
}

/// The next tracking update, within 20 s, that satisfies `done` (ADR 0031).
#[track_caller]
fn tracking_until(
    client: &mut Client,
    subscription: u64,
    done: impl Fn(&TrackingView) -> bool,
) -> TrackingSnapshot {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut last = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let Some(incoming) = client.next_incoming(left).expect("read") else {
            panic!("not reached; last update: {last:#?}");
        };
        if incoming.method != methods::TRACKING_UPDATE {
            continue;
        }
        let update: TrackingSnapshot = serde_json::from_value(incoming.params).expect("an update");
        assert_eq!(update.subscription, subscription);
        if done(&update.tracking) {
            return update;
        }
        last = Some(update);
    }
}

/// PLAN.md 6.6, ADR 0031, on a real daemon: a panel sees the plan, the todo and the journal
/// change as an agent calls the tools on another connection, the open session's diff follow
/// the code, and a change made by hand in `.timonier/`.
#[test]
fn the_tracking_follows_the_agent_live() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    );
    write(root, "a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(root, "a/src/lib.rs", "pub fn f() {}\n");
    write(root, "b/Cargo.toml", "[package]\nname = \"b\"\n");
    write(root, "b/src/lib.rs", "pub fn g() {}\n");
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut panel = daemon.client();
    panel.initialize("vscode").unwrap();
    indexed(&mut panel, 2);
    let first: TrackingSnapshot = panel
        .request(methods::TRACKING_SUBSCRIBE, &Value::Null)
        .expect("subscribed");
    assert_eq!(first.revision, 0);
    assert_eq!(first.tracking, TrackingView::default());
    let subscription = first.subscription;
    let mut agent = daemon.client();
    agent.initialize("test-agent").unwrap();

    tool(
        &mut agent,
        "session_start",
        json!({"objective": "Link a to b"}),
    );
    tool(
        &mut agent,
        "plan_add_task",
        json!({"title": "Link", "new_milestone": "First"}),
    );
    tool(
        &mut agent,
        "todo_add",
        json!({"text": "Call g", "task": "T-1"}),
    );
    let seen = tracking_until(&mut panel, subscription, |t| !t.todo.is_empty());
    assert!(seen.revision > 0);
    let tracking = &seen.tracking;
    assert_eq!(tracking.milestones[0].title, "First");
    assert_eq!(
        (tracking.milestones[0].done, tracking.milestones[0].of),
        (0, 1)
    );
    assert_eq!(tracking.todo[0].text, "Call g");
    assert_eq!(tracking.sessions[0].agent, "test-agent");
    assert!(tracking.sessions[0].open);

    // The code changes: the open session's diff follows.
    write(
        root,
        "a/Cargo.toml",
        "[package]\nname = \"a\"\n[dependencies]\nb = { path = \"../b\" }\n",
    );
    write(root, "a/src/lib.rs", "pub fn f() { b::g() }\n");
    let seen = tracking_until(&mut panel, subscription, |t| {
        t.sessions[0]
            .changes
            .as_ref()
            .is_some_and(|c| !c.linked.is_empty() && c.files == 1)
    });
    // The manifest is no file of the graph: the one file is a/src/lib.rs.
    let changes = seen.tracking.sessions[0].changes.clone().unwrap();
    assert_eq!(changes.modified[0].id, "pkg:cargo/a", "{changes:?}");
    assert_eq!(
        (
            changes.linked[0].from.name.as_str(),
            changes.linked[0].to.name.as_str()
        ),
        ("a", "b")
    );

    tool(&mut agent, "todo_complete", json!({"item_id": "D-1"}));
    tool(
        &mut agent,
        "plan_update_task",
        json!({"task_id": "T-1", "status": "in_progress"}),
    );
    tool(
        &mut agent,
        "plan_update_task",
        json!({"task_id": "T-1", "status": "done"}),
    );
    tool(&mut agent, "session_end", json!({"summary": "a calls b."}));
    let seen = tracking_until(&mut panel, subscription, |t| t.open.is_none());
    let tracking = &seen.tracking;
    assert_eq!(
        (tracking.milestones[0].done, tracking.milestones[0].of),
        (1, 1)
    );
    assert!(tracking.todo.is_empty());
    let ended = &tracking.sessions[0];
    assert_eq!(ended.summary.as_deref(), Some("a calls b."));
    assert!(ended.todo[0].done);
    // Written in the journal at the end, the same as it showed live.
    assert_eq!(ended.changes.as_ref(), Some(&changes));
    let journal: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!(".timonier/sessions/{}.json", ended.id)))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["changes"]["linked"][0]["to"]["id"], "pkg:cargo/b");
    // The bridge (ADR 0032): no review yet, so every session counts, and a's file is to read.
    assert_eq!(tracking.sessions_since_review, 1);
    assert!(tracking.review.since.is_none());
    assert!(
        tracking.debt.is_some_and(|d| d.files >= 1),
        "{:?}",
        tracking.debt
    );
    assert!(
        tracking.look.iter().any(|item| matches!(
            item,
            LookItem::Debt { item, path, .. } if item.id.as_str() == "pkg:cargo/a" && path.is_empty()
        )),
        "{:?}",
        tracking.look
    );
    // Reviewed: nothing to look at, no session since.
    let _: ReviewMark = panel
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked");
    let seen = tracking_until(&mut panel, subscription, |t| t.review.since.is_some());
    assert_eq!(seen.tracking.sessions_since_review, 0);
    let seen = if seen.tracking.look.is_empty() {
        seen
    } else {
        tracking_until(&mut panel, subscription, |t| t.look.is_empty())
    };
    assert!(seen.tracking.debt.is_none());

    // By hand: the panel sees the file break, then mend.
    let plan = root.join(".timonier/plan.json");
    let kept = std::fs::read_to_string(&plan).unwrap();
    std::fs::write(&plan, "{ not json").unwrap();
    let seen = tracking_until(&mut panel, subscription, |t| !t.problems.is_empty());
    assert_eq!(seen.tracking.problems[0].file, "plan.json");
    assert!(seen.tracking.milestones.is_empty());
    std::fs::write(&plan, kept.replace("\"First\"", "\"Renamed\"")).unwrap();
    tracking_until(&mut panel, subscription, |t| {
        t.problems.is_empty() && t.milestones.first().is_some_and(|m| m.title == "Renamed")
    });
    // Q52: set up once `timonier init` left its record.
    assert!(!seen.tracking.configured);
    std::fs::write(root.join(".timonier/init.json"), "{}\n").unwrap();
    tracking_until(&mut panel, subscription, |t| t.configured);
}

/// PLAN.md 6.8, ADR 0033 (Q49, Q50), on a real daemon: when to hold an agent that stops, and
/// `actual.json` written from the head, the same until the graph changes.
#[test]
fn the_daemon_says_when_to_hold_an_agent_and_writes_the_actual_architecture() {
    let repo = project();
    let root = repo.path();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test-agent").unwrap();
    indexed(&mut client, 2);
    let reason = |client: &mut Client| -> Option<String> {
        let stop: AgentStop = client
            .request(methods::AGENT_STOP, &Value::Null)
            .expect("answered");
        stop.reason
    };
    assert_eq!(reason(&mut client), None, "nothing changed yet");
    // As after `timonier init`: a first session, ended. What changes next is measured from it.
    tool(
        &mut client,
        "session_start",
        json!({"objective": "Initialise"}),
    );
    tool(&mut client, "todo_add", json!({"text": "Initialise"}));
    tool(&mut client, "todo_complete", json!({"item_id": "D-1"}));
    tool(
        &mut client,
        "session_end",
        json!({"summary": "Initialised."}),
    );
    assert_eq!(reason(&mut client), None);
    std::fs::write(root.join("src/extra.rs"), "pub fn extra() {}\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let held = loop {
        if let Some(held) = reason(&mut client) {
            break held;
        }
        assert!(Instant::now() < deadline, "the change is never seen");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        held.contains("no Timonier session open (src/extra.rs)"),
        "{held}"
    );
    tool(&mut client, "session_start", json!({"objective": "Extra"}));
    assert!(reason(&mut client).is_some_and(|r| r.contains("todo is empty")));
    tool(&mut client, "todo_add", json!({"text": "Write extra"}));
    assert!(reason(&mut client).is_some_and(|r| r.contains("1 todo item(s) not done")));
    tool(&mut client, "todo_complete", json!({"item_id": "D-1"}));
    assert_eq!(reason(&mut client), None);
    tool(
        &mut client,
        "session_end",
        json!({"summary": "Extra written."}),
    );
    assert_eq!(reason(&mut client), None, "all in a session");

    let first: ActualWritten = client
        .request(methods::ACTUAL_WRITE, &Value::Null)
        .expect("written");
    assert!(first.changed);
    let actual: Value =
        serde_json::from_str(&std::fs::read_to_string(&first.path).unwrap()).unwrap();
    assert_eq!(actual["version"], 1);
    assert!(
        actual["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["kind"] == "package"),
        "{actual}"
    );
    let again: ActualWritten = client
        .request(methods::ACTUAL_WRITE, &Value::Null)
        .expect("written");
    assert!(!again.changed, "the same graph, the same file");
}

/// PLAN.md 6.5, ADR 0030, on a real daemon: what a session writes is debt until a review; what
/// changes with no session open is debt too, and untracked.
#[test]
fn a_view_shows_the_review_debt() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    );
    write(root, "a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(root, "a/src/lib.rs", "pub mod two;\npub fn f() {}\n");
    write(root, "a/src/two.rs", "pub fn t() {}\n");
    write(root, "b/Cargo.toml", "[package]\nname = \"b\"\n");
    write(root, "b/src/lib.rs", "pub fn g() {}\n");
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test-agent").unwrap();
    indexed(&mut client, 3);
    let _: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked");
    let snapshot = subscribe(&mut client, &[]);
    let mut mirror = Mirror::new(&snapshot);
    follow_until(&mut client, &mut mirror, |state| {
        debt_of(state, "pkg:cargo/a").is_none()
    });

    // A session writes a/src/two.rs: one file of a's two to read again, none untracked.
    tool(
        &mut client,
        "session_start",
        serde_json::json!({"objective": "Change a"}),
    );
    write(root, "a/src/two.rs", "pub fn t() {}\npub fn u() {}\n");
    follow_until(&mut client, &mut mirror, |state| {
        debt_of(state, "pkg:cargo/a").is_some()
    });
    assert_eq!(
        debt_of(&mirror.state, "pkg:cargo/a"),
        Some(timonier_core::protocol::NodeDebt {
            files: 1,
            of: 2,
            untracked: 0
        })
    );
    assert_eq!(debt_of(&mirror.state, "pkg:cargo/b"), None);
    tool(
        &mut client,
        "session_end",
        serde_json::json!({"summary": "a changed."}),
    );

    // b changes with no session open: debt, and untracked.
    write(root, "b/src/lib.rs", "pub fn g() {}\npub fn h() {}\n");
    follow_until(&mut client, &mut mirror, |state| {
        debt_of(state, "pkg:cargo/b").is_some_and(|d| d.untracked == 1)
    });
    assert_eq!(
        debt_of(&mirror.state, "pkg:cargo/a").map(|d| d.files),
        Some(1)
    );

    // Reviewed: no debt left.
    let _: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked");
    follow_until(&mut client, &mut mirror, |state| {
        debt_of(state, "pkg:cargo/a").is_none() && debt_of(state, "pkg:cargo/b").is_none()
    });
}

/// PLAN.md 5.2, ADR 0024, on a real daemon: nothing before the first review (Q27); after it, a
/// dependency between two crates and a module show in the view's review diff, and the summary.
#[test]
fn a_view_shows_what_changed_since_the_review() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    );
    write(root, "a/Cargo.toml", "[package]\nname = \"a\"\n");
    write(root, "a/src/lib.rs", "pub fn f() {}\n");
    write(root, "b/Cargo.toml", "[package]\nname = \"b\"\n");
    write(root, "b/src/lib.rs", "pub fn g() {}\n");
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    indexed(&mut client, 2);
    let snapshot = subscribe(&mut client, &[]);
    assert_eq!(snapshot.review.since, None, "no review yet");
    assert!(snapshot.nodes.iter().all(|n| n.review.is_none()));
    let mut mirror = Mirror::new(&snapshot);

    let mark: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked");
    follow_until(&mut client, &mut mirror, |state| {
        state.review().since == Some(mark)
    });
    assert_eq!(
        mirror.state.review(),
        &ReviewSummary {
            since: Some(mark),
            ..ReviewSummary::default()
        },
        "nothing changed since"
    );

    // a now depends on b, and has a new module.
    write(
        root,
        "a/Cargo.toml",
        "[package]\nname = \"a\"\n\n[dependencies]\nb = { path = \"../b\" }\n",
    );
    write(root, "a/src/extra.rs", "pub fn e() {\n    b::g();\n}\n");
    write(root, "a/src/lib.rs", "pub mod extra;\npub fn f() {}\n");
    follow_until(&mut client, &mut mirror, |state| {
        let review = state.review();
        !review.dependencies_added.is_empty() && !review.modified.is_empty()
    });
    let review = mirror.state.review();
    let dependencies: Vec<(&str, &str)> = review
        .dependencies_added
        .iter()
        .map(|d| (d.src.id.as_str(), d.dst.id.as_str()))
        .collect();
    assert_eq!(dependencies, [("pkg:cargo/a", "pkg:cargo/b")]);
    let modified: Vec<&str> = review.modified.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(modified, ["pkg:cargo/a"]);
    assert!(
        review.added.is_empty() && review.removed.is_empty(),
        "{review:?}"
    );
    let a = mirror
        .state
        .nodes()
        .find(|n| n.id.as_str() == "pkg:cargo/a")
        .expect("a on the map");
    assert_eq!(
        a.review.and_then(|r| r.change),
        Some(ReviewChange::Modified)
    );
    let edge = mirror
        .state
        .edges()
        .find(|e| e.src.as_str() == "pkg:cargo/a" && e.dst.as_str() == "pkg:cargo/b")
        .expect("a → b on the map");
    assert!(edge.review.is_some_and(|r| r.added));
    // The new module, inside a.
    let open = fresh(&mut client, &["pkg:cargo/a"]);
    let extra = open
        .nodes()
        .find(|n| n.id.as_str() == "file:a/src/extra.rs")
        .expect("the new file");
    assert_eq!(
        extra.review.and_then(|r| r.change),
        Some(ReviewChange::Added)
    );
    // Marked again: the diff starts from there.
    let again: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked again");
    follow_until(&mut client, &mut mirror, |state| {
        state.review().since == Some(again)
    });
    assert!(mirror.state.review().dependencies_added.is_empty());
    assert!(mirror.state.nodes().all(|n| n.review.is_none()));
}

/// ADR 0024: `review/mark` freezes the head as reviewed, and the status names the last review.
#[test]
fn a_client_marks_the_head_as_reviewed() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    indexed(&mut client, 2);
    let before = status(&mut client);
    assert_eq!(before.last_review, None, "no review yet");
    let mark: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked");
    assert_eq!(mark.seq, before.head, "the head is what is reviewed");
    assert!(mark.timestamp_ms > 0);
    let after = status(&mut client);
    assert_eq!(after.last_review, Some(mark));
    assert_eq!(after.head, mark.seq + 1, "a new head opens");
    let again: ReviewMark = client
        .request(methods::REVIEW_MARK, &Value::Null)
        .expect("marked again");
    assert_eq!(status(&mut client).last_review, Some(again));
}

#[test]
fn a_view_follows_the_graph_through_deltas() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    indexed(&mut client, 2);

    let expanded = ["pkg:cargo/demo"];
    let snapshot = subscribe(&mut client, &expanded);
    assert_eq!(snapshot.revision, 0);
    let mut mirror = Mirror::new(&snapshot);
    // The package, open on its files (a level is split beyond 12 elements, ADR 0016).
    assert_eq!(
        names(&mirror.state),
        ["file:src/a.rs", "file:src/lib.rs", "pkg:cargo/demo"]
    );
    let lib = mirror
        .state
        .nodes()
        .find(|n| n.id.as_str() == "file:src/lib.rs")
        .unwrap();
    assert_eq!(
        lib.parent.as_ref().map(NodeId::as_str),
        Some("pkg:cargo/demo")
    );
    assert_eq!(lib.file.as_deref(), Some("src/lib.rs"));
    let edges: Vec<(&str, &str)> = mirror
        .state
        .edges()
        .map(|e| (e.src.as_str(), e.dst.as_str()))
        .collect();
    assert!(
        edges.contains(&("file:src/lib.rs", "file:src/a.rs")),
        "{edges:?}"
    );

    write(repo.path(), "src/b.rs", "pub fn g() { crate::a::f(); }\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub fn top() { a::f(); }\n",
    );
    let first =
        next_delta(&mut client, mirror.subscription, Duration::from_secs(20)).expect("a delta");
    assert_eq!(first.base, 0);
    assert_eq!(mirror.receive(&first), Received::Applied);
    assert_eq!(follow(&mut client, &mut mirror), 0, "no delta was lost");
    assert!(mirror.revision >= 1);
    assert_eq!(mirror.state, fresh(&mut client, &expanded));
    assert_eq!(
        names(&mirror.state),
        [
            "file:src/a.rs",
            "file:src/b.rs",
            "file:src/lib.rs",
            "pkg:cargo/demo"
        ]
    );
    let edges: Vec<(&str, &str)> = mirror
        .state
        .edges()
        .map(|e| (e.src.as_str(), e.dst.as_str()))
        .collect();
    assert!(
        edges.contains(&("file:src/b.rs", "file:src/a.rs")),
        "{edges:?}"
    );
}

#[test]
fn a_client_that_missed_a_delta_gets_a_full_snapshot() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    indexed(&mut client, 2);
    let expanded = ["pkg:cargo/demo"];
    let mut mirror = Mirror::new(&subscribe(&mut client, &expanded));

    // The first delta is lost on the way (a webview reloaded, a full queue).
    write(repo.path(), "src/b.rs", "pub fn g() {}\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub fn top() { a::f(); }\n",
    );
    let lost =
        next_delta(&mut client, mirror.subscription, Duration::from_secs(20)).expect("a delta");
    assert_eq!(lost.base, 0);
    std::thread::sleep(Duration::from_millis(500));

    // The next one does not start where the client is: it asks for the whole view.
    write(repo.path(), "src/c.rs", "pub fn h() {}\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub mod c;\npub fn top() { a::f(); }\n",
    );
    let next =
        next_delta(&mut client, mirror.subscription, Duration::from_secs(20)).expect("a delta");
    assert_eq!(mirror.receive(&next), Received::Gap);
    mirror.resync(&mut client);
    assert!(mirror.revision >= next.revision);
    follow(&mut client, &mut mirror);
    assert_eq!(mirror.state, fresh(&mut client, &expanded));

    // Deltas apply again after the snapshot.
    write(repo.path(), "src/c.rs", "pub fn h() {}\npub fn i() {}\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub fn top() { a::f(); }\n",
    );
    let delta =
        next_delta(&mut client, mirror.subscription, Duration::from_secs(20)).expect("a delta");
    assert_eq!(mirror.receive(&delta), Received::Applied);
    assert_eq!(follow(&mut client, &mut mirror), 0);
    assert_eq!(mirror.state, fresh(&mut client, &expanded));
}

#[test]
fn subscriptions_belong_to_their_connection() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    indexed(&mut client, 2);
    let kept = subscribe(&mut client, &["pkg:cargo/demo"]);
    let dropped = subscribe(&mut client, &["pkg:cargo/demo"]);
    assert_ne!(kept.subscription, dropped.subscription);
    let _: Value = client
        .request(
            methods::UNSUBSCRIBE,
            &SubscriptionParams {
                subscription: dropped.subscription,
            },
        )
        .unwrap();
    let again = client.request::<Value>(
        methods::UNSUBSCRIBE,
        &SubscriptionParams {
            subscription: dropped.subscription,
        },
    );
    assert_eq!(rpc_code(again.unwrap_err()), errors::UNKNOWN_SUBSCRIPTION);

    // Another client cannot touch this one's views.
    let mut other = daemon.client();
    other.initialize("other").unwrap();
    let stolen = other.request::<ViewSnapshot>(
        methods::RESYNC,
        &SubscriptionParams {
            subscription: kept.subscription,
        },
    );
    assert_eq!(rpc_code(stolen.unwrap_err()), errors::UNKNOWN_SUBSCRIPTION);

    write(repo.path(), "src/a.rs", "pub fn f() {}\npub fn more() {}\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub fn top() { a::f(); }\npub fn more() {}\n",
    );
    let mut seen = Vec::new();
    while let Some(incoming) = client.next_incoming(Duration::from_millis(1500)).unwrap() {
        let delta: ViewDelta = serde_json::from_value(incoming.params).unwrap();
        seen.push(delta.subscription);
    }
    assert!(!seen.is_empty());
    assert!(seen.iter().all(|s| *s == kept.subscription), "{seen:?}");
    assert!(
        other
            .next_incoming(Duration::from_millis(200))
            .unwrap()
            .is_none()
    );
}

#[test]
fn one_daemon_serves_a_project() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    // A socket left by a daemon that died is replaced.
    let paths = Paths::in_runtime(repo.path(), &run_dir(runtime.path())).unwrap();
    assert!(Lock::acquire(&paths).unwrap().is_some());
    std::fs::write(&paths.socket, "stale").unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test").unwrap();

    let mut options = Options::new(repo.path());
    options.runtime = Some(run_dir(runtime.path()));
    assert!(
        Daemon::bind(options).unwrap().is_none(),
        "a second daemon started"
    );
    assert!(timonier_daemon::server::is_serving(&daemon.paths));
    client.initialize("still").unwrap();
}

#[test]
fn the_daemon_stops_after_a_while_without_clients() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let mut daemon = Running::start(repo.path(), runtime.path(), Duration::from_millis(400));
    let mut client = daemon.client();
    client.initialize("test").unwrap();
    std::thread::sleep(Duration::from_millis(1000));
    assert!(!daemon.stopped(Duration::ZERO), "stopped with a client");
    drop(client);
    assert!(
        daemon.stopped(Duration::from_secs(10)),
        "still running without clients"
    );
    assert!(!daemon.paths.socket.exists());
}

#[test]
fn shutdown_closes_the_other_clients() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let mut daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut watcher = daemon.client();
    watcher.initialize("watcher").unwrap();
    subscribe(&mut watcher, &[]);
    let mut stopper = daemon.client();
    stopper.initialize("stopper").unwrap();
    let _: Value = stopper.request(methods::SHUTDOWN, &Value::Null).unwrap();
    assert!(daemon.stopped(Duration::from_secs(20)));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match watcher.next_incoming(Duration::from_millis(100)) {
            Err(ClientError::Closed | ClientError::Io(_)) => break,
            _ => assert!(Instant::now() < deadline, "the client was not closed"),
        }
    }
}

/// PLAN.md 4.5: the daemon asks the client that relays its IDE's language servers. Without
/// one, refinement waits and keeps its queue.
/// `view/displayed` (ADR 0014): the files the map draws wait for refinement, first.
#[test]
fn files_shown_on_the_map_wait_for_refinement() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut map = daemon.client();
    map.initialize("map").unwrap();
    indexed(&mut map, 2);
    assert_eq!(status(&mut map).refinements_waiting, 0);
    map.notify(
        methods::DISPLAYED,
        &FilesParams {
            paths: vec!["src/lib.rs".into(), "src/a.rs".into()],
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&mut map).refinements_waiting != 2 {
        assert!(
            Instant::now() < deadline,
            "the displayed files were not queued"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_relaying_client_answers_the_daemon_s_call_hierarchies() {
    use timonier_core::protocol::{OutgoingCall, OutgoingCallsParams};
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut editor = daemon.client();
    editor.initialize("editor").unwrap();
    indexed(&mut editor, 2);
    editor
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/lib.rs".into()],
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&mut editor).refinements_waiting != 1 {
        assert!(Instant::now() < deadline, "the refinement was not queued");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        status(&mut editor).refinements_waiting,
        1,
        "lost without a relay"
    );

    let mut relay = daemon.client();
    let _: InitializeResult = relay
        .request(
            methods::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION,
                client: "relay".into(),
                lsp_relay: true,
            },
        )
        .unwrap();
    // `top` calls `a::f`, whose name starts at column 7 of src/a.rs.
    let asked = relay
        .next_incoming(Duration::from_secs(20))
        .unwrap()
        .expect("a question");
    assert_eq!(asked.method, methods::OUTGOING_CALLS);
    let params: OutgoingCallsParams = serde_json::from_value(asked.params).unwrap();
    assert_eq!(params.path, "src/lib.rs");
    assert_eq!(params.position.line, 1);
    let answer = vec![OutgoingCall {
        path: Some("src/a.rs".into()),
        at: timonier_core::Position::new(0, 7),
        sites: 1,
    }];
    relay
        .respond(asked.id.expect("a request"), &answer)
        .unwrap();

    let expanded = ["pkg:cargo/demo", "file:src/lib.rs", "file:src/a.rs"];
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let view = fresh(&mut editor, &expanded);
        let verified = view.edges().any(|e| {
            e.src.as_str() == "rust:demo::top"
                && e.dst.as_str() == "rust:demo::a::f"
                && e.best == timonier_core::Provenance::Lsp
                && !e.approximate_only
        });
        if verified {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no verified call: {:?}",
            view.edges().collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(status(&mut editor).refinements_waiting, 0);
}

/// ADR 0022, over the socket: a relaying client answers `lsp/definitions` for the usages of a
/// file, then `lsp/references` for a symbol whose signature changed.
#[test]
fn a_relaying_client_answers_definitions_and_references() {
    use timonier_core::protocol::{DefinitionTarget, DefinitionsParams};
    let repo = project();
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod shape;\npub fn top() { a::f(); }\n",
    );
    write(
        repo.path(),
        "src/a.rs",
        "use crate::shape::Unit;\npub fn f() {}\npub fn g(u: &Unit) {}\n",
    );
    write(repo.path(), "src/shape.rs", "pub struct Unit;\n");
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut editor = daemon.client();
    editor.initialize("editor").unwrap();
    indexed(&mut editor, 3);
    let mut relay = daemon.client();
    let _: InitializeResult = relay
        .request(
            methods::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION,
                client: "relay".into(),
                lsp_relay: true,
            },
        )
        .unwrap();
    editor
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/a.rs".into()],
            },
        )
        .unwrap();
    // Answers each question until `done`; the methods asked, in order.
    let mut asked: Vec<String> = Vec::new();
    let mut serve = |relay: &mut Client,
                     editor: &mut Client,
                     done: &dyn Fn(&mut Client) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(editor) {
            assert!(Instant::now() < deadline, "asked so far: {asked:?}");
            let Some(question) = relay.next_incoming(Duration::from_millis(100)).unwrap() else {
                continue;
            };
            let id = question.id.expect("a request");
            match question.method.as_str() {
                methods::OUTGOING_CALLS => relay.respond(id, &Vec::<Value>::new()).unwrap(),
                methods::DEFINITIONS => {
                    let params: DefinitionsParams =
                        serde_json::from_value(question.params).unwrap();
                    let answer: Vec<Vec<DefinitionTarget>> = params
                        .questions
                        .iter()
                        .map(|_| {
                            vec![DefinitionTarget {
                                path: Some("src/shape.rs".into()),
                                at: timonier_core::Position::new(0, 11),
                            }]
                        })
                        .collect();
                    relay.respond(id, &answer).unwrap();
                }
                methods::REFERENCES => relay
                    .respond(id, &vec!["src/a.rs".to_owned(), "src/shape.rs".to_owned()])
                    .unwrap(),
                other => panic!("{other}"),
            }
            asked.push(question.method);
        }
    };
    // The use of `Unit` by a.rs, verified by its definition.
    serve(&mut relay, &mut editor, &|editor| {
        fresh(editor, &["pkg:cargo/demo"]).edges().any(|e| {
            e.kind == timonier_core::EdgeKind::References
                && e.src.as_str() == "file:src/a.rs"
                && e.dst.as_str() == "file:src/shape.rs"
                && e.best == timonier_core::Provenance::Lsp
        })
    });
    // `Unit` changes: the relay names its users, and a.rs, verified, is asked about again.
    write(repo.path(), "src/shape.rs", "pub struct Unit(pub u32);\n");
    let before = status(&mut editor).lsp_answers;
    serve(&mut relay, &mut editor, &|editor| {
        let now = status(editor).lsp_answers;
        now.references > before.references && now.definitions > before.definitions
    });
    assert!(asked.iter().any(|m| m == methods::DEFINITIONS), "{asked:?}");
    assert!(asked.iter().any(|m| m == methods::REFERENCES), "{asked:?}");
    // The relay named the users: no lookup fell back on the name (review n° 5, m-δ).
    let lookups = status(&mut editor).lookups;
    assert!(lookups.named >= 1, "{lookups:?}");
    assert_eq!(lookups.by_name, 0);
}

/// Review n° 5, I-α, over the socket: a relaying client whose documents are not saved refuses
/// with `ContentModified`. The daemon holds the file, asks nothing more meanwhile, and asks
/// again once the client sends `editor/saved`.
#[test]
fn a_relaying_client_with_unsaved_documents_is_asked_again_once_saved() {
    use timonier_core::protocol::{OutgoingCall, OutgoingCallsParams};
    use timonier_core::rpc::ErrorObject;
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut relay = daemon.client();
    let _: InitializeResult = relay
        .request(
            methods::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION,
                client: "relay".into(),
                lsp_relay: true,
            },
        )
        .unwrap();
    indexed(&mut relay, 2);
    relay
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/lib.rs".into()],
            },
        )
        .unwrap();
    let question = |relay: &mut Client| {
        relay
            .next_incoming(Duration::from_secs(20))
            .unwrap()
            .expect("a question")
    };
    let asked = question(&mut relay);
    assert_eq!(asked.method, methods::OUTGOING_CALLS);
    relay
        .refuse(
            asked.id.expect("a request"),
            ErrorObject::new(errors::CONTENT_MODIFIED, "unsaved changes in src/lib.rs"),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&mut relay).refinements_held != 1 {
        assert!(Instant::now() < deadline, "the file was not held");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(status(&mut relay).refinements_waiting, 0);
    assert!(
        relay
            .next_incoming(Duration::from_millis(300))
            .unwrap()
            .is_none(),
        "nothing is asked while the documents are not saved"
    );
    relay.notify(methods::SAVED, &json!({})).unwrap();
    let asked = question(&mut relay);
    assert_eq!(asked.method, methods::OUTGOING_CALLS);
    let params: OutgoingCallsParams = serde_json::from_value(asked.params).unwrap();
    assert_eq!(params.path, "src/lib.rs");
    let answer = vec![OutgoingCall {
        path: Some("src/a.rs".into()),
        at: timonier_core::Position::new(0, 7),
        sites: 1,
    }];
    relay
        .respond(asked.id.expect("a request"), &answer)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !fresh(
        &mut relay,
        &["pkg:cargo/demo", "file:src/lib.rs", "file:src/a.rs"],
    )
    .edges()
    .any(|e| e.dst.as_str() == "rust:demo::a::f" && e.best == timonier_core::Provenance::Lsp)
    {
        assert!(Instant::now() < deadline, "no verified call");
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = status(&mut relay);
    assert_eq!(
        (status.refinements_waiting, status.refinements_held),
        (0, 0)
    );
}

/// Review n° 6, m6-1 and m6-3, over the socket: the relay changes hands. What the leaving client
/// held back goes to the one that relays in its place, and a question lost when a third one
/// takes over goes to that one: no file leaves the queue unverified.
#[test]
fn held_and_lost_questions_go_to_the_client_that_relays_next() {
    use timonier_core::protocol::{OutgoingCall, OutgoingCallsParams};
    use timonier_core::rpc::ErrorObject;
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let relay = |name: &str| {
        let mut client = daemon.client();
        let _: InitializeResult = client
            .request(
                methods::INITIALIZE,
                &InitializeParams {
                    protocol_version: PROTOCOL_VERSION,
                    client: name.into(),
                    lsp_relay: true,
                },
            )
            .unwrap();
        client
    };
    let question = |client: &mut Client| {
        let asked = client
            .next_incoming(Duration::from_secs(20))
            .unwrap()
            .expect("a question");
        assert_eq!(asked.method, methods::OUTGOING_CALLS);
        asked
    };
    let mut first = relay("first");
    indexed(&mut first, 2);
    let mut second = relay("second");
    first
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/lib.rs".into()],
            },
        )
        .unwrap();
    // The last client relays; its documents are not saved.
    let asked = question(&mut second);
    second
        .refuse(
            asked.id.expect("a request"),
            ErrorObject::new(errors::CONTENT_MODIFIED, "unsaved changes"),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&mut first).refinements_held != 1 {
        assert!(Instant::now() < deadline, "the file was not held");
        std::thread::sleep(Duration::from_millis(20));
    }
    // It leaves: the first one relays again, and is asked.
    drop(second);
    let _unanswered = question(&mut first);
    // A third one takes over before the first answers: the question goes to it.
    let mut third = relay("third");
    let asked = question(&mut third);
    let params: OutgoingCallsParams = serde_json::from_value(asked.params).unwrap();
    assert_eq!(params.path, "src/lib.rs");
    let answer = vec![OutgoingCall {
        path: Some("src/a.rs".into()),
        at: timonier_core::Position::new(0, 7),
        sites: 1,
    }];
    third
        .respond(asked.id.expect("a request"), &answer)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !fresh(
        &mut first,
        &["pkg:cargo/demo", "file:src/lib.rs", "file:src/a.rs"],
    )
    .edges()
    .any(|e| e.dst.as_str() == "rust:demo::a::f" && e.best == timonier_core::Provenance::Lsp)
    {
        assert!(Instant::now() < deadline, "no verified call");
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = status(&mut first);
    assert_eq!(
        (status.refinements_waiting, status.refinements_held),
        (0, 0)
    );
}

/// Review n° 1 of Phase 4, I2: deltas that do not fit in a client's queue are not lost for
/// good. Once the queue drains, each view learns of the gap and resyncs, though the graph no
/// longer changes.
#[test]
fn a_client_whose_queue_overflowed_learns_of_the_gap() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("burst").unwrap();
    indexed(&mut client, 2);
    // More views than the client's queue holds (1 024 frames).
    let mut mirrors: Vec<Mirror> = (0..1100)
        .map(|_| Mirror::new(&subscribe(&mut client, &["pkg:cargo/demo"])))
        .collect();
    // The client does not read while the graph changes three times: the deltas fill the socket,
    // then the queue, then overflow.
    for (i, file) in ["b", "c", "d"].iter().enumerate() {
        write(repo.path(), &format!("src/{file}.rs"), "pub fn g() {}\n");
        let mods: String = ["a", "b", "c", "d"][..i + 2]
            .iter()
            .map(|m| format!("pub mod {m};\n"))
            .collect();
        write(
            repo.path(),
            "src/lib.rs",
            &format!("{mods}pub fn top() {{ a::f(); }}\n"),
        );
        std::thread::sleep(Duration::from_millis(800));
    }
    let mut resyncs = 0;
    let index: std::collections::HashMap<u64, usize> = mirrors
        .iter()
        .enumerate()
        .map(|(i, m)| (m.subscription, i))
        .collect();
    while let Some(incoming) = client.next_incoming(Duration::from_millis(1500)).unwrap() {
        let delta: ViewDelta = serde_json::from_value(incoming.params).unwrap();
        let mirror = &mut mirrors[index[&delta.subscription]];
        if mirror.receive(&delta) == Received::Gap {
            mirror.resync(&mut client);
            resyncs += 1;
        }
    }
    let now = fresh(&mut client, &["pkg:cargo/demo"]);
    let behind = mirrors.iter().filter(|m| m.state != now).count();
    assert_eq!(
        behind, 0,
        "{behind} views out of date, and never told: {resyncs} resyncs"
    );
    // The queue did overflow: some views had to resync.
    assert!(
        resyncs > 0,
        "no delta was lost: the test did not overflow the queue"
    );
}

/// Review n° 1 of Phase 4, I1: a relaying client that stops reading holds nothing. A save is
/// indexed, refinement gives up on it, and the daemon stops when asked.
#[test]
fn a_relaying_client_that_stops_reading_holds_nothing() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let mut daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut relay = daemon.client();
    let _: InitializeResult = relay
        .request(
            methods::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION,
                client: "mute relay".into(),
                lsp_relay: true,
            },
        )
        .unwrap();
    let mut observer = daemon.client();
    observer.initialize("observer").unwrap();
    indexed(&mut observer, 2);
    // The relay fills its queue with views, then reads no more.
    for _ in 0..1100 {
        let _: ViewSnapshot = relay
            .request(
                methods::SUBSCRIBE,
                &SubscribeParams {
                    view: ViewSpec::default(),
                },
            )
            .unwrap();
    }
    let watched = subscribe(&mut observer, &["pkg:cargo/demo"]);
    write(repo.path(), "src/a.rs", "pub fn f() {}\npub fn more() {}\n");
    std::thread::sleep(Duration::from_millis(600));
    // Refinement would ask the relay: it must not wait for it.
    observer
        .notify(
            methods::OPENED,
            &FilesParams {
                paths: vec!["src/lib.rs".into(), "src/a.rs".into()],
            },
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    write(repo.path(), "src/b.rs", "pub fn g() {}\n");
    write(
        repo.path(),
        "src/lib.rs",
        "pub mod a;\npub mod b;\npub fn top() { a::f(); }\n",
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let view = fresh(&mut observer, &["pkg:cargo/demo"]);
        if view.nodes().any(|n| n.id.as_str() == "file:src/b.rs") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the save was not indexed: the pipeline waits for the relay"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = watched;
    let start = Instant::now();
    let _: Value = observer.request(methods::SHUTDOWN, &Value::Null).unwrap();
    assert!(
        daemon.stopped(Duration::from_secs(10)),
        "the daemon did not stop"
    );
    assert!(start.elapsed() < Duration::from_secs(10));
}
