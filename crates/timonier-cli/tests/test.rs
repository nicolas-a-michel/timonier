//! `timonier test` (PLAN.md 8.2, ADR 0043, 0045): a fixture project whose tests are a script
//! that works, allocates, writes a JUnit XML and fails; the run is measured and kept by a real
//! daemon.

// Their fake programs are shell scripts, on a `:`-separated PATH (T-125).
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use assert_cmd::cargo::cargo_bin_cmd;
use timonier_core::perf::{Accounting, RunRecord, TestStatus};
use timonier_store::{PerfStore, StoredRun};

/// The tests of the fixture: some work, 30 MB that `sort` holds, a report, a failure.
const SCRIPT: &str = r#"#!/bin/sh
printf '%s\n%s\n' "$OTEL_EXPORTER_OTLP_ENDPOINT" "$OTEL_RESOURCE_ATTRIBUTES" > otel.txt
sleep 0.3
head -c 30000000 /dev/zero | sort > /dev/null
mkdir -p reports
cat > reports/junit.xml <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="fixture" tests="3" failures="1">
  <testsuite name="demo" tests="3" failures="1">
    <testcase name="parses" classname="demo::parser" time="0.25" timestamp="2024-01-09T07:50:12.665+00:00"/>
    <testcase name="renders" classname="demo::view" time="0.5" file="src/view.rs" line="12"/>
    <testcase name="fails" classname="demo::view" time="0.01"><failure message="no"/></testcase>
  </testsuite>
</testsuites>
XML
exit 1
"#;

/// A project with its tests, in git, and a daemon stopped when the test ends, even failed.
struct Project {
    repo: tempfile::TempDir,
    runtime: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let repo = tempfile::tempdir().expect("temp dir");
        let write = |path: &str, text: &str| {
            let path = repo.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("dir");
            std::fs::write(path, text).expect("write");
        };
        write("Cargo.toml", "[package]\nname = \"demo\"\n");
        write(
            "src/lib.rs",
            "pub fn f() {\n    g();\n}\n\npub fn g() {}\n\npub fn h() {}\n\nfn p() {}\n",
        );
        write("run-tests.sh", SCRIPT);
        write(
            ".timonier/tests.json",
            r#"{"version": 1, "runners": [{"language": "rust", "runner": "fixture",
                "command": "sh run-tests.sh", "junit": "reports/junit.xml"}]}"#,
        );
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .status()
                .expect("git runs");
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "--no-gpg-sign", "-m", "fixture"]);
        Self {
            repo,
            runtime: tempfile::tempdir().expect("temp dir"),
        }
    }

    fn root(&self) -> PathBuf {
        self.repo.path().canonicalize().expect("canonical root")
    }

    fn tim(&self, args: &[&str], rusage: bool) -> Output {
        self.tim_with(args, rusage, &[])
    }

    /// `tim` with more variables in its environment, which the tests' command inherits.
    fn tim_with(&self, args: &[&str], rusage: bool, variables: &[(&str, &str)]) -> Output {
        let mut command = cargo_bin_cmd!("tim");
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("TIMONIER_SCIP", "off")
            .env("TIMONIER_RUNTIME_DIR", self.runtime.path().join("run"))
            // The approved commands (Q84), never in the developer's home.
            .env("TIMONIER_DATA_DIR", self.runtime.path().join("data"))
            // The project's own attributes, which the run's go after.
            .env("OTEL_RESOURCE_ATTRIBUTES", "service.name=demo")
            .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT");
        if rusage {
            command.env("TIMONIER_MEASURE", "rusage");
        }
        for (name, value) in variables {
            command.env(name, value);
        }
        command.output().expect("tim runs")
    }

    fn perf(&self) -> PerfStore {
        PerfStore::open(self.root().join(".timonier/perf/perf.sqlite")).expect("the measurements")
    }

    fn run(&self, id: u64) -> StoredRun {
        self.perf()
            .run(id)
            .expect("readable")
            .expect("the run is kept")
    }

    /// What was measured of a finished run.
    fn record(&self, id: u64) -> RunRecord {
        self.run(id).record.expect("the run finished")
    }
}

impl Project {
    /// Its tests become `examples/emit_spans.rs`, instrumented with a real exporter, which sits
    /// next to the test binaries' folder (`target/<profile>/examples`).
    fn instrumented_tests(&self) {
        let exe = std::env::current_exe().expect("the test binary");
        let emit = exe
            .parent()
            .and_then(Path::parent)
            .map(|dir| dir.join("examples").join("emit_spans"))
            .expect("the examples folder");
        assert!(
            emit.is_file(),
            "{} (cargo test builds the examples)",
            emit.display()
        );
        std::fs::write(
            self.root().join("run-tests.sh"),
            format!(
                "#!/bin/sh\n{}\nmkdir -p reports\ncat > reports/junit.xml <<'XML'\n\
                 <testsuite name=\"demo\" tests=\"1\"><testcase name=\"parses\" time=\"0.2\"/></testsuite>\n\
                 XML\n",
                emit.display()
            ),
        )
        .expect("the script");
    }

    /// A request to the project's daemon: its result.
    fn rpc(&self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let paths = timonier_daemon::runtime::Paths::in_runtime(
            &self.root(),
            &self.runtime.path().join("run"),
        )
        .expect("the daemon's paths");
        let mut client =
            timonier_daemon::client::Client::connect(&paths.socket).expect("the daemon answers");
        client.initialize("test-views").expect("the handshake");
        client.request(method, &params).expect("the daemon answers")
    }

    /// A tool of the MCP catalogue, called on the project's daemon: its structured content.
    fn tool(&self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let paths = timonier_daemon::runtime::Paths::in_runtime(
            &self.root(),
            &self.runtime.path().join("run"),
        )
        .expect("the daemon's paths");
        let mut client =
            timonier_daemon::client::Client::connect(&paths.socket).expect("the daemon answers");
        client.initialize("test-agent").expect("the handshake");
        let result: serde_json::Value = client
            .request(
                timonier_core::protocol::methods::TOOLS_CALL,
                &serde_json::json!({ "name": name, "arguments": arguments }),
            )
            .expect("the tool answers");
        result["structuredContent"].clone()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.tim(&["daemon", "stop"], false);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn head(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn a_run_of_the_fixture_is_measured_and_kept() {
    let project = Project::new();
    let output = project.tim(&["test", "--", "sh", "run-tests.sh"], false);
    // The command's own exit status.
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let said = stderr(&output);
    assert!(said.contains("timonier: R-1 · "), "{said}");
    assert!(said.contains("3 tests, 1 failed"), "{said}");

    let run = project.run(1);
    let record = project.record(1);
    let measured = &record.measured;
    assert_eq!(measured.exit_code, Some(1));
    assert!(measured.wall_us >= 300_000, "{measured:?}");
    assert!(
        measured.user_cpu_us + measured.system_cpu_us > 0,
        "{measured:?}"
    );
    // `sort` holds its 30 MB: the largest process, and the whole tree in a scope.
    assert!(measured.largest_process_bytes > 25_000_000, "{measured:?}");
    if measured.accounting == Accounting::SystemdScope {
        assert!(
            measured
                .tree_peak_bytes
                .is_some_and(|peak| peak >= 25_000_000),
            "{measured:?}"
        );
    }
    // The JUnit XML of the runner whose command it is (.timonier/tests.json).
    assert_eq!(record.junit.as_deref(), Some("reports/junit.xml"));
    let tests: Vec<_> = record
        .tests
        .iter()
        .map(|t| (t.name.as_str(), t.status, t.duration_us))
        .collect();
    assert_eq!(
        tests,
        [
            ("parses", TestStatus::Passed, 250_000),
            ("renders", TestStatus::Passed, 500_000),
            ("fails", TestStatus::Failed, 10_000),
        ]
    );
    assert_eq!(record.tests[1].file.as_deref(), Some("src/view.rs"));
    assert_eq!(record.tests[1].line, Some(12));
    assert_eq!(record.tests[0].started_at_us, Some(1_704_786_612_665_000));
    // Where it sits: git's view, and the machine.
    assert_eq!(
        run.place.commit.as_deref(),
        Some(head(&project.root()).as_str())
    );
    assert_eq!(run.place.branch.as_deref(), Some("main"));
    assert_eq!(run.fingerprint.as_ref().map(String::len), Some(16));
    assert_eq!(run.command, ["sh", "run-tests.sh"]);
    // The measurements keep themselves out of git.
    let ignore = std::fs::read_to_string(project.root().join(".timonier/perf/.gitignore"))
        .expect("the folder's .gitignore");
    assert!(ignore.lines().any(|l| l == "*"), "{ignore}");
}

#[test]
fn without_a_command_the_recorded_runners_run_and_rusage_measures_alone() {
    let project = Project::new();
    // The repository's commands: not run until this machine approves them (Q84).
    let refused = project.tim(&["test"], true);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    let said = stderr(&refused);
    assert!(said.contains("not approved:\n  sh run-tests.sh"), "{said}");
    assert!(said.contains("timonier test --approve"), "{said}");
    assert!(
        !project.root().join(".timonier/perf").exists(),
        "no run was made"
    );
    let output = project.tim(&["test", "--approve"], true);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    // Approved once: the next run asks nothing.
    let again = project.tim(&["test"], true);
    assert_eq!(again.status.code(), Some(1), "{again:?}");
    let record = project.record(1);
    assert_eq!(record.command, ["sh", "-c", "sh run-tests.sh"]);
    assert_eq!(record.measured.accounting, Accounting::Rusage);
    assert_eq!(record.measured.tree_peak_bytes, None);
    assert!(record.measured.samples.is_empty());
    assert_eq!(record.tests.len(), 3);
}

#[test]
fn a_report_the_run_did_not_write_is_not_read() {
    let project = Project::new();
    // An old report, from before this run; the command writes none.
    std::fs::create_dir_all(project.root().join("reports")).expect("dir");
    std::fs::write(project.root().join("reports/junit.xml"), "<testsuite/>").expect("report");
    std::thread::sleep(std::time::Duration::from_millis(20));
    let output = project.tim(
        &["test", "--junit", "reports/junit.xml", "--", "true"],
        true,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stderr(&output).contains("no test durations"), "{output:?}");
    let record = project.record(1);
    assert!(record.tests.is_empty());
    assert!(
        record
            .junit_problem
            .as_deref()
            .is_some_and(|p| p.contains("older than the run")),
        "{:?}",
        record.junit_problem
    );
}

#[test]
fn a_command_that_cannot_run_says_so() {
    let project = Project::new();
    let output = project.tim(&["test", "--", "no-such-program-here"], true);
    assert_eq!(output.status.code(), Some(127), "{output:?}");
    assert!(
        stderr(&output).contains("cannot run `no-such-program-here`"),
        "{output:?}"
    );
}

/// PLAN.md 8.3, ADR 0047: the command sees where its traces go and which run and snapshot they
/// belong to, after the project's own attributes; each test of its JUnit XML becomes a root
/// span of the run.
#[test]
fn the_tests_are_told_their_run_and_each_test_becomes_a_root_span() {
    let project = Project::new();
    let output = project.tim(&["test", "--", "sh", "run-tests.sh"], true);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = project.run(1);
    let seen = std::fs::read_to_string(project.root().join("otel.txt")).expect("the script ran");
    let mut lines = seen.lines();
    // The receiver's port: 4318, or one of the system's choice when another daemon holds it.
    let endpoint = lines.next().expect("the endpoint");
    let port = endpoint
        .strip_prefix("http://127.0.0.1:")
        .expect("on the loopback");
    assert!(port.parse::<u16>().is_ok_and(|p| p > 0), "{endpoint}");
    assert_eq!(
        lines.next(),
        Some(
            format!(
                "service.name=demo,timonier.run_id=R-1,timonier.snapshot={}",
                run.place.snapshot
            )
            .as_str()
        )
    );

    let spans = project.perf().spans(1).expect("the spans");
    let roots: Vec<_> = spans
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.parent_span_id.is_none(),
                s.synthesized,
                s.test,
                s.duration_ns,
            )
        })
        .collect();
    assert_eq!(
        roots,
        [
            ("parses", true, true, Some(0), 250_000_000),
            ("renders", true, true, Some(1), 500_000_000),
            ("fails", true, true, Some(2), 10_000_000),
        ]
    );
    assert_eq!(spans[0].start_ns, Some(1_704_786_612_665_000_000));
    assert_eq!(spans[1].start_ns, None);
    assert_eq!(spans[2].test_classname.as_deref(), Some("demo::view"));
    assert_eq!(spans[2].attributes["timonier.test.status"], "failed");
}

/// ADR 0047: a run has its number from its start, and each run its own.
#[test]
fn a_run_is_numbered_when_it_begins() {
    let project = Project::new();
    project.tim(&["test", "--", "true"], true);
    project.tim(&["test", "--", "true"], true);
    assert!(project.record(1).tests.is_empty());
    assert_eq!(project.run(2).command, ["true"]);
}

/// PLAN.md 8.4, ADR 0048: a test process instrumented with a real OTLP exporter sends its spans
/// to the run's receiver; they are kept with their test and their node of the graph.
#[test]
fn the_spans_of_a_real_exporter_come_back_attached_to_their_test_and_node() {
    let project = Project::new();
    project.instrumented_tests();
    let output = project.tim(&["test", "--", "sh", "run-tests.sh"], true);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let spans = project.perf().spans(1).expect("the spans");
    let of = |name: &str| {
        spans
            .iter()
            .find(|s| s.name == name && !s.synthesized)
            .unwrap_or_else(|| panic!("no span {name}: {spans:#?}"))
    };
    let (load, draw, inline) = (of("load"), of("draw"), of("inline"));
    // Their test, the JUnit's `parses`, whose root Timonier made.
    let root = spans
        .iter()
        .find(|s| s.synthesized)
        .expect("the test's root");
    assert_eq!(root.name, "parses");
    for span in [load, draw, inline] {
        assert_eq!(span.test, Some(0), "{span:?}");
    }
    // As the exporter nested them.
    assert_eq!(draw.parent_span_id.as_deref(), Some(load.span_id.as_str()));
    assert_eq!(load.trace_id, inline.trace_id);
    // Their node: by ID, by name in the file, by line.
    use timonier_core::perf::Attachment;
    assert_eq!(
        (load.node.as_deref(), load.attached_by),
        (Some("rust:demo::f"), Some(Attachment::Id))
    );
    assert_eq!(
        (draw.node.as_deref(), draw.attached_by),
        (Some("rust:demo::g"), Some(Attachment::Name))
    );
    assert_eq!(
        (inline.node.as_deref(), inline.attached_by),
        (Some("rust:demo::f"), Some(Attachment::Line))
    );
    assert!(load.duration_ns > 0 && load.start_ns.is_some());
}

/// PLAN.md 8.5, ADR 0049: a fixture with known gaps (a public function no test reaches, a
/// private one, a span with no code, one with its file only) makes them all show, with the
/// overhead against a run without traces.
#[test]
fn a_fixture_s_known_gaps_all_show() {
    let project = Project::new();
    project.instrumented_tests();
    // The daemon starts with the first run; before it, there is no daemon to ask.
    let traced = project.tim(&["test", "--", "sh", "run-tests.sh"], true);
    assert_eq!(traced.status.code(), Some(0), "{traced:?}");
    let untraced = project.tim(
        &["test", "--without-traces", "--", "sh", "run-tests.sh"],
        true,
    );
    assert_eq!(untraced.status.code(), Some(0), "{untraced:?}");
    assert!(
        project
            .perf()
            .spans(2)
            .expect("spans")
            .iter()
            .all(|s| s.synthesized)
    );

    let gaps = project.tool("get_instrumentation_gaps", serde_json::json!({}));
    let report = &gaps["result"];
    assert_eq!(report["run"], "R-1", "{gaps}");
    // `f` and `g` are seen, `h` is not; `p` is private.
    assert_eq!(report["public_functions"], 3, "{gaps}");
    assert_eq!(report["never_seen_count"], 1, "{gaps}");
    assert_eq!(report["never_seen"][0]["id"], "rust:demo::h");
    assert_eq!(report["never_seen"][0]["file"], "src/lib.rs");
    assert_eq!(report["never_seen"][0]["line"], 7);
    let without: Vec<(String, Vec<String>)> = report["without_code"]
        .as_array()
        .expect("spans without code")
        .iter()
        .map(|w| {
            (
                w["name"].as_str().unwrap_or_default().to_owned(),
                serde_json::from_value(w["missing"].clone()).unwrap_or_default(),
            )
        })
        .collect();
    let all = ["code.function.name", "code.file.path", "code.line.number"].map(String::from);
    assert!(
        without.contains(&("bare".to_owned(), all.to_vec())),
        "{without:?}"
    );
    assert!(
        without.contains(&("file only".to_owned(), vec![all[0].clone(), all[2].clone()])),
        "{without:?}"
    );
    // The bare span has nothing to attach by; the other, its file.
    assert_eq!(report["unattached"], 1, "{gaps}");
    // Against the run without traces of the same commit and command.
    assert_eq!(
        report["overhead"]["traced_runs"],
        serde_json::json!(["R-1"]),
        "{gaps}"
    );
    assert_eq!(
        report["overhead"]["untraced_runs"],
        serde_json::json!(["R-2"])
    );
    assert!(report["overhead"]["percent"].is_number(), "{gaps}");

    // Paged, and checked by the daemon.
    let page = project.tool(
        "get_instrumentation_gaps",
        serde_json::json!({"limit": 1, "cursor": "1"}),
    );
    assert_eq!(
        page["result"]["never_seen"],
        serde_json::json!([]),
        "{page}"
    );
    let wrong = project.tool(
        "get_instrumentation_gaps",
        serde_json::json!({"run": "R-x"}),
    );
    assert_eq!(wrong["error"]["code"], "invalid_argument", "{wrong}");
    let absent = project.tool(
        "get_instrumentation_gaps",
        serde_json::json!({"run": "R-9"}),
    );
    assert_eq!(absent["error"]["code"], "no_run", "{absent}");
}

#[test]
fn the_conventions_of_a_language_are_those_init_copied() {
    let project = Project::new();
    project.tim(&["test", "--", "true"], true);
    let none = project.tool(
        "get_instrumentation_conventions",
        serde_json::json!({"language": "rust"}),
    );
    assert_eq!(none["error"]["code"], "unknown_language", "{none}");
    let folder = project.root().join(".timonier/conventions/rust");
    std::fs::create_dir_all(&folder).expect("dir");
    std::fs::write(
        folder.join("instrumentation.md"),
        "# Rust\nOne span per public function.\n",
    )
    .expect("conventions");
    let read = project.tool(
        "get_instrumentation_conventions",
        serde_json::json!({"language": "rust"}),
    );
    assert_eq!(
        read["result"]["conventions"], "# Rust\nOne span per public function.\n",
        "{read}"
    );
    let other = project.tool(
        "get_instrumentation_conventions",
        serde_json::json!({"language": "python"}),
    );
    assert_eq!(other["error"]["code"], "unknown_language");
    assert!(
        other["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("rust")),
        "{other}"
    );
}

/// PLAN.md 8.7, ADR 0051: what the performance views read of a run with real spans: the runs
/// listed, one run's tests and spans from its start, the map's layer with each node's own time,
/// the traffic between nodes and their lineage.
#[test]
fn the_views_read_a_run_its_spans_and_the_map_s_layer() {
    use timonier_core::protocol::methods;
    let project = Project::new();
    project.instrumented_tests();
    let output = project.tim(&["test", "--", "sh", "run-tests.sh"], true);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let runs = project.rpc(methods::PERF_RUNS, serde_json::Value::Null);
    let listed = &runs["runs"][0];
    assert_eq!(listed["run"], "R-1", "{runs}");
    assert_eq!(
        (listed["tests"].as_u64(), listed["spans"].as_u64()),
        (Some(1), Some(5))
    );
    assert_eq!(listed["finished"], true);

    let run = project.rpc(methods::PERF_RUN, serde_json::json!({"run": "R-1"}));
    let spans = run["spans"].as_array().expect("spans");
    assert_eq!(spans.len(), 6, "five received and the test's root: {run}");
    assert_eq!(spans[0]["synthesized"], true, "the root first");
    let load = spans.iter().find(|s| s["name"] == "load").expect("load");
    assert_eq!(load["node"], "rust:demo::f");
    assert_eq!(load["test"], 0);
    // Times from the run's start: small, never epoch nanoseconds.
    assert!(
        load["startNs"]
            .as_u64()
            .is_some_and(|ns| ns < 600_000_000_000),
        "{load}"
    );
    assert_eq!(run["nodes"]["rust:demo::f"], "f");
    assert_eq!(run["tests"][0]["name"], "parses");

    let map = project.rpc(methods::PERF_MAP, serde_json::json!({}));
    assert_eq!(map["run"], "R-1", "{map}");
    let node = |id: &str| {
        map["nodes"]
            .as_array()
            .and_then(|n| n.iter().find(|n| n["node"] == id))
            .cloned()
            .unwrap_or_default()
    };
    // `load` and `inline` in f, `draw` in g.
    assert_eq!(node("rust:demo::f")["calls"], 2, "{map}");
    assert_eq!(node("rust:demo::g")["calls"], 1, "{map}");
    assert!(
        map["links"].as_array().is_some_and(|l| l
            .iter()
            .any(|l| l["from"] == "rust:demo::f" && l["to"] == "rust:demo::g")),
        "{map}"
    );
    let lineage = map["ancestors"]["rust:demo::f"]
        .as_array()
        .expect("its lineage");
    assert_eq!(
        lineage.last(),
        Some(&serde_json::json!("file:src/lib.rs")),
        "{map}"
    );
    // A run that is not there.
    let none = project.rpc(methods::PERF_RUN, serde_json::json!({"run": "R-9"}));
    assert!(none.is_null(), "{none}");
}

/// PLAN.md 8.8, ADR 0052: a budget of the whiteboard exceeded on purpose (a p95 of a
/// nanosecond on the crate `demo`) is read through MCP, given to the map with its box, and
/// written in the journal of the session that ends; a generous one holds.
#[test]
fn a_budget_exceeded_on_purpose_shows_on_the_map_through_mcp_and_in_the_journal() {
    use timonier_core::protocol::methods;
    let project = Project::new();
    project.instrumented_tests();
    std::fs::write(
        project.root().join(".timonier/intent.json"),
        r#"{
  "version": 1,
  "components": [{ "id": "C-1", "name": "demo", "layer": null, "role": "the parser" }],
  "budgets": [
    { "id": "N-1", "attached_to": "C-1", "p95_ms": 0.000001 },
    { "id": "N-2", "attached_to": "C-1", "calls": 1000 }
  ]
}
"#,
    )
    .expect("the fixture's whiteboard");
    // An agent's session, open while the run happens, on the project's daemon.
    project.tim(&["daemon", "start"], false);
    let opened = project.tool(
        "session_start",
        serde_json::json!({"objective": "speed the parser up"}),
    );
    assert!(opened["result"]["id"].is_string(), "{opened}");
    let output = project.tim(&["test", "--", "sh", "run-tests.sh"], true);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // What the CI replays: `timonier check` fails on it (ADR 0056).
    let checked = project.tim(&["check"], true);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(
        stderr(&checked).contains("timonier: budget N-1 exceeded on demo: p95_ms "),
        "{}",
        stderr(&checked)
    );
    // Said at the end of the run too (relecture n° 1, m10).
    let said = stderr(&output);
    assert!(
        said.contains("timonier: budget N-1 exceeded on demo: p95_ms "),
        "{said}"
    );
    assert!(said.contains("timonier: instrumentation: Run R-"), "{said}");

    let violations = project.tool("get_budget_violations", serde_json::json!({}));
    let exceeded = violations["result"]["budgets"].as_array().expect("budgets");
    assert_eq!(exceeded.len(), 1, "{violations}");
    assert_eq!(exceeded[0]["id"], "N-1");
    assert_eq!(exceeded[0]["on"], "demo");
    assert_eq!(exceeded[0]["state"], "exceeded");
    assert_eq!(exceeded[0]["limits"][0]["kind"], "p95_ms");
    assert!(
        exceeded[0]["limits"][0]["measured"]
            .as_f64()
            .is_some_and(|ms| ms > 0.0)
    );
    let all = project.tool("get_budgets", serde_json::json!({}));
    let states: Vec<_> = all["result"]["budgets"]
        .as_array()
        .expect("budgets")
        .iter()
        .map(|b| (b["id"].clone(), b["state"].clone()))
        .collect();
    assert_eq!(
        states,
        [
            (serde_json::json!("N-1"), serde_json::json!("exceeded")),
            (serde_json::json!("N-2"), serde_json::json!("ok"))
        ],
        "{all}"
    );

    // The map's data: the box that shows it, with its lineage.
    let map = project.rpc(methods::PERF_BUDGETS, serde_json::Value::Null);
    assert_eq!(
        map["checks"][0]["boxes"],
        serde_json::json!(["pkg:cargo/demo"]),
        "{map}"
    );
    assert!(map["ancestors"]["pkg:cargo/demo"].is_array(), "{map}");

    // What the crate cost: its incoming calls, their latency, what inside took time.
    let perf = project.tool("get_perf", serde_json::json!({"component": "demo"}));
    let result = &perf["result"];
    assert_eq!(result["node"], "pkg:cargo/demo", "{perf}");
    assert!(result["calls"].as_u64().is_some_and(|n| n > 0), "{perf}");
    assert!(result["p95_ms"].is_number(), "{perf}");
    assert_eq!(result["tests"], serde_json::json!(["parses"]));
    assert_eq!(result["budgets"].as_array().map(Vec::len), Some(2));
    let unknown = project.tool("get_perf", serde_json::json!({"component": "nowhere"}));
    assert_eq!(unknown["error"]["code"], "no_measure", "{unknown}");

    // The journal of the session keeps it.
    let ended = project.tool("session_end", serde_json::json!({"summary": "measured"}));
    let budgets = ended["result"]["budgets"]
        .as_array()
        .expect("the session's budgets");
    assert_eq!(budgets.len(), 1, "{ended}");
    assert!(
        budgets[0]
            .as_str()
            .is_some_and(|b| b.starts_with("N-1 on demo: p95")),
        "{ended}"
    );

    // Relecture n° 2, I-1: a run that reaches nothing of the crate (another runner, a single
    // test elsewhere) leaves the budget as it was judged.
    let elsewhere = project.tim(&["test", "--", "sh", "-c", "true"], true);
    assert_eq!(elsewhere.status.code(), Some(0), "{elsewhere:?}");
    let violations = project.tool("get_budget_violations", serde_json::json!({}));
    assert_eq!(
        violations["result"]["budgets"][0]["id"], "N-1",
        "still exceeded: {violations}"
    );
}

/// PLAN.md 8.9, ADR 0053: a function slowed on purpose is a regression against the main
/// branch: « probable » in the batch that sees it first, « confirmed » in the next one; what
/// did not change is not one.
#[test]
fn a_function_slowed_on_purpose_goes_from_probable_to_confirmed() {
    let project = Project::new();
    project.instrumented_tests();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(project.root())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    // The runner's reports stay out of git, as `target/` does: a run would see a modified tree
    // otherwise, kept out of the reference (Q77).
    std::fs::write(project.root().join(".gitignore"), "reports/\n").expect("gitignore");
    git(&["add", "-A"]);
    git(&["commit", "-q", "--no-gpg-sign", "-m", "the reference"]);
    let reference = project.tim(&["test", "--runs", "5", "--", "sh", "run-tests.sh"], true);
    assert_eq!(reference.status.code(), Some(0), "{reference:?}");
    assert!(
        stderr(&reference).contains("no reference yet"),
        "{}",
        stderr(&reference)
    );

    // The next commit makes `f` slower by 40 ms.
    git(&[
        "commit",
        "-q",
        "--no-gpg-sign",
        "--allow-empty",
        "-m",
        "slower",
    ]);
    let slow = [("TIMONIER_FIXTURE_SLOW_MS", "40")];
    let first = project.tim_with(
        &["test", "--runs", "5", "--", "sh", "run-tests.sh"],
        true,
        &slow,
    );
    let said = stderr(&first);
    assert!(
        said.contains("timonier: regression probable on f:"),
        "{said}"
    );
    assert!(said.contains("run the tests again to confirm it"), "{said}");
    let report = project.tool("get_regressions", serde_json::json!({}));
    let found = report["result"]["regressions"]
        .as_array()
        .expect("regressions");
    let f = found
        .iter()
        .find(|r| r["key"] == "node:rust:demo::f")
        .unwrap_or_else(|| panic!("{report}"));
    assert_eq!(f["status"], "probable");
    assert!(
        f["verdict"]["pValue"].as_f64().is_some_and(|p| p < 0.01),
        "{f}"
    );
    assert!(
        f["verdict"]["change"].as_f64().is_some_and(|c| c > 0.05),
        "{f}"
    );
    // The test's duration, from the JUnit, did not change: no regression there.
    assert!(
        !found
            .iter()
            .any(|r| r["key"].as_str().is_some_and(|k| k.starts_with("test:"))),
        "{report}"
    );
    assert_eq!(
        report["result"]["reference"].as_array().map(Vec::len),
        Some(1)
    );

    let second = project.tim_with(
        &["test", "--runs", "5", "--", "sh", "run-tests.sh"],
        true,
        &slow,
    );
    let said = stderr(&second);
    assert!(
        said.contains("timonier: regression confirmed on f:"),
        "{said}"
    );
    let report = project.tool("get_regressions", serde_json::json!({}));
    let f = report["result"]["regressions"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["key"] == "node:rust:demo::f").cloned())
        .unwrap_or_else(|| panic!("{report}"));
    assert_eq!(f["status"], "confirmed");
}

/// Q77: runs on a modified working tree stand for no commit: they are no reference.
#[test]
fn runs_on_a_modified_tree_are_no_reference() {
    let project = Project::new();
    project.instrumented_tests();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(project.root())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    std::fs::write(project.root().join(".gitignore"), "reports/\n").expect("gitignore");
    git(&["add", "-A"]);
    git(&["commit", "-q", "--no-gpg-sign", "-m", "first"]);
    // Work not committed: these runs measure no commit.
    std::fs::write(project.root().join("notes.txt"), "work in progress\n").expect("notes");
    let modified = project.tim(&["test", "--runs", "2", "--", "sh", "run-tests.sh"], true);
    assert_eq!(modified.status.code(), Some(0), "{modified:?}");
    std::fs::remove_file(project.root().join("notes.txt")).expect("notes");
    git(&[
        "commit",
        "-q",
        "--no-gpg-sign",
        "--allow-empty",
        "-m",
        "second",
    ]);
    let clean = project.tim(&["test", "--runs", "2", "--", "sh", "run-tests.sh"], true);
    assert!(
        stderr(&clean).contains("no reference yet"),
        "{}",
        stderr(&clean)
    );
    let runs = project.tool("get_regressions", serde_json::json!({}));
    assert!(
        runs["result"]["reference"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "{runs}"
    );
}

/// Q77 (relecture n° 2, m-2): on a modified tree, a regression is confirmed by a batch of the
/// same changes only.
#[test]
fn a_regression_on_a_modified_tree_is_confirmed_by_the_same_changes_only() {
    let project = Project::new();
    project.instrumented_tests();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(project.root())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    std::fs::write(project.root().join(".gitignore"), "reports/\n").expect("gitignore");
    git(&["add", "-A"]);
    git(&["commit", "-q", "--no-gpg-sign", "-m", "the reference"]);
    let reference = project.tim(&["test", "--runs", "5", "--", "sh", "run-tests.sh"], true);
    assert_eq!(reference.status.code(), Some(0), "{reference:?}");
    git(&[
        "commit",
        "-q",
        "--no-gpg-sign",
        "--allow-empty",
        "-m",
        "next",
    ]);
    let slow = [("TIMONIER_FIXTURE_SLOW_MS", "40")];
    let batch = |notes: &str| {
        std::fs::write(project.root().join("notes.txt"), notes).expect("notes");
        let output = project.tim_with(
            &["test", "--runs", "5", "--", "sh", "run-tests.sh"],
            true,
            &slow,
        );
        stderr(&output)
    };
    let first = batch("one\n");
    assert!(
        first.contains("timonier: regression probable on f:"),
        "{first}"
    );
    let other = batch("two\n");
    assert!(
        other.contains("timonier: regression probable on f:"),
        "other changes confirm nothing: {other}"
    );
    let same = batch("two\n");
    assert!(
        same.contains("timonier: regression confirmed on f:"),
        "{same}"
    );
}

/// `timonier test --install` (the Performances panel's « Run the tests »): a runner's program
/// missing from the PATH is installed first, with the command its pack gives
/// (`cargo install cargo-nextest --locked`), here through a fake `cargo`.
#[test]
fn a_missing_runner_program_is_installed_first() {
    let project = Project::new();
    std::fs::write(
        project.root().join(".timonier/tests.json"),
        r#"{"version": 1, "runners": [{"language": "rust", "runner": "cargo-nextest",
            "command": "sh run-tests.sh", "junit": "reports/junit.xml"}]}"#,
    )
    .unwrap();
    let tools = tempfile::tempdir().unwrap();
    let cargo = tools.path().join("cargo");
    std::fs::write(
        &cargo,
        format!(
            "#!/bin/sh\necho \"$@\" > \"{0}/asked\"\nprintf '#!/bin/sh\\n' > \"{0}/cargo-nextest\"\nchmod +x \"{0}/cargo-nextest\"\n",
            tools.path().display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", tools.path().display());
    let output = project.tim_with(
        &["test", "--approve", "--install"],
        true,
        &[("PATH", path.as_str())],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installing cargo-nextest: cargo install cargo-nextest --locked"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(tools.path().join("asked")).unwrap(),
        "install cargo-nextest --locked\n"
    );
    assert!(tools.path().join("cargo-nextest").is_file());
    // The tests then ran, and the run was kept.
    assert!(stderr.contains("3 tests, 1 failed"), "{stderr}");
}
