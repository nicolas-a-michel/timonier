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
            "get_plan",
            "get_session",
            "get_todo",
            "plan_add_task",
            "plan_update_task",
            "session_end",
            "session_start",
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
