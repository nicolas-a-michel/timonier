//! `timonier mcp` (PLAN.md 6.2, ADR 0027, 0028): a real MCP client → the proxy → a real daemon,
//! on a temporary project, with the runtime directory in a temporary directory too.

use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation, JsonObject,
};
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Value, json};

struct Agent;

impl ClientHandler for Agent {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("test-agent", "1.0.0"),
        )
    }
}

struct Project {
    repo: tempfile::TempDir,
    runtime: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let repo = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(repo.path().join("src")).expect("src");
        std::fs::write(
            repo.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        std::fs::write(repo.path().join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
        Self {
            repo,
            runtime: tempfile::tempdir().expect("temp dir"),
        }
    }

    async fn agent(&self) -> RunningService<RoleClient, Agent> {
        let mut command = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("tim"));
        command
            .args(["mcp", "--root"])
            .arg(self.repo.path())
            .env("TIMONIER_SCIP", "off")
            .env("TIMONIER_RUNTIME_DIR", self.runtime.path().join("run"));
        let transport = TokioChildProcess::new(command).expect("tim mcp starts");
        Agent.serve(transport).await.expect("the MCP handshake")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::process::Command::new(assert_cmd::cargo::cargo_bin!("tim"))
            .args(["daemon", "stop", "--root"])
            .arg(self.repo.path())
            .env("TIMONIER_SCIP", "off")
            .env("TIMONIER_RUNTIME_DIR", self.runtime.path().join("run"))
            .output();
    }
}

/// A tool's structured content, and whether it is an error.
async fn call(
    agent: &RunningService<RoleClient, Agent>,
    name: &str,
    arguments: Value,
) -> (Value, bool) {
    let arguments: JsonObject = serde_json::from_value(arguments).expect("an object");
    let result = agent
        .call_tool(CallToolRequestParams::new(name.to_owned()).with_arguments(arguments))
        .await
        .expect("the call answers");
    let content = result
        .structured_content
        .clone()
        .expect("structured content");
    // ADR 0028: the header, in the structure and first in the text, on every result.
    assert_eq!(
        content["mailbox"],
        json!({"unread": 0, "priority": null}),
        "{content}"
    );
    let text = serde_json::to_value(&result.content).expect("content")[0]["text"].clone();
    assert!(
        text.as_str()
            .is_some_and(|t| t.starts_with("Mailbox: 0 unread.\n")),
        "{text}"
    );
    (content, result.is_error == Some(true))
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_keeps_its_session_through_the_proxy() {
    let project = Project::new();
    let agent = project.agent().await;
    // The tools are the daemon's catalogue, as the snapshot holds it.
    let tools = agent.list_all_tools().await.expect("tools/list");
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(
        names,
        [
            "get_budget_violations",
            "get_budgets",
            "get_component",
            "get_drift",
            "get_instrumentation_conventions",
            "get_instrumentation_gaps",
            "get_intended_architecture",
            "get_perf",
            "get_plan",
            "get_regressions",
            "get_session",
            "get_tech_inventory",
            "get_todo",
            "get_workflow",
            "link_ticket",
            "open_pull_request",
            "plan_add_task",
            "plan_update_task",
            "propose_component_name",
            "query_graph",
            "session_end",
            "session_start",
            "task_start",
            "todo_add",
            "todo_complete"
        ]
    );
    let catalogue: Value =
        serde_json::from_str(include_str!("../../timonier-core/tests/tools.json"))
            .expect("snapshot");
    assert_eq!(serde_json::to_value(&tools).expect("tools"), catalogue);

    // Refused without a session, with what to do.
    let (refused, error) = call(&agent, "todo_add", json!({"text": "x"})).await;
    assert!(error);
    assert_eq!(refused["error"]["code"], "no_open_session");

    let (session, error) = call(
        &agent,
        "session_start",
        json!({"objective": "Try the tools"}),
    )
    .await;
    assert!(!error, "{session}");
    // The agent is the MCP client's name.
    assert_eq!(session["result"]["agent"], "test-agent");
    let id = session["result"]["id"].as_str().expect("an id").to_owned();
    call(
        &agent,
        "plan_add_task",
        json!({"title": "Try", "new_milestone": "First"}),
    )
    .await;
    call(
        &agent,
        "todo_add",
        json!({"text": "Call every tool", "task": "T-1"}),
    )
    .await;
    let (bad, error) = call(
        &agent,
        "plan_update_task",
        json!({"task_id": "T-1", "status": "done"}),
    )
    .await;
    assert!(error);
    assert_eq!(bad["error"]["code"], "forbidden_transition");
    let (ended, error) = call(
        &agent,
        "session_end",
        json!({"summary": "All tools called."}),
    )
    .await;
    assert!(!error, "{ended}");
    assert!(
        ended["result"]["snapshots"]["end"].as_u64()
            > ended["result"]["snapshots"]["start"].as_u64()
    );

    // The daemon wrote the files, versioned, in the project.
    let journal = project
        .repo
        .path()
        .join(format!(".timonier/sessions/{id}.json"));
    let kept: Value =
        serde_json::from_str(&std::fs::read_to_string(journal).expect("journal")).expect("json");
    assert_eq!(kept["status"], "ended");
    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(project.repo.path().join(".timonier/plan.json")).expect("plan"),
    )
    .expect("json");
    assert_eq!(plan["milestones"][0]["tasks"][0]["id"], "T-1");
    // 6.4: a change made with no session open is untracked; one made in a session is not.
    std::fs::write(
        project.repo.path().join("src/lib.rs"),
        "pub fn f() {}\n\npub fn outside() {}\n",
    )
    .expect("an untracked change");
    let (second, error) = call(&agent, "session_start", json!({"objective": "Go on"})).await;
    assert!(!error, "{second}");
    std::fs::write(
        project.repo.path().join("src/lib.rs"),
        "pub fn f() {}\n\npub fn outside() {}\n\npub fn inside() {}\n",
    )
    .expect("a tracked change");
    call(&agent, "session_end", json!({"summary": "One function."})).await;
    call(&agent, "session_start", json!({"objective": "A third"})).await;
    let untracked: Value = serde_json::from_str(
        &std::fs::read_to_string(
            project
                .repo
                .path()
                .join(".timonier/sessions/untracked.json"),
        )
        .expect("untracked.json"),
    )
    .expect("json");
    let gaps = untracked["gaps"].as_array().expect("gaps");
    assert_eq!(gaps.len(), 1, "{untracked}");
    assert_eq!(gaps[0]["files"], json!(["src/lib.rs"]));
    assert_eq!(gaps[0]["until"]["session"], second["result"]["id"]);
    agent.cancel().await.expect("the agent goes");
}

/// Review n° 1 of Phase 6, B1: a client of MCP 2026-07-28 may skip `initialize` and name itself
/// in each request's metadata (SEP-2575); its sessions carry that name.
#[test]
fn a_client_named_in_its_requests_metadata_names_the_session() {
    use std::io::{BufRead, BufReader, Write};
    let project = Project::new();
    #[allow(clippy::zombie_processes)] // killed, then waited for below
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["mcp", "--root"])
        .arg(project.repo.path())
        .env("TIMONIER_SCIP", "off")
        .env("TIMONIER_RUNTIME_DIR", project.runtime.path().join("run"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("tim mcp starts");
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name": "claude-code", "version": "2.1"},
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "session_start",
            "arguments": {"objective": "Named in the metadata"},
            "_meta": meta,
        },
    });
    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(stdin, "{request}").expect("sent");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("an answer");
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let answer: Value = serde_json::from_str(&line).expect("JSON");
    assert_eq!(
        answer["result"]["structuredContent"]["result"]["agent"], "claude-code",
        "{answer}"
    );
}

/// PLAN.md 7.5, ADR 0041: an agent reads the whiteboard, its sketch as an image, and how the
/// indexed code stands against it, through the proxy and a real daemon.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_reads_the_whiteboard_and_the_drift_of_the_code() {
    let project = Project::new();
    let root = project.repo.path();
    // Two crates: `app` calls `store`.
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\", \"store\"]\n",
    )
    .expect("workspace");
    std::fs::remove_dir_all(root.join("src")).expect("no root crate");
    for (name, manifest, code) in [
        (
            "store",
            "[package]\nname = \"store\"\n",
            "pub fn save() {}\n",
        ),
        (
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\nstore = { path = \"../store\" }\n",
            "pub fn run() {\n    store::save();\n}\n",
        ),
    ] {
        std::fs::create_dir_all(root.join(name).join("src")).expect("src");
        std::fs::write(root.join(name).join("Cargo.toml"), manifest).expect("manifest");
        std::fs::write(root.join(name).join("src/lib.rs"), code).expect("lib");
    }
    let agent = project.agent().await;
    let (nothing, _) = call(&agent, "get_intended_architecture", json!({})).await;
    assert_eq!(nothing["result"]["drawn"], false);

    // The developer draws: the app may not depend on the store; a web front is wanted.
    std::fs::create_dir_all(root.join(".timonier")).expect(".timonier");
    std::fs::write(
        root.join(".timonier/intent.json"),
        r#"{"version": 1,
            "components": [{"id": "C-1", "name": "App", "layer": null, "role": "Runs"},
                           {"id": "C-2", "name": "Store", "layer": null, "role": "Keeps"},
                           {"id": "C-3", "name": "Web", "layer": null, "role": ""}],
            "edges": [{"id": "E-1", "src": "C-1", "dst": "C-2", "kind": "depends_on", "policy": "forbidden"}],
            "sketch": {"shapes": {"C-1": {"x": 0, "y": 0, "width": 180, "height": 80},
                                  "C-2": {"x": 300, "y": 0, "width": 180, "height": 80},
                                  "C-3": {"x": 600, "y": 0, "width": 180, "height": 80}},
                       "strokes": [{"id": "S-1", "x": 20, "y": 100, "points": [[0, 0, 0.5], [60, 8, 0.5]]}]}}"#,
    )
    .expect("intent.json");

    let result = agent
        .call_tool(CallToolRequestParams::new(
            "get_intended_architecture".to_owned(),
        ))
        .await
        .expect("the call answers");
    let content = serde_json::to_value(&result.content).expect("content");
    assert_eq!(content[1]["type"], "image", "{content}");
    assert_eq!(content[1]["mimeType"], "image/png");
    assert!(
        content[1]["data"]
            .as_str()
            .is_some_and(|d| d.starts_with("iVBORw0KGgo"))
    );
    let structured = result.structured_content.expect("structured content");
    assert_eq!(structured["result"]["components"][2]["name"], "Web");
    assert_eq!(structured["result"]["places"]["strokes"][0]["near"], "C-1");

    // Once the code is indexed, the drift: the forbidden edge is crossed, the web is missing.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let drift = loop {
        let (drift, error) = call(&agent, "get_drift", json!({})).await;
        assert!(!error, "{drift}");
        if !drift["result"]["forbidden"]
            .as_array()
            .is_none_or(Vec::is_empty)
        {
            break drift;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never crossed: {drift}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    };
    let result = &drift["result"];
    assert_eq!(result["anchors"][0]["actual"], "pkg:cargo/app");
    assert_eq!(result["anchors"][1]["actual"], "pkg:cargo/store");
    assert_eq!(result["missing"], json!(["C-3"]));
    assert_eq!(result["forbidden"][0]["edge"], "E-1");
    let crossed = &result["forbidden"][0]["dependencies"][0];
    assert!(
        crossed["from"].as_str().is_some_and(|f| f.contains("app"))
            && crossed["to"].as_str().is_some_and(|t| t.contains("store")),
        "{crossed}"
    );
    // Never written back: the whiteboard is the developer's.
    let kept = std::fs::read_to_string(root.join(".timonier/intent.json")).expect("kept");
    assert!(kept.starts_with(r#"{"version": 1,"#));
    agent.cancel().await.expect("the agent goes");
}

/// PLAN.md 11.10, ADR 0076: the agent reads the map as the developer sees it, and gives a box
/// a summary the map shows.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_reads_the_map_and_explains_a_box() {
    let project = Project::new();
    let root = project.repo.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\", \"store\"]\n",
    )
    .expect("workspace");
    std::fs::remove_dir_all(root.join("src")).expect("no root crate");
    for (name, manifest, code) in [
        (
            "store",
            "[package]\nname = \"store\"\n",
            "pub fn save() {}\n",
        ),
        (
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\nstore = { path = \"../store\" }\n",
            "pub fn run() {\n    store::save();\n}\n",
        ),
    ] {
        std::fs::create_dir_all(root.join(name).join("src")).expect("src");
        std::fs::write(root.join(name).join("Cargo.toml"), manifest).expect("manifest");
        std::fs::write(root.join(name).join("src/lib.rs"), code).expect("lib");
    }
    let agent = project.agent().await;

    // Once indexed, the store is used by the app.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let store = loop {
        let (store, error) = call(
            &agent,
            "get_component",
            json!({"component": "pkg:cargo/store"}),
        )
        .await;
        if !error
            && store["result"]["used_by"]
                .as_array()
                .is_some_and(|u| !u.is_empty())
        {
            break store;
        }
        assert!(std::time::Instant::now() < deadline, "never used: {store}");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    };
    let result = &store["result"];
    assert_eq!(result["name"], "store", "{result}");
    assert_eq!(result["files"], json!(["store/src/lib.rs"]), "{result}");
    assert_eq!(result["used_by"][0]["from"], "pkg:cargo/app", "{result}");
    assert!(
        result["used_by"][0]["links"].as_u64() >= Some(1),
        "{result}"
    );

    // The map's top, then what the app holds.
    let (top, error) = call(&agent, "query_graph", json!({})).await;
    assert!(!error, "{top}");
    let listed = |page: &Value| -> Vec<String> {
        page["result"]["nodes"]
            .as_array()
            .map(|n| {
                n.iter()
                    .filter_map(|n| n["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let at_top = listed(&top);
    assert!(!at_top.is_empty(), "{top}");
    let (open, error) = call(&agent, "query_graph", json!({"expanded": at_top})).await;
    assert!(!error, "{open}");
    assert!(
        listed(&open).len() > at_top.len() || listed(&open).iter().any(|id| id == "pkg:cargo/app"),
        "{open}"
    );
    let (one, _) = call(&agent, "query_graph", json!({"limit": 1})).await;
    assert_eq!(listed(&one).len(), 1, "{one}");
    assert_eq!(
        one["result"]["next_cursor"].is_string(),
        top["result"]["total"].as_u64() > Some(1)
    );

    // The agent explains the store; the map and get_component say it.
    let (named, error) = call(
        &agent,
        "propose_component_name",
        json!({"component": "pkg:cargo/store", "summary": "Keeps what the app saves."}),
    )
    .await;
    assert!(!error, "{named}");
    assert_eq!(named["result"]["summary"], "Keeps what the app saves.");
    let (store, _) = call(
        &agent,
        "get_component",
        json!({"component": "pkg:cargo/store"}),
    )
    .await;
    assert_eq!(
        store["result"]["summary"], "Keeps what the app saves.",
        "{store}"
    );
    let names = std::fs::read_to_string(root.join(".timonier/names.json")).expect("names.json");
    assert!(names.contains("Keeps what the app saves."), "{names}");

    // A file is no box; bounds are the daemon's to check.
    let (file, error) = call(
        &agent,
        "get_component",
        json!({"component": "file:store/src/lib.rs"}),
    )
    .await;
    assert!(error, "{file}");
    assert_eq!(file["error"]["code"], "not_found", "{file}");
    let (bad, error) = call(&agent, "query_graph", json!({"cursor": "x"})).await;
    assert!(error, "{bad}");
    assert_eq!(bad["error"]["code"], "invalid_argument", "{bad}");
    agent.cancel().await.expect("the agent goes");
}

/// PLAN.md T-110: the daemon stopped (an upgrade, a `kill`), the agent's next call still
/// answers: the proxy reaches the daemon again, and starts it when none answers.
#[tokio::test(flavor = "multi_thread")]
async fn the_proxy_reaches_a_daemon_that_went_again() {
    let project = Project::new();
    let agent = project.agent().await;
    let (plan, error) = call(&agent, "get_plan", json!({})).await;
    assert!(!error, "{plan}");
    let stopped = std::process::Command::new(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["daemon", "stop", "--root"])
        .arg(project.repo.path())
        .env("TIMONIER_SCIP", "off")
        .env("TIMONIER_RUNTIME_DIR", project.runtime.path().join("run"))
        .output()
        .expect("tim runs");
    assert!(stopped.status.success(), "{stopped:?}");
    // The proxy notices the connection closed; give it a moment.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let (again, error) = call(&agent, "get_plan", json!({})).await;
    assert!(!error, "{again}");
    assert_eq!(again["result"], plan["result"]);
    agent.cancel().await.expect("the agent goes");
}

/// A client of protocol 2026-07-28 skips `initialize` and says its revision in each request's
/// `_meta` (SEP-2567); the result of a tool must then say it is complete (SEP-2322), or Claude
/// Code refuses it (« missing required resultType », seen with 0.1.1 on 2026-10-23).
#[tokio::test(flavor = "multi_thread")]
async fn a_2026_07_28_client_gets_complete_results() {
    use std::io::{BufRead, Write};
    let project = Project::new();
    let mut proxy = std::process::Command::new(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["mcp", "--root"])
        .arg(project.repo.path())
        .env("TIMONIER_SCIP", "off")
        .env("TIMONIER_RUNTIME_DIR", project.runtime.path().join("run"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("timonier mcp starts");
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "1"},
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "get_plan", "arguments": {}, "_meta": meta}
    });
    let mut stdin = proxy.stdin.take().expect("stdin");
    writeln!(stdin, "{request}").expect("written");
    let mut line = String::new();
    std::io::BufReader::new(proxy.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("an answer");
    let _ = proxy.kill();
    let _ = proxy.wait();
    let answer: Value = serde_json::from_str(&line).expect("JSON");
    assert_eq!(answer["result"]["resultType"], "complete", "{answer}");
}
