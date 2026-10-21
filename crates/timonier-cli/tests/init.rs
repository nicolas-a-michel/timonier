//! `timonier init` (PLAN.md 6.8, ADR 0033) on fixture projects, Rust and TypeScript, each a git
//! repository in a temporary directory, with the daemon's runtime and the home directory there
//! too: the developer's machine is never touched.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

struct Fixture {
    repo: tempfile::TempDir,
    home: tempfile::TempDir,
    runtime: tempfile::TempDir,
}

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let fixture = Self {
            repo: tempfile::tempdir().expect("temp dir"),
            home: tempfile::tempdir().expect("temp dir"),
            runtime: tempfile::tempdir().expect("temp dir"),
        };
        for (path, content) in files {
            let path = fixture.root().join(path);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("dir");
            std::fs::write(path, content).expect("a file");
        }
        fixture.git(&["init", "-q"]);
        fixture
    }

    fn root(&self) -> &Path {
        self.repo.path()
    }

    /// `timonier` and git's hooks find each other on the PATH, as on a real machine.
    fn path(&self) -> String {
        let bin = PathBuf::from(assert_cmd::cargo::cargo_bin!("timonier"));
        let dir = bin.parent().expect("a folder").display().to_string();
        // The runners' programs the packs ask for (ADR 0034), as installed.
        let tools = self.home.path().join("bin");
        if !tools.join("cargo-nextest").exists() {
            std::fs::create_dir_all(&tools).expect("bin");
            std::fs::write(tools.join("cargo-nextest"), "#!/bin/sh\n").expect("a program");
        }
        format!(
            "{dir}:{}:{}",
            tools.display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }

    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(self.root())
            .env("HOME", self.home.path())
            .env("PATH", self.path())
            .env_remove("CODEX_HOME")
            .env("TIMONIER_RUNTIME_DIR", self.runtime.path().join("run"))
            // The approved commands (Q84), never in the developer's home.
            .env(
                "TIMONIER_DATA_DIR",
                self.home.path().join(".local/share/timonier"),
            );
        command
    }

    fn tim(&self, args: &[&str]) -> Output {
        let output = self
            .command(assert_cmd::cargo::cargo_bin!("tim"))
            .args(args)
            .output()
            .expect("tim runs");
        assert!(
            output.status.success(),
            "tim {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn git(&self, args: &[&str]) -> Output {
        // A throwaway repository: no signing, a made-up author.
        let output = self
            .command(Path::new("git"))
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
        output
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root().join(path)).unwrap_or_else(|_| panic!("{path}"))
    }

    fn json(&self, path: &str) -> Value {
        serde_json::from_str(&self.read(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// Every file of the working tree, with its content, but the database and git's own.
    fn tree(&self) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![self.root().to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("a dir").flatten() {
                let path = entry.path();
                let name = path
                    .strip_prefix(self.root())
                    .expect("inside")
                    .display()
                    .to_string();
                if name == ".git" || name.starts_with(".timonier/graph.sqlite") {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push((name, std::fs::read(&path).expect("a file")));
                }
            }
        }
        files.sort();
        files
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self
            .command(assert_cmd::cargo::cargo_bin!("tim"))
            .args(["daemon", "stop"])
            .output();
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

const RUST: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    ),
    ("src/lib.rs", "pub mod a;\npub fn top() { a::f(); }\n"),
    ("src/a.rs", "pub fn f() {}\n"),
    ("CLAUDE.md", "# Demo\n\nMy own rules.\n"),
];

const TYPESCRIPT: &[(&str, &str)] = &[
    (
        "package.json",
        "{\"name\": \"demo\", \"scripts\": {\"test\": \"vitest run\"}}\n",
    ),
    ("tsconfig.json", "{}\n"),
    (
        "src/main.ts",
        "import { helper } from \"./util\";\nexport function run(): void { helper(); }\n",
    ),
    ("src/util.ts", "export function helper(): void {}\n"),
    (
        ".vscode/mcp.json",
        "{\n  \"servers\": {\n    \"other\": {\n      \"type\": \"stdio\",\n      \"command\": \"other\"\n    }\n  }\n}\n",
    ),
];

#[test]
fn init_configures_a_rust_project_once_and_shows_its_diff_first() {
    let project = Fixture::new(RUST);
    let before = project.tree();
    // --dry-run: the exact diff, and nothing written.
    let dry = stdout(&project.tim(&["init", "--dry-run", "--agents", "claude,codex"]));
    assert!(dry.contains("languages: rust"), "{dry}");
    assert!(dry.contains("--- a/CLAUDE.md\n+++ b/CLAUDE.md"), "{dry}");
    assert!(dry.contains("+@AGENTS.md"), "{dry}");
    assert!(dry.contains("--- /dev/null\n+++ b/.mcp.json"), "{dry}");
    assert!(
        dry.contains("session_start « Initialise Timonier »"),
        "{dry}"
    );
    assert_eq!(project.tree(), before, "a dry run writes nothing");
    // --detect (T-118): every agent, as JSON, and nothing written either.
    let detected: serde_json::Value =
        serde_json::from_str(&stdout(&project.tim(&["init", "--detect"]))).expect("JSON");
    let keys: Vec<&str> = detected["agents"]
        .as_array()
        .expect("agents")
        .iter()
        .filter_map(|a| a["key"].as_str())
        .collect();
    assert_eq!(keys, ["claude", "copilot", "cursor", "codex"]);
    assert!(detected["agents"][0]["found"].is_boolean());
    assert_eq!(project.tree(), before, "--detect writes nothing");

    let first = stdout(&project.tim(&["init", "--yes", "--agents", "claude,codex"]));
    assert!(first.contains("Timonier is set up."), "{first}");
    // The instructions, the import, the server, the hooks.
    assert!(
        project
            .read("AGENTS.md")
            .contains("<!-- timonier:begin -->\n## Timonier")
    );
    assert_eq!(
        project.read("CLAUDE.md"),
        "# Demo\n\nMy own rules.\n\n<!-- timonier:begin -->\n@AGENTS.md\n<!-- timonier:end -->\n"
    );
    assert_eq!(
        project.json(".mcp.json")["mcpServers"]["timonier"]["args"],
        serde_json::json!(["mcp"])
    );
    assert_eq!(
        project.json(".claude/settings.json")["hooks"]["Stop"][0]["hooks"][0]["command"],
        "timonier hook stop --agent claude"
    );
    assert!(
        project
            .read(".codex/config.toml")
            .contains("[mcp_servers.timonier]")
    );
    assert!(project.root().join(".codex/hooks.json").is_file());
    assert!(
        !project.root().join(".vscode").exists(),
        "Copilot was not chosen"
    );
    // The Rust pack (ADR 0034): nextest writes the JUnit XML, the conventions are copied.
    assert!(first.contains("tests: cargo-nextest (rust)"), "{first}");
    assert!(
        project
            .read(".config/nextest.toml")
            .contains("[profile.timonier.junit]")
    );
    let tests = project.json(".timonier/tests.json");
    assert_eq!(
        tests["runners"][0]["command"],
        "cargo nextest run --profile timonier"
    );
    assert!(
        project
            .read(".timonier/conventions/rust/instrumentation.md")
            .contains("tracing-opentelemetry")
    );
    assert!(
        project
            .read("AGENTS.md")
            .contains(".timonier/conventions/<language>/instrumentation.md")
    );
    assert!(
        !first.contains("install it (`cargo install cargo-nextest"),
        "nextest is there"
    );
    // The plan and the first session, through the daemon.
    let plan = project.json(".timonier/plan.json");
    assert_eq!(plan["milestones"][0]["title"], "Initialisation");
    assert_eq!(plan["milestones"][0]["tasks"][0]["status"], "done");
    let sessions: Vec<_> = std::fs::read_dir(project.root().join(".timonier/sessions"))
        .expect("sessions")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("S-"))
        .collect();
    assert_eq!(sessions.len(), 1);
    let session: Value =
        serde_json::from_str(&std::fs::read_to_string(sessions[0].path()).unwrap()).unwrap();
    assert_eq!(session["agent"], "timonier-init");
    assert_eq!(session["status"], "ended");

    // A second run changes nothing.
    let after = project.tree();
    let second = stdout(&project.tim(&["init", "--yes", "--agents", "claude,codex"]));
    assert!(second.contains("Nothing to change"), "{second}");
    assert_eq!(project.tree(), after);
    let dry = stdout(&project.tim(&["init", "--dry-run", "--agents", "claude,codex"]));
    assert!(!dry.contains("+++"), "{dry}");

    // The git hook: actual.json written and in the commit.
    project.git(&["add", "-A"]);
    project.git(&["commit", "-q", "-m", "first"]);
    let committed = stdout(&project.git(&["show", "--name-only", "--format=", "HEAD"]));
    assert!(committed.contains(".timonier/actual.json"), "{committed}");
    let actual = project.json(".timonier/actual.json");
    assert!(
        actual["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["id"] == "pkg:cargo/demo"),
        "{actual}"
    );
}

#[test]
fn init_configures_a_typescript_project_and_keeps_what_is_there() {
    let project = Fixture::new(TYPESCRIPT);
    let out = stdout(&project.tim(&["init", "--yes", "--agents", "copilot,cursor"]));
    assert!(out.contains("languages: typescript"), "{out}");
    assert!(out.contains("tests: vitest (typescript)"), "{out}");
    // How the tests write JUnit XML, and the conventions (ADR 0034).
    let tests = project.json(".timonier/tests.json");
    assert_eq!(tests["runners"][0]["runner"], "vitest");
    assert!(
        tests["runners"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--reporter=junit")
    );
    assert!(
        project
            .read(".timonier/conventions/typescript/instrumentation.md")
            .contains("code.function.name")
    );
    let vscode = project.json(".vscode/mcp.json");
    assert_eq!(
        vscode["servers"]["other"]["command"], "other",
        "theirs kept"
    );
    assert_eq!(vscode["servers"]["timonier"]["type"], "stdio");
    assert_eq!(
        vscode["servers"]["timonier"]["args"],
        serde_json::json!(["mcp", "--root", "${workspaceFolder}"])
    );
    assert_eq!(
        project.json(".github/hooks/timonier.json")["hooks"]["Stop"][0]["command"],
        "timonier hook stop --agent copilot"
    );
    assert_eq!(
        project.json(".cursor/mcp.json")["mcpServers"]["timonier"]["type"],
        "stdio"
    );
    assert!(
        !project.root().join(".cursor/hooks.json").exists(),
        "Cursor cannot hold a stop"
    );
    // No CLAUDE.md, no Claude: AGENTS.md alone, of Timonier's making.
    assert!(!project.root().join("CLAUDE.md").exists());
    let record = project.json(".timonier/init.json");
    assert!(
        record["created"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "AGENTS.md")
    );
    assert_eq!(record["modified"][0]["path"], ".vscode/mcp.json");
    let second = stdout(&project.tim(&["init", "--yes", "--agents", "copilot,cursor"]));
    assert!(second.contains("Nothing to change"), "{second}");
}

#[test]
fn the_stop_hook_holds_an_agent_once_in_its_format() {
    let project = Fixture::new(RUST);
    project.tim(&["init", "--yes", "--agents", "claude"]);
    // A change with no session open.
    std::fs::write(project.root().join("src/b.rs"), "pub fn g() {}\n").unwrap();
    let hook = |agent: &str, input: &str| -> String {
        use std::io::Write;
        let mut child = project
            .command(assert_cmd::cargo::cargo_bin!("timonier"))
            .args(["hook", "stop", "--agent", agent])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("the hook runs");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        stdout(&output)
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let held = loop {
        let out = hook("claude", r#"{"stop_hook_active": false}"#);
        if !out.is_empty() {
            break out;
        }
        assert!(std::time::Instant::now() < deadline, "never held");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let held: Value = serde_json::from_str(&held).unwrap();
    assert_eq!(held["decision"], "block");
    assert!(held["reason"].as_str().unwrap().contains("src/b.rs"));
    let copilot: Value = serde_json::from_str(&hook("copilot", "{}")).unwrap();
    assert_eq!(copilot["hookSpecificOutput"]["decision"], "block");
    // Held once: the second stop goes.
    assert_eq!(hook("claude", r#"{"stop_hook_active": true}"#), "");
}

/// PLAN.md 6.9: `init` then `uninit` gives the tree back as it was, byte for byte.
#[test]
fn uninit_gives_the_original_tree_back() {
    for (files, agents) in [
        (RUST, "claude,copilot,cursor,codex"),
        (TYPESCRIPT, "copilot,cursor"),
    ] {
        let project = Fixture::new(files);
        let original = project.tree();
        project.tim(&["init", "--yes", "--agents", agents]);
        assert_ne!(project.tree(), original);
        let dry = stdout(&project.tim(&["uninit", "--dry-run"]));
        assert!(dry.contains("+++ /dev/null"), "{dry}");
        assert!(
            project.root().join(".timonier").exists(),
            "a dry run takes nothing back"
        );
        let out = stdout(&project.tim(&["uninit"]));
        assert!(out.contains("no longer set up"), "{out}");
        assert_eq!(project.tree(), original, "{out}");
        let hook = project.root().join(".git/hooks/pre-commit");
        assert!(!hook.exists(), "the git hook it made is gone");
    }
}

/// Claude Code's telemetry goes into the user's settings, outside the project, for the AI cost
/// (ADR 0058, Q85); it stays out of `.timonier/init.json`, and `uninit` takes it back with the
/// last project that measures it.
#[test]
fn init_turns_claude_code_s_telemetry_on_in_the_home_and_uninit_takes_it_back() {
    let project = Fixture::new(RUST);
    let settings = project.home.path().join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let original = "{\n  \"model\": \"opus\"\n}\n";
    std::fs::write(&settings, original).unwrap();
    let dry = stdout(&project.tim(&["init", "--dry-run", "--agents", "claude"]));
    assert!(dry.contains("outside the project"), "{dry}");
    assert!(
        dry.contains("+    \"OTEL_LOGS_EXPORTER\": \"otlp\","),
        "{dry}"
    );
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), original);

    let out = stdout(&project.tim(&["init", "--yes", "--agents", "claude"]));
    assert!(out.contains("restart Claude Code"), "{out}");
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(written["model"], "opus");
    assert_eq!(written["env"]["CLAUDE_CODE_ENABLE_TELEMETRY"], "1");
    assert_eq!(
        written["env"]["OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"],
        "http://127.0.0.1:4319/v1/logs"
    );
    let record = project.read(".timonier/init.json");
    assert!(
        !record.contains(&project.home.path().display().to_string()),
        "{record}"
    );
    let projects = std::fs::read_to_string(
        project
            .home
            .path()
            .join(".local/share/timonier/agents/projects.json"),
    )
    .unwrap();
    assert!(
        projects.contains(&project.root().display().to_string()),
        "{projects}"
    );

    let out = stdout(&project.tim(&["uninit"]));
    assert!(out.contains("telemetry off"), "{out}");
    let back: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(back, serde_json::json!({"model": "opus"}));
}

/// A home folder is no project (T-119): `init` there refuses and writes nothing; under it, a
/// folder outside any repository does not resolve to it, even with a `.timonier/` left in it.
#[test]
fn the_home_folder_is_no_project() {
    let project = Fixture::new(RUST);
    let home = project.home.path();
    let refused = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["init", "--yes", "--agents", "claude", "--root"])
        .arg(home)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let said = String::from_utf8_lossy(&refused.stderr);
    assert!(said.contains("your home folder"), "{said}");
    assert!(!home.join(".timonier").exists());
    assert!(!home.join("AGENTS.md").exists());
    // A `.timonier/` left in the home: a folder under it is still not the home's project.
    std::fs::create_dir_all(home.join(".timonier")).unwrap();
    let below = home.join("notes");
    std::fs::create_dir_all(&below).unwrap();
    let status = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .current_dir(&below)
        .args(["init", "--dry-run", "--agents", "claude"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&status.stdout);
    assert!(
        out.contains(&format!("Timonier init in {}", below.display())),
        "{out}"
    );
}

/// What the developer added since `init` stays when `uninit` takes Timonier's part back.
#[test]
fn uninit_keeps_what_the_developer_added() {
    let project = Fixture::new(RUST);
    project.tim(&["init", "--yes", "--agents", "claude"]);
    let agents = project.read("AGENTS.md");
    std::fs::write(
        project.root().join("AGENTS.md"),
        format!("{agents}\n# Mine\n"),
    )
    .unwrap();
    project.tim(&["uninit"]);
    assert_eq!(project.read("AGENTS.md"), "# Mine\n");
    assert_eq!(project.read("CLAUDE.md"), "# Demo\n\nMy own rules.\n");
}

/// PLAN.md 6.10: `doctor` finds what was broken, repairs it with `--fix`, and `--check` says it
/// by its exit status.
#[test]
fn doctor_finds_and_repairs_the_setup() {
    let project = Fixture::new(RUST);
    let check = |project: &Fixture, args: &[&str]| {
        project
            .command(assert_cmd::cargo::cargo_bin!("tim"))
            .args(args)
            .output()
            .expect("tim runs")
    };
    let never = check(&project, &["doctor", "--check", "--no-smoke"]);
    assert!(!never.status.success());
    assert!(stdout(&never).contains("run `timonier init`"));
    project.tim(&["init", "--yes", "--agents", "claude,cursor"]);
    let fine = project.tim(&["doctor", "--check", "--no-smoke"]);
    assert!(stdout(&fine).contains("in order"), "{}", stdout(&fine));
    // A daemon that cannot start: its runtime directory cannot be made.
    let dead = check(&project, &["doctor", "--check", "--no-smoke"]);
    assert!(dead.status.success(), "the daemon above runs");
    let blocked = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["doctor", "--check", "--no-smoke"])
        .env("TIMONIER_RUNTIME_DIR", "/dev/null/run")
        .output()
        .expect("tim runs");
    assert!(!blocked.status.success());
    assert!(
        stdout(&blocked).contains("the daemon does not start"),
        "{}",
        stdout(&blocked)
    );
    std::fs::remove_file(project.root().join(".timonier/intent.json")).unwrap();
    let no_whiteboard = check(&project, &["doctor", "--check", "--no-smoke"]);
    assert!(
        stdout(&no_whiteboard).contains("the whiteboard (.timonier/intent.json) is missing"),
        "{}",
        stdout(&no_whiteboard)
    );
    std::fs::remove_file(project.root().join(".mcp.json")).unwrap();
    std::fs::write(project.root().join(".timonier/todo.json"), "{ broken").unwrap();
    let broken = check(&project, &["doctor", "--check", "--no-smoke"]);
    assert!(!broken.status.success());
    let said = stdout(&broken);
    assert!(
        said.contains("the MCP server is not declared for Claude Code (.mcp.json) (repairable)"),
        "{said}"
    );
    assert!(said.contains(".timonier/todo.json does not read"), "{said}");
    assert!(
        !project.root().join(".mcp.json").exists(),
        "--check repairs nothing"
    );
    let fixed = check(&project, &["doctor", "--fix", "--no-smoke"]);
    assert!(
        stdout(&fixed).contains("repaired .mcp.json"),
        "{}",
        stdout(&fixed)
    );
    assert!(
        !fixed.status.success(),
        "the broken todo.json is still to do by hand"
    );
    std::fs::write(
        project.root().join(".timonier/todo.json"),
        "{\"version\": 1, \"items\": []}\n",
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !check(&project, &["doctor", "--check", "--no-smoke"])
        .status
        .success()
    {
        assert!(std::time::Instant::now() < deadline, "never in order again");
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Without cargo-nextest, `init` says how to get the JUnit XML.
#[test]
fn a_missing_runner_program_is_said() {
    let project = Fixture::new(RUST);
    // The PATH without the fixture's tools.
    let bin = PathBuf::from(assert_cmd::cargo::cargo_bin!("timonier"));
    let out = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", bin.parent().unwrap().display()),
        )
        .args(["init", "--yes", "--agents", "claude"])
        .output()
        .expect("tim runs");
    let said = stdout(&out);
    assert!(
        said.contains(
            "cargo-nextest (rust): Timonier reads the tests' JUnit XML through cargo-nextest"
        ),
        "{said}"
    );
}

/// Q54: once an agent has tracked work, `uninit` keeps the plan and the journal; `--purge`
/// removes them, confirmed (`--yes` without a terminal).
#[test]
fn uninit_keeps_the_tracked_work_unless_purged() {
    use std::io::{BufRead, BufReader, Write};
    let project = Fixture::new(RUST);
    project.tim(&["init", "--yes", "--agents", "claude"]);
    // An agent's session, through the MCP server.
    #[allow(clippy::zombie_processes)] // killed, then waited for below
    let mut agent = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .arg("mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("tim mcp starts");
    let mut stdin = agent.stdin.take().unwrap();
    let mut answers = BufReader::new(agent.stdout.take().unwrap());
    // A request (with an id) gets its answer; a notification (id 0) gets none.
    let mut call = |id: u32, method: &str, params: Value| -> Value {
        if id == 0 {
            writeln!(
                stdin,
                "{}",
                serde_json::json!({"jsonrpc": "2.0", "method": method})
            )
            .unwrap();
            return Value::Null;
        }
        let request =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{request}").unwrap();
        let mut line = String::new();
        answers.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    call(
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "some-agent", "version": "1"}
        }),
    );
    call(0, "notifications/initialized", Value::Null);
    for (id, name, arguments) in [
        (2, "session_start", serde_json::json!({"objective": "Work"})),
        (3, "session_end", serde_json::json!({"summary": "Worked."})),
    ] {
        let answer = call(
            id,
            "tools/call",
            serde_json::json!({"name": name, "arguments": arguments}),
        );
        assert_eq!(answer["result"]["isError"], false, "{answer}");
    }
    let _ = agent.kill();
    let _ = agent.wait();

    let out = stdout(&project.tim(&["uninit"]));
    assert!(out.contains("--purge` removes them too"), "{out}");
    assert!(!project.root().join(".mcp.json").exists(), "the setup goes");
    assert!(!project.root().join(".timonier/init.json").exists());
    assert!(
        project.root().join(".timonier/plan.json").exists(),
        "the plan stays"
    );
    assert!(
        project.root().join(".timonier/.gitignore").exists(),
        "the database stays out of git"
    );
    // Purged: without a terminal, only with --yes.
    let refused = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["init", "--yes", "--agents", "claude"])
        .output()
        .unwrap();
    assert!(refused.status.success());
    let unconfirmed = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["uninit", "--purge"])
        .output()
        .unwrap();
    assert!(!unconfirmed.status.success());
    assert!(String::from_utf8_lossy(&unconfirmed.stderr).contains("add --yes"));
    assert!(project.root().join(".timonier/plan.json").exists());
    project.tim(&["uninit", "--purge", "--yes"]);
    // The folder was there before this `init`: the tracking goes, its database stays ignored.
    for gone in ["plan.json", "todo.json", "sessions", "init.json"] {
        assert!(
            !project.root().join(".timonier").join(gone).exists(),
            "{gone}"
        );
    }
}

/// Review n° 2 of Phase 6: the daemon (VSCode's) made `.timonier/` before `init`; `init` then
/// `uninit` still give the tree back, and the folder the daemon made stays as it was.
#[test]
fn uninit_takes_back_what_init_wrote_into_a_folder_the_daemon_made() {
    let project = Fixture::new(RUST);
    project.tim(&["daemon", "start"]);
    let original = project.tree();
    assert!(
        project.root().join(".timonier/.gitignore").exists(),
        "the daemon's folder"
    );
    project.tim(&["init", "--yes", "--agents", "claude"]);
    project.git(&["add", "-A"]);
    project.git(&["commit", "-q", "-m", "set up"]);
    assert!(
        project.root().join(".timonier/actual.json").exists(),
        "written by the git hook"
    );
    let out = stdout(&project.tim(&["uninit"]));
    assert!(!out.contains("they were there before"), "{out}");
    assert_eq!(project.tree(), original, "{out}");
}

/// Q57, ADR 0039: `init` puts an empty whiteboard; `uninit` takes it back while empty, and keeps
/// one the developer drew on unless `--purge`.
#[test]
fn init_puts_an_empty_whiteboard_that_uninit_keeps_once_drawn_on() {
    let empty = timonier_core::intent::Intent::empty().to_text();
    let project = Fixture::new(RUST);
    let intent = project.root().join(".timonier/intent.json");
    project.tim(&["init", "--yes", "--agents", "claude"]);
    assert_eq!(std::fs::read_to_string(&intent).unwrap(), empty);
    project.tim(&["uninit", "--yes"]);
    assert!(!intent.exists(), "still empty: init's");

    project.tim(&["init", "--yes", "--agents", "claude"]);
    // As the whiteboard writes it after a gesture, through the daemon.
    let drawn = timonier_core::intent::parse(
        r#"{"version": 1, "layers": [{"id": "L-1", "name": "Domain"}]}"#,
    )
    .unwrap()
    .to_text();
    std::fs::write(&intent, &drawn).unwrap();
    let out = stdout(&project.tim(&["uninit"]));
    assert_eq!(std::fs::read_to_string(&intent).unwrap(), drawn, "{out}");
    assert!(out.contains("--purge` removes them too"), "{out}");
    // A second `init` leaves the drawing as it is.
    project.tim(&["init", "--yes", "--agents", "claude"]);
    assert_eq!(std::fs::read_to_string(&intent).unwrap(), drawn);
}

/// PLAN.md 8.6, ADR 0050: `init` adds the OpenTelemetry SDK, its module and a smoke test to a
/// Rust project; `doctor` runs the smoke test under `timonier test`, and its trace comes back
/// attached to the graph. The fixture builds the real SDK: slow the first time, its build kept
/// in `target/tmp` for the next.
#[test]
fn doctor_s_smoke_test_produces_a_trace_attached_to_the_graph() {
    let project = Fixture::new(RUST);
    project.tim(&["init", "--yes", "--agents", "claude"]);
    let manifest = project.read("Cargo.toml");
    assert!(manifest.contains("otel = [\"dep:"), "{manifest}");
    assert!(
        project
            .read("src/timonier_otel.rs")
            .contains("pub fn init()")
    );
    assert!(
        project
            .read("tests/timonier_smoke.rs")
            .contains("fn timonier_smoke()")
    );
    assert!(
        project
            .read(".timonier/tests.json")
            .contains("timonier_smoke")
    );

    // Cargo and its toolchain as this machine has them, not in the fixture's empty home.
    let home = std::env::var("HOME").unwrap_or_default();
    let cargo_home = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{home}/.cargo"));
    let rustup_home = std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{home}/.rustup"));
    let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("smoke-fixture");
    let doctor = project
        .command(assert_cmd::cargo::cargo_bin!("tim"))
        .args(["doctor", "--check"])
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_TARGET_DIR", target)
        .env("TIMONIER_MEASURE", "rusage")
        .output()
        .expect("tim runs");
    let said = stdout(&doctor);
    assert!(
        said.contains("the smoke test (rust) produced a trace attached to the graph"),
        "{said}\n{}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    // Its run is a smoke test, which neither the budgets nor the views take (relecture n° 3).
    let perf = timonier_store::PerfStore::open(project.root().join(".timonier/perf/perf.sqlite"))
        .expect("the runs");
    let runs = perf.summaries(10).expect("the runs");
    assert!(!runs.is_empty() && runs.iter().all(|r| r.smoke), "{runs:?}");
    assert_eq!(perf.latest_traced().expect("the runs"), None);

    // And `uninit` takes it all back.
    project.tim(&["uninit", "--yes"]);
    assert_eq!(project.read("Cargo.toml"), RUST[0].1);
    assert!(!project.root().join("src/timonier_otel.rs").exists());
    assert!(!project.root().join("tests").exists());
}
