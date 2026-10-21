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

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// T-111: the work of a session, committed after it ended, holds nobody (the commit's clustering
/// regroups files, which is no change); what changes after the commit with no session open
/// still does.
#[test]
fn work_committed_after_its_session_ended_holds_no_agent() {
    let repo = project();
    let root = repo.path();
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
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
    let status = |client: &mut Client| -> StatusResult {
        client
            .request(methods::STATUS, &Value::Null)
            .expect("answered")
    };
    let until = |client: &mut Client, what: &str, done: &dyn Fn(&mut Client) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(client) {
            assert!(Instant::now() < deadline, "never: {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    tool(&mut client, "session_start", json!({"objective": "Extra"}));
    tool(&mut client, "todo_add", json!({"text": "Write extra"}));
    std::fs::write(root.join("src/extra.rs"), "pub fn extra() {}\n").unwrap();
    std::fs::write(root.join("src/a.rs"), "pub fn f() { g(); }\nfn g() {}\n").unwrap();
    until(&mut client, "the files indexed", &|c| status(c).files >= 3);
    tool(&mut client, "todo_complete", json!({"item_id": "D-1"}));
    tool(
        &mut client,
        "session_end",
        json!({"summary": "Extra written."}),
    );
    assert_eq!(reason(&mut client), None);

    let before = status(&mut client).head;
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "extra"]);
    until(&mut client, "the commit's snapshot", &|c| {
        status(c).head > before
    });
    // The commit's snapshot and the head that follows it settle; nobody is held meanwhile.
    for _ in 0..20 {
        assert_eq!(reason(&mut client), None, "committed after its session");
        std::thread::sleep(Duration::from_millis(50));
    }

    std::fs::write(root.join("src/late.rs"), "pub fn late() {}\n").unwrap();
    until(&mut client, "the late change held", &|c| {
        reason(c).is_some_and(|r| r.contains("src/late.rs") && !r.contains("extra"))
    });
}

/// PLAN.md 7.3, ADR 0039 (Q66), on a real daemon: the whiteboard read and written, checked, in
/// its one layout; a file that does not read says why, and is left as it is.
#[test]
fn the_daemon_reads_and_writes_the_whiteboard() {
    let repo = project();
    let root = repo.path();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("vscode").unwrap();
    let read = |client: &mut Client| -> Value {
        client
            .request(methods::INTENT_READ, &Value::Null)
            .expect("answered")
    };
    assert_eq!(read(&mut client), json!({"intent": null, "problem": null}));

    let intent = json!({
        "version": 1,
        "components": [
            {"id": "C-2", "name": "Store", "layer": null, "role": ""},
            {"id": "C-1", "name": "Parser", "layer": null, "role": "Turns tokens into a tree"}
        ],
        "edges": [{"id": "E-1", "src": "C-1", "dst": "C-2", "kind": "depends_on", "policy": "forbidden"}],
        "sketch": {"shapes": {"C-1": {"x": 0, "y": 0, "width": 180, "height": 80}}}
    });
    let written: Value = client
        .request(methods::INTENT_WRITE, &json!({ "intent": intent }))
        .expect("written");
    assert_eq!(written["changed"], true);
    assert_eq!(
        written["intent"]["components"][0]["id"], "C-1",
        "in id order"
    );
    let text = std::fs::read_to_string(root.join(".timonier/intent.json")).unwrap();
    assert!(text.starts_with("{\n  \"version\": 1,\n"), "{text}");
    assert_eq!(read(&mut client)["intent"], written["intent"]);
    let again: Value = client
        .request(methods::INTENT_WRITE, &json!({ "intent": intent }))
        .expect("written");
    assert_eq!(
        again["changed"], false,
        "the same whiteboard, the same file"
    );

    // A whiteboard that does not hold together is refused, and the file stays.
    let broken = json!({"version": 1, "edges": [{"id": "E-1", "src": "C-1", "dst": "C-9", "kind": "depends_on", "policy": "allowed"}]});
    match client.request::<Value>(methods::INTENT_WRITE, &json!({ "intent": broken })) {
        Err(ClientError::Rpc(error)) => {
            assert!(error.message.contains("does not join"), "{error:?}")
        }
        other => panic!("not refused: {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(root.join(".timonier/intent.json")).unwrap(),
        text
    );

    // The map's comparison (ADR 0042): the drawn names, and what the code lacks.
    let drift: Value = client
        .request(methods::INTENT_DRIFT, &Value::Null)
        .expect("answered");
    assert_eq!(drift["drawn"], true, "{drift}");
    assert_eq!(drift["names"]["C-1"], "Parser");
    assert_eq!(drift["drift"]["missing"], json!(["C-1", "C-2"]), "{drift}");

    // Edited by hand into something that does not read: the reason, not a whiteboard.
    std::fs::write(root.join(".timonier/intent.json"), "{ \"version\": 1, ").unwrap();
    let problem = read(&mut client);
    assert_eq!(problem["intent"], Value::Null);
    assert!(
        problem["problem"].as_str().unwrap().contains("intent.json"),
        "{problem}"
    );
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

/// Review of Phase 7, m8: a client of another version, which the daemon refuses, can still stop
/// it, so that the next start runs the version installed; nothing else goes before the handshake.
#[test]
fn a_client_of_another_version_can_stop_the_daemon() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let mut daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut other = daemon.client();
    let refused = other.request::<Value>(
        methods::INITIALIZE,
        &json!({"protocolVersion": PROTOCOL_VERSION + 1, "client": "timonier 9.9"}),
    );
    assert!(matches!(refused, Err(ClientError::Rpc(_))), "{refused:?}");
    assert!(matches!(
        other.request::<Value>(methods::STATUS, &Value::Null),
        Err(ClientError::Rpc(_))
    ));
    let _: Value = other.request(methods::SHUTDOWN, &Value::Null).unwrap();
    assert!(daemon.stopped(Duration::from_secs(20)));
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

/// T-114: a symbol of the map carries its kind and its signature, as its pack wrote it.
#[test]
fn a_symbol_of_the_map_says_its_kind_and_signature() {
    let repo = project();
    let root = repo.path();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("vscode").unwrap();
    indexed(&mut client, 2);
    let snapshot = subscribe(&mut client, &["pkg:cargo/demo", "file:src/a.rs"]);
    let f = snapshot
        .nodes
        .iter()
        .find(|n| n.id.as_str() == "rust:demo::a::f")
        .unwrap_or_else(|| panic!("{:?}", snapshot.nodes));
    assert_eq!(
        f.kind,
        timonier_core::NodeKind::Symbol(timonier_core::SymbolKind::Function)
    );
    assert_eq!(f.signature.as_deref(), Some("pub fn f()"));
    let file = snapshot
        .nodes
        .iter()
        .find(|n| n.id.as_str() == "file:src/a.rs")
        .expect("the file");
    assert_eq!(file.signature, None);
}

/// PLAN.md 9.1, ADR 0055, on a real daemon: the inventory of a crate that declares two HTTP
/// clients names the duplicate, its dependencies from the lockfile, and says the
/// vulnerabilities unchecked when OSV is not asked (Q79); the SBOM is CycloneDX.
#[test]
fn the_inventory_finds_two_http_clients_and_gives_a_cyclonedx_sbom() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nreqwest = \"0.12\"\nureq = \"3\"\n\n[dev-dependencies]\nattohttpc = \"0.28\"\n",
    );
    write(root, "src/lib.rs", "pub fn f() {}\n");
    // A lockfile `cargo metadata --offline` cannot complete: the fallback reads it alone.
    write(
        root,
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"reqwest\"\nversion = \"0.12.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n[[package]]\nname = \"ureq\"\nversion = \"3.4.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n[[package]]\nname = \"attohttpc\"\nversion = \"0.28.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    );
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("test-agent").unwrap();
    indexed(&mut client, 1);
    let inventory = tool(&mut client, "get_tech_inventory", json!({"cve": false}));
    let result = &inventory["result"];
    assert_eq!(result["total"], 3, "{inventory}");
    assert_eq!(result["languages"][0]["language"], "rust");
    // Two clients for the code that ships; the one of the tests is no duplicate.
    assert_eq!(
        result["duplicates"],
        json!([{"ecosystem": "cargo", "function": "http-client", "packages": ["reqwest", "ureq"], "used_by": ["demo"]}]),
        "{inventory}"
    );
    assert_eq!(result["cve"]["state"], "not_checked", "{inventory}");
    assert!(
        result["vulnerabilities"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    let ureq = result["dependencies"]
        .as_array()
        .and_then(|d| d.iter().find(|d| d["name"] == "ureq").cloned())
        .expect("ureq");
    assert_eq!(ureq["version"], "3.4.2");
    assert_eq!(ureq["direct"], true);
    // Paged: one at a time, the next page after.
    let page = tool(
        &mut client,
        "get_tech_inventory",
        json!({"cve": false, "limit": 1}),
    );
    assert_eq!(
        page["result"]["dependencies"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(page["result"]["next_cursor"], "1");

    let sbom: Value = client
        .request(methods::INVENTORY_SBOM, &json!({"cve": false}))
        .expect("the SBOM");
    assert_eq!(sbom["bomFormat"], "CycloneDX");
    assert_eq!(sbom["specVersion"], "1.5");
    assert_eq!(
        sbom["components"].as_array().map(Vec::len),
        Some(4),
        "{sbom}"
    );
}

/// The audit of 2026-10-21, finding 2: the receiver takes a run's spans only with the run's
/// token, which `run/start` gives to `timonier test`; any other local sender is dropped.
#[test]
fn the_receiver_takes_a_run_s_spans_with_its_token_only() {
    use std::io::{Read, Write};
    let repo = project();
    let root = repo.path();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(root, runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("timonier-test").unwrap();
    indexed(&mut client, 2);
    let started: Value = client
        .request(
            methods::RUN_START,
            &json!({"command": ["true"], "startedAtMs": 1}),
        )
        .expect("run/start");
    let run = started["run"].as_str().unwrap().to_owned();
    let token = started["token"].as_str().expect("a token").to_owned();
    assert_eq!(token.len(), 32);
    let endpoint = started["endpoint"].as_str().expect("an endpoint");
    let address = endpoint.trim_start_matches("http://").to_owned();
    let send = |name: &str, header: Option<&str>| {
        let body = json!({"resourceSpans": [{
            "resource": {"attributes": [{"key": "timonier.run_id", "value": {"stringValue": run}}]},
            "scopeSpans": [{"spans": [{"traceId": "0af7651916cd43dd8448eb211c80319c",
                "spanId": format!("{:0>16}", name.len()), "name": name,
                "startTimeUnixNano": "1000", "endTimeUnixNano": "2000"}]}]
        }]})
        .to_string();
        let mut stream = std::net::TcpStream::connect(&address).expect("the receiver");
        let token = header
            .map(|t| format!("x-timonier-token: {t}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "POST /v1/traces HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\n{token}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).ok();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    };
    send("none", None);
    send("wrong", Some("0123456789abcdef0123456789abcdef"));
    send("its-own", Some(&token));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let view: Value = client
            .request(methods::PERF_RUN, &json!({"run": run}))
            .expect("perf/run");
        let names: Vec<&str> = view["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["name"].as_str())
            .collect();
        if !names.is_empty() {
            assert_eq!(names, ["its-own"], "{view}");
            break;
        }
        assert!(Instant::now() < deadline, "no span kept: {view}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The audit of 2026-10-21, finding 6: a view that opens more boxes than a map shows is refused.
#[test]
fn a_view_that_opens_too_many_boxes_is_refused() {
    let repo = project();
    let runtime = tempfile::tempdir().unwrap();
    let daemon = Running::start(repo.path(), runtime.path(), Duration::from_secs(600));
    let mut client = daemon.client();
    client.initialize("vscode").unwrap();
    let expanded: Vec<String> = (0..10_001).map(|n| format!("file:src/{n}.rs")).collect();
    let refused = client
        .request::<Value>(methods::SUBSCRIBE, &json!({"view": {"expanded": expanded}}))
        .unwrap_err();
    assert!(
        format!("{refused:?}").contains("10000 boxes at most"),
        "{refused:?}"
    );
}

/// PLAN.md 9.2, ADR 0056, on a real daemon with a fake `gh` and a local remote: « Solo
/// discipliné » end to end — the task's issue and branch, its pull request pushed and opened with
/// a written description, GitHub's auto-merge asked, the issue closed with the task.
#[test]
fn solo_disciplined_opens_the_issue_the_branch_and_the_pull_request() {
    let repo = project();
    let root = repo.path();
    let remote = tempfile::tempdir().unwrap();
    let tools = tempfile::tempdir().unwrap();
    let calls = tools.path().join("calls");
    let gh = tools.path().join("gh");
    std::fs::write(
        &gh,
        format!(
            "#!/bin/sh\n[ \"$1\" = --probe ] && exit 0\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$1 $2\" in\n  \"issue create\") echo https://github.com/me/demo/issues/42 ;;\n  \"pr create\") echo https://github.com/me/demo/pull/7 ;;\nesac\n",
            calls.display()
        ),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Another test's fork can hold the script open for writing until it execs (ETXTBSY).
    for _ in 0..100 {
        match std::process::Command::new(&gh).arg("--probe").status() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => {
                assert!(other.unwrap().success());
                break;
            }
        }
    }
    git(
        remote.path(),
        &["init", "--quiet", "--bare", "--initial-branch=main"],
    );
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "first"]);
    git(
        root,
        &[
            "remote",
            "add",
            "origin",
            &remote.path().display().to_string(),
        ],
    );
    git(
        root,
        &["push", "--quiet", "--set-upstream", "origin", "main"],
    );

    let runtime = tempfile::tempdir().unwrap();
    let mut options = Options::new(root);
    options.runtime = Some(run_dir(runtime.path()));
    options.idle = Duration::from_secs(600);
    options.debounce = Duration::from_millis(100);
    options.signals = false;
    options.gh = Some(gh.clone());
    let daemon = Daemon::bind(options).unwrap().unwrap();
    let paths = daemon.paths().clone();
    let thread = std::thread::spawn(move || daemon.run());
    let mut client = Client::connect(&paths.socket).unwrap();
    client.set_timeout(Duration::from_secs(20));
    client.initialize("test-agent").unwrap();
    indexed(&mut client, 2);

    // The panel chooses Solo discipliné: the file is the daemon's to write.
    let mut workflow = serde_json::to_value(timonier_core::workflow::Workflow::preset(
        timonier_core::workflow::Preset::SoloDisciplined,
    ))
    .unwrap();
    let set: Value = client
        .request(methods::WORKFLOW_SET, &workflow)
        .expect("set");
    assert_eq!(set["preset"], "solo_disciplined");
    // Its CI: the tests, then the budgets and the rules.
    let ci = std::fs::read_to_string(root.join(".github/workflows/timonier.yml")).expect("the CI");
    assert!(ci.starts_with("# Written by Timonier"), "{ci}");
    assert!(
        ci.contains("run: timonier test --approve") && ci.contains("run: timonier check"),
        "{ci}"
    );
    assert!(ci.contains("branches: [main]"), "{ci}");
    workflow["merge"]["human_approval"] = json!(true);
    assert!(
        client
            .request::<Value>(methods::WORKFLOW_SET, &workflow)
            .is_err(),
        "auto-merge and a review exclude each other"
    );
    let read = tool(&mut client, "get_workflow", json!({}));
    assert!(
        read["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("task_start")
    );

    tool(&mut client, "session_start", json!({"objective": "Cache"}));
    let added = tool(
        &mut client,
        "plan_add_task",
        json!({"new_milestone": "Phase 1", "title": "Écrire le cache", "criterion": "a second indexing parses nothing"}),
    );
    let task = added["result"]["id"].as_str().unwrap().to_owned();
    let started = tool(&mut client, "task_start", json!({"task_id": task}));
    assert_eq!(started["result"]["ticket"], 42, "{started}");
    assert_eq!(started["result"]["branch"], "tim/42-ecrire-le-cache");
    assert_eq!(started["result"]["task"]["status"], "in_progress");
    assert_eq!(started["result"]["task"]["ticket"], "github#42");
    let branch = std::process::Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "symbolic-ref",
            "--short",
            "HEAD",
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&branch.stdout).trim(),
        "tim/42-ecrire-le-cache"
    );

    write(root, "src/cache.rs", "pub fn cache() {}\n");
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "--quiet", "-m", "feat: the cache\n\nRefs #42"],
    );
    let opened = tool(&mut client, "open_pull_request", json!({}));
    assert_eq!(
        opened["result"]["url"], "https://github.com/me/demo/pull/7",
        "{opened}"
    );
    // Pushed to the remote.
    let pushed = std::process::Command::new("git")
        .args([
            "--git-dir",
            &remote.path().display().to_string(),
            "rev-parse",
            "--verify",
            "tim/42-ecrire-le-cache",
        ])
        .output()
        .unwrap();
    assert!(pushed.status.success(), "{pushed:?}");

    let done = client
        .request::<Value>(
            methods::TOOLS_CALL,
            &json!({"name": "plan_update_task", "arguments": {"task_id": task, "status": "done"}}),
        )
        .unwrap();
    assert!(
        done["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Issue #42 closed."),
        "{done}"
    );

    let said = std::fs::read_to_string(&calls).unwrap();
    let lines: Vec<&str> = said.lines().collect();
    assert!(lines[0].starts_with("issue create --title T-1 — Écrire le cache --body Done when: a second indexing parses nothing"), "{said}");
    let pr = lines
        .iter()
        .find(|l| l.starts_with("pr create"))
        .expect("a pull request");
    assert!(
        pr.contains("--head tim/42-ecrire-le-cache --base main --title T-1 — Écrire le cache"),
        "{pr}"
    );
    // The description, over several lines of the log.
    let body = &said[said.find("pr create").unwrap()..];
    assert!(
        body.contains("Done when: a second indexing parses nothing"),
        "{body}"
    );
    assert!(body.contains("## Changes"), "{body}");
    assert!(body.contains("Closes #42"), "{body}");
    assert!(
        lines.contains(&"pr merge tim/42-ecrire-le-cache --auto --squash"),
        "{said}"
    );
    assert!(lines.contains(&"issue close 42"), "{said}");

    let _ = client.request::<Value>(methods::SHUTDOWN, &Value::Null);
    let _ = thread.join();
}

/// One `claude_code.api_request` event of Claude Code's session `session`, made at `at_ms`, in
/// OTLP/JSON, posted to the agents' port; `false` while nothing listens there.
fn post_api_request(port: u16, session: &str, at_ms: i64, input: u64) -> bool {
    use std::io::{Read, Write};
    let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
        "timeUnixNano": format!("{}", at_ms * 1_000_000),
        "body": {"stringValue": "claude_code.api_request"},
        "attributes": [
            {"key": "session.id", "value": {"stringValue": session}},
            {"key": "model", "value": {"stringValue": "claude-opus-5-5"}},
            {"key": "input_tokens", "value": {"intValue": input.to_string()}},
            {"key": "output_tokens", "value": {"intValue": "100"}},
            {"key": "cost_usd", "value": {"doubleValue": 0.01}}
        ]
    }]}]}]})
    .to_string();
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    write!(
        stream,
        "POST /v1/logs HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("the test setup");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok();
    answer.starts_with("HTTP/1.1 200")
}

/// PLAN.md 9.3, ADR 0058, on a real daemon: Claude Code's requests reach the agents' port, its
/// stop hook names its session, and the Timonier session it worked in gets its tokens; a request
/// made before the session started does not count. Claude is not held over another agent's
/// session.
#[test]
fn claude_code_s_requests_give_the_session_its_tokens() {
    let repo = project();
    let root = repo.path();
    let runtime = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut options = Options::new(root);
    options.runtime = Some(run_dir(runtime.path()));
    options.signals = false;
    options.agents = Some(timonier_daemon::server::AgentsOptions {
        data: data.path().to_path_buf(),
        port,
    });
    let daemon = Daemon::bind(options).unwrap().unwrap();
    let paths = daemon.paths().clone();
    let running = Running {
        paths,
        thread: Some(std::thread::spawn(move || daemon.run())),
    };
    let now = || {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    };
    let stop = |client: &mut Client, params: Value| -> Option<String> {
        let stop: AgentStop = client.request(methods::AGENT_STOP, &params).unwrap();
        stop.reason
    };

    // Another agent's session: Claude cannot end it, and is not held over it.
    let mut other = running.client();
    other.initialize("other-agent").unwrap();
    tool(&mut other, "session_start", json!({"objective": "Theirs"}));
    assert!(stop(&mut other, Value::Null).is_some());
    assert_eq!(stop(&mut other, json!({"agent": "claude"})), None);
    tool(&mut other, "session_end", json!({"summary": "Done."}));

    let mut client = running.client();
    client.initialize("claude-code").unwrap();
    // Seconds: the journal's times are to the second.
    let before = now() - 5_000;
    std::thread::sleep(Duration::from_millis(1100));
    let session = tool(&mut client, "session_start", json!({"objective": "Cost"}));
    let id = session["result"]["id"].as_str().unwrap().to_owned();
    std::thread::sleep(Duration::from_millis(1100));
    let deadline = Instant::now() + Duration::from_secs(20);
    while !post_api_request(port, "cc-1", now(), 1200) {
        assert!(Instant::now() < deadline, "the agents' port never listens");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(post_api_request(port, "cc-1", now(), 300));
    assert!(post_api_request(port, "cc-1", before, 99_999));
    // Another Claude Code session, which this project's hook never names.
    assert!(post_api_request(port, "cc-elsewhere", now(), 5));
    tool(&mut client, "todo_add", json!({"text": "Measure"}));
    tool(&mut client, "todo_complete", json!({"item_id": "D-1"}));
    tool(&mut client, "session_end", json!({"summary": "Measured."}));

    assert_eq!(
        stop(
            &mut client,
            json!({"agent": "claude", "agentSession": "cc-1"})
        ),
        None
    );
    let journal: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!(".timonier/sessions/{id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(journal["agent_sessions"], json!(["cc-1"]), "{journal}");
    assert_eq!(
        journal["usage"],
        json!({"requests": 2, "input": 1500, "output": 200, "cache_read": 0,
               "cache_creation": 0, "cost_usd_micros": 20_000}),
        "{journal}"
    );
    // The other agent's session, ended before, is no Claude's.
    let theirs = std::fs::read_dir(root.join(".timonier/sessions"))
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .find(|t| t.contains("Theirs"))
        .unwrap();
    assert!(!theirs.contains("cc-1"), "{theirs}");
}
