//! `timonier test -- <command>`: runs the tests under the measuring relay (level 1, ADR 0045),
//! reads the duration of each test in the runner's JUnit XML, and hands the run to the daemon,
//! which keeps it (ADR 0043). Without a command, the runners of `.timonier/tests.json`.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Args;
use timonier_core::perf::{
    Accounting, Machine, Measured, RegressionReport, RunFinish, RunFinished, RunRecord, RunStart,
    RunStarted, TestCase, TestStatus, merged_resource_attributes,
};
use timonier_core::protocol::{StatusResult, methods};
use timonier_daemon::runtime::Paths;

use crate::daemon;
use crate::init::plan::{TESTS, Tests};

#[derive(Debug, Args)]
pub struct TestArgs {
    /// The JUnit XML the command writes, when `.timonier/tests.json` does not say it
    #[arg(long)]
    junit: Option<PathBuf>,
    /// Turn the instrumentation off (`OTEL_SDK_DISABLED=true`): a run to measure the overhead
    /// of the traces against
    #[arg(long)]
    without_traces: bool,
    /// How many times to run each command, one batch (ADR 0053): a number, or `auto` for 5, up
    /// to 15 when the runs are spread; what the regressions are judged on
    #[arg(long, default_value = "1", value_parser = parse_runs)]
    runs: Runs,
    /// Approve the commands of `.timonier/tests.json` for this machine, after reading them
    /// (Q84): a command this machine has not approved does not run
    #[arg(long)]
    approve: bool,
    /// Install first a program a runner needs and the machine lacks, as its pack says
    /// (`cargo install cargo-nextest --locked`)
    #[arg(long)]
    install: bool,
    #[command(flatten)]
    target: daemon::Target,
    /// The command that runs the tests; without one, those of `.timonier/tests.json`
    #[arg(last = true)]
    command: Vec<String>,
}

/// How many runs a batch makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runs {
    Exactly(usize),
    /// 5, up to 15 while the runs are spread (spec « Performance regression threshold »).
    Auto,
}

fn parse_runs(text: &str) -> std::result::Result<Runs, String> {
    if text == "auto" {
        return Ok(Runs::Auto);
    }
    match text.parse::<usize>() {
        Ok(n) if (1..=100).contains(&n) => Ok(Runs::Exactly(n)),
        _ => Err("a number from 1 to 100, or `auto`".to_owned()),
    }
}

/// How `timonier test` names itself to the daemon.
const CLIENT: &str = "timonier-test";

/// How long a run waits for a daemon it started to index the project.
const INDEXING: Duration = Duration::from_secs(30);

/// A command line run by the system's shell: `sh -c`, or `cmd /C` on Windows (T-125), where
/// a wrapper of the project (`./gradlew`, `./mvnw`) is written `.\gradlew` (ADR 0064).
fn shell(command: String) -> Vec<String> {
    if cfg!(windows) {
        let command = match command.strip_prefix("./") {
            Some(rest) => format!(".\\{rest}"),
            None => command,
        };
        vec!["cmd".to_owned(), "/C".to_owned(), command]
    } else {
        vec!["sh".to_owned(), "-c".to_owned(), command]
    }
}

/// Forces the measure without a systemd scope (`rusage`), for tests and odd machines.
const MEASURE: &str = "TIMONIER_MEASURE";

/// What the run sets for the OpenTelemetry SDK of the tests (spec « Wiring »).
const ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const RESOURCE_ATTRIBUTES: &str = "OTEL_RESOURCE_ATTRIBUTES";
const HEADERS: &str = "OTEL_EXPORTER_OTLP_HEADERS";
const SDK_DISABLED: &str = "OTEL_SDK_DISABLED";

/// One run to make: the command and where its JUnit XML lands.
struct Planned {
    command: Vec<String>,
    junit: Option<PathBuf>,
}

pub fn run(args: &TestArgs) -> ExitCode {
    match plan(args) {
        Ok((paths, planned)) => {
            let mut worst = 0;
            for planned in planned {
                worst = worst.max(batch(&paths, &planned, !args.without_traces, args.runs));
            }
            ExitCode::from(worst)
        }
        Err(error) => {
            eprintln!("timonier: {error:#}");
            // Commands not approved: 2, apart from the tests' own failures (Q84).
            if error.is::<NotApproved>() {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

/// Commands of `.timonier/tests.json` this machine has not approved (Q84).
#[derive(Debug)]
struct NotApproved(String);

impl std::fmt::Display for NotApproved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotApproved {}

fn plan(args: &TestArgs) -> Result<(Paths, Vec<Planned>)> {
    let paths = daemon::resolve(&args.target)?;
    let runners = std::fs::read_to_string(paths.root.join(TESTS))
        .ok()
        .and_then(|text| serde_json::from_str::<Tests>(&text).ok())
        .map(|tests| tests.runners)
        .unwrap_or_default();
    let junit = |declared: Option<&String>| {
        args.junit
            .clone()
            .or_else(|| declared.map(|j| paths.root.join(j)))
    };
    if !args.command.is_empty() {
        // The JUnit of the runner whose command this is, if one is.
        let joined = args.command.join(" ");
        let declared = runners
            .iter()
            .find(|r| r.command.as_deref() == Some(joined.as_str()))
            .and_then(|r| r.junit.as_ref());
        let planned = Planned {
            command: args.command.clone(),
            junit: junit(declared),
        };
        return Ok((paths, vec![planned]));
    }
    let planned: Vec<Planned> = runners
        .iter()
        .filter_map(|r| {
            Some(Planned {
                command: shell(r.command.clone()?),
                junit: junit(r.junit.as_ref()),
            })
        })
        .collect();
    anyhow::ensure!(
        !planned.is_empty(),
        "no command: give one after `--`, or run `timonier init` to record the test runners in \
         {TESTS}"
    );
    // The file is the repository's: what this machine has not approved does not run (Q84).
    let commands: Vec<&str> = runners
        .iter()
        .filter_map(|r| r.command.as_deref())
        .collect();
    if args.approve {
        crate::approval::approve(&paths.root, commands.iter().copied())?;
    } else {
        let unknown: Vec<&str> = commands
            .iter()
            .copied()
            .filter(|c| !crate::approval::approved(&paths.root, c))
            .collect();
        if !unknown.is_empty() {
            return Err(NotApproved(crate::approval::refusal(
                &unknown,
                "timonier test --approve",
            ))
            .into());
        }
    }
    if args.install {
        install_missing(&runners)?;
    }
    Ok((paths, planned))
}

/// Installs the programs the runners need and the `PATH` lacks, with the commands their packs
/// give; says what it runs. A missing program no pack knows how to install is said.
fn install_missing(runners: &[crate::init::plan::TestRunner]) -> anyhow::Result<()> {
    let packs = timonier_daemon::Packs::builtin()?;
    let environment = crate::init::detect::Environment::from_process();
    for runner in runners {
        let rule = packs
            .iter()
            .filter(|p| p.language.as_str() == runner.language)
            .flat_map(|p| p.manifest.tests.runners.iter())
            .find(|r| r.name == runner.runner);
        let Some(program) = rule.and_then(|r| r.program.as_deref()) else {
            continue;
        };
        if environment.on_path(program) {
            continue;
        }
        let Some(install) = rule.and_then(|r| r.install.as_deref()) else {
            anyhow::bail!("{program} is missing, and no pack says how to install it");
        };
        eprintln!("timonier: installing {program}: {install}");
        let mut words = shell(install.to_owned());
        let status = std::process::Command::new(words.remove(0))
            .args(&words)
            .status()
            .with_context(|| format!("cannot run `{install}`"))?;
        anyhow::ensure!(status.success(), "`{install}` failed ({status})");
    }
    Ok(())
}

/// Runs one command `runs` times, one batch, then says the regressions the daemon finds in it
/// (ADR 0053); returns the worst exit status. A failing run ends the batch: its times would not
/// compare.
fn batch(paths: &Paths, planned: &Planned, traced: bool, runs: Runs) -> u8 {
    let name = format!("B-{}-{}", millis(SystemTime::now()), std::process::id());
    let mut costs = Vec::new();
    let mut worst = 0;
    loop {
        let outcome = run_once(paths, planned, traced, Some(&name), false);
        worst = worst.max(outcome.code);
        if let Some(cost) = outcome.cost {
            costs.push(cost);
        }
        let more = match runs {
            Runs::Exactly(n) => costs.len() < n,
            Runs::Auto => timonier_core::regression::needs_more(&costs),
        };
        if outcome.code != 0 || outcome.cost.is_none() || !more {
            break;
        }
    }
    if costs.len() > 1 {
        say_regressions(paths, &name);
    }
    if traced {
        say_budgets_and_gaps(paths);
    }
    worst
}

/// What the last runs say of the whiteboard's budgets and of the instrumentation (relecture
/// n° 1, m10): a line per exceeded budget, and the gaps in one line, as the agent reads them.
fn say_budgets_and_gaps(paths: &Paths) {
    let Some(mut client) = daemon::connect_as(paths, CLIENT) else {
        return;
    };
    let budgets: Result<Option<timonier_core::budgets::BudgetsView>, _> =
        client.request(methods::PERF_BUDGETS, &serde_json::Value::Null);
    for check in budgets.ok().flatten().map(|b| b.checks).unwrap_or_default() {
        if check.state != timonier_core::budgets::BudgetState::Exceeded {
            continue;
        }
        let over: Vec<String> = check
            .limits
            .iter()
            .filter(|l| l.exceeded)
            .map(|l| {
                let kind = serde_json::to_value(l.kind).unwrap_or_default();
                format!(
                    "{} {} > {}",
                    kind.as_str().unwrap_or("limit"),
                    l.measured.unwrap_or_default(),
                    l.limit
                )
            })
            .collect();
        eprintln!(
            "timonier: budget {} exceeded on {}: {}",
            check.id,
            check.on,
            over.join(", ")
        );
    }
    let gaps: Result<serde_json::Value, _> = client.request(
        methods::TOOLS_CALL,
        &serde_json::json!({ "name": "get_instrumentation_gaps", "arguments": {} }),
    );
    // The tool's one-line summary, after its mailbox header.
    if let Some(line) = gaps
        .ok()
        .and_then(|g| g["content"][0]["text"].as_str().map(str::to_owned))
        .and_then(|text| text.lines().last().map(str::to_owned))
    {
        eprintln!("timonier: instrumentation: {line}");
    }
}

/// The regressions the daemon finds in a batch, one line each.
fn say_regressions(paths: &Paths, batch: &str) {
    let Some(mut client) = daemon::connect_as(paths, CLIENT) else {
        return;
    };
    let report: Result<RegressionReport, _> = client.request(
        methods::PERF_REGRESSIONS,
        &serde_json::json!({ "batch": batch }),
    );
    let Ok(report) = report else { return };
    if let Some(note) = &report.note {
        eprintln!("timonier: regressions: {note}");
    }
    for found in &report.regressions {
        eprintln!(
            "timonier: regression {} on {}: {:+.1} % (p = {:.4}){}",
            match found.status {
                timonier_core::perf::RegressionStatus::Probable => "probable",
                timonier_core::perf::RegressionStatus::Confirmed => "confirmed",
            },
            found.label,
            found.verdict.change * 100.0,
            found.verdict.p_value,
            if found.status == timonier_core::perf::RegressionStatus::Probable {
                ": run the tests again to confirm it"
            } else {
                ""
            },
        );
    }
    if report.regressions.is_empty() && report.note.is_none() {
        eprintln!(
            "timonier: no regression against the main branch ({} measures compared)",
            report.checked
        );
    }
}

/// What one run gave: the command's exit status, the run as the daemon numbered it, and what
/// its CPU work cost (instructions, else CPU microseconds).
pub struct Outcome {
    pub code: u8,
    pub run: Option<String>,
    pub cost: Option<f64>,
}

/// Timonier's smoke test (ADR 0050): `command`, run by the shell under a run with traces.
pub fn smoke(paths: &Paths, command: &str) -> Outcome {
    let planned = Planned {
        command: shell(command.to_owned()),
        junit: None,
    };
    run_once(paths, &planned, true, None, true)
}

#[tracing::instrument(skip_all)]
fn run_once(
    paths: &Paths,
    planned: &Planned,
    traced: bool,
    batch: Option<&str>,
    smoke: bool,
) -> Outcome {
    // Some runners write their JUnit XML only into a folder that exists (`node --test`, T-120;
    // `swift test` for its folder of reports, ADR 0067); folders under every module are the
    // runner's to make.
    let folder = planned.junit.as_deref().and_then(|p| match reports(p) {
        None => p.parent().map(Path::to_path_buf),
        Some((base, None)) => Some(base),
        Some((_, Some(_))) => None,
    });
    if let Some(folder) = folder {
        let _ = std::fs::create_dir_all(folder);
    }
    let started = SystemTime::now();
    let begun = begin(
        paths,
        &RunStart {
            command: planned.command.clone(),
            started_at_ms: millis(started),
            traced,
            batch: batch.map(str::to_owned),
            smoke,
        },
    );
    if let Err(error) = &begun {
        eprintln!("timonier: the run will not be recorded nor traced: {error:#}");
    }
    let out = report_path(paths);
    let scope = systemd_scope();
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("timonier"));
    let mut relay = if scope {
        let mut c = Command::new("systemd-run");
        c.args(["--user", "--scope", "--quiet", "--collect", "--"])
            .arg(&exe)
            .args(["__measure", "--scope"]);
        c
    } else {
        let mut c = Command::new(&exe);
        c.arg("__measure");
        c
    };
    relay
        .arg("--out")
        .arg(&out)
        .arg("--")
        .args(&planned.command);
    if let Ok(begun) = &begun {
        // Where the traces go, and which run and snapshot they belong to (ADR 0047).
        let existing = std::env::var(RESOURCE_ATTRIBUTES).ok();
        if let Some(endpoint) = &begun.endpoint {
            relay.env(ENDPOINT, endpoint);
        }
        // The run's token, which the receiver asks of its senders (audit, finding 2): after
        // the headers already set, which keep theirs.
        if let Some(token) = &begun.token {
            let existing = std::env::var(HEADERS).ok().filter(|h| !h.trim().is_empty());
            let ours = format!("{}={token}", timonier_daemon::OTLP_TOKEN_HEADER);
            relay.env(
                HEADERS,
                existing.map_or_else(|| ours.clone(), |h| format!("{h},{ours}")),
            );
        }
        if !traced {
            // OpenTelemetry's own switch: the SDK makes no span and sends nothing (ADR 0049).
            relay.env(SDK_DISABLED, "true");
        }
        relay.env(
            RESOURCE_ATTRIBUTES,
            merged_resource_attributes(existing.as_deref(), &begun.resource_attributes()),
        );
    }
    let status = match relay.status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("timonier: cannot start the measure: {error}");
            return Outcome {
                code: 1,
                run: None,
                cost: None,
            };
        }
    };
    let code = status
        .code()
        .and_then(|c| u8::try_from(c & 0xff).ok())
        .unwrap_or(1);
    let measured = std::fs::read(&out)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Measured>(&bytes).ok());
    let _ = std::fs::remove_file(&out);
    let Some(mut measured) = measured else {
        eprintln!("timonier: the run was not measured (the relay wrote nothing)");
        return Outcome {
            code,
            run: None,
            cost: None,
        };
    };
    // One origin for the spans and the samples: the run's start.
    measured.rebase_samples(millis(started));
    let (tests, junit_problem) = match &planned.junit {
        Some(path) => match read_junit(path, started) {
            Ok(tests) => (tests, None),
            Err(problem) => (Vec::new(), Some(problem)),
        },
        None => (Vec::new(), None),
    };
    let cost = measured.cost();
    let record = RunRecord {
        command: planned.command.clone(),
        started_at_ms: millis(started),
        measured,
        junit: planned.junit.as_ref().map(|p| relative(&paths.root, p)),
        junit_problem,
        tests,
        machine: machine(),
    };
    let summary = summary(&record);
    let Ok(begun) = begun else {
        eprintln!("timonier: {summary}");
        return Outcome {
            code,
            run: None,
            cost: Some(cost),
        };
    };
    let finish = RunFinish {
        run: begun.run.clone(),
        record,
    };
    let recorded = match end(paths, &finish) {
        Ok(_) => {
            eprintln!("timonier: {} · {summary}", begun.run);
            true
        }
        Err(error) => {
            eprintln!("timonier: {summary}");
            eprintln!("timonier: the run is not recorded: {error:#}");
            false
        }
    };
    Outcome {
        code,
        run: recorded.then_some(begun.run),
        cost: Some(cost),
    }
}

/// The run begins at the daemon: its number, its snapshot, where its traces go.
fn begin(paths: &Paths, start: &RunStart) -> Result<RunStarted> {
    daemon::launch(paths)?;
    let mut client = daemon::connect_as(paths, CLIENT).context("the daemon does not answer")?;
    // The run's snapshot, and the nodes its spans attach to, are those of the indexed code.
    let deadline = Instant::now() + INDEXING;
    loop {
        let status: StatusResult = client.request(methods::STATUS, &serde_json::Value::Null)?;
        if status.indexed || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(client.request(methods::RUN_START, start)?)
}

fn end(paths: &Paths, finish: &RunFinish) -> Result<RunFinished> {
    let mut client = daemon::connect_as(paths, CLIENT).context("the daemon does not answer")?;
    Ok(client.request(methods::RUN_FINISH, finish)?)
}

/// A file of the runtime directory, closed to others, for the relay's report.
fn report_path(paths: &Paths) -> PathBuf {
    let _ = timonier_daemon::runtime::prepare_runtime(paths);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    paths
        .runtime
        .join(format!("measure-{}-{nanos}.json", std::process::id()))
}

/// Whether a transient systemd user scope can hold the run (ADR 0045): `systemd-run` is there,
/// and the user's manager delegates cpu and memory.
fn systemd_scope() -> bool {
    if !cfg!(target_os = "linux") || std::env::var(MEASURE).is_ok_and(|v| v == "rusage") {
        return false;
    }
    let on_path = std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| dir.join("systemd-run").is_file())
    });
    let Some(manager) = user_manager() else {
        return false;
    };
    let controllers =
        std::fs::read_to_string(manager.join("cgroup.controllers")).unwrap_or_default();
    let words: Vec<&str> = controllers.split_whitespace().collect();
    on_path && words.contains(&"cpu") && words.contains(&"memory")
}

/// The cgroup of the user's systemd manager (`…/user@<uid>.service`) this process runs under.
fn user_manager() -> Option<PathBuf> {
    let text = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = text.lines().find_map(|line| line.strip_prefix("0::"))?;
    let mut dir = PathBuf::from("/sys/fs/cgroup");
    for part in path.split('/').filter(|p| !p.is_empty()) {
        dir.push(part);
        if part.starts_with("user@") && part.ends_with(".service") {
            return Some(dir);
        }
    }
    None
}

/// The test cases of a JUnit XML that this run wrote: one older than the run is someone
/// else's report.
fn read_junit(path: &Path, started: SystemTime) -> Result<Vec<TestCase>, String> {
    if let Some((base, under)) = reports(path) {
        return read_reports(path, &base, under.as_deref(), started);
    }
    let shown = path.display();
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|_| format!("no JUnit XML at {shown}"))?;
    if modified < started {
        return Err(format!(
            "{shown} is older than the run: the runner did not write it"
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    let suites = junit_parser::from_reader(BufReader::new(within_a_suite(&text).as_bytes()))
        .map_err(|e| format!("{shown}: {e}"))?;
    let mut cases = Vec::new();
    for suite in &suites.suites {
        collect(suite, &mut cases);
    }
    Ok(cases)
}

/// A folder of reports, one XML file per class (Surefire's, Gradle's; ADR 0064): its path ends
/// with `/`, and `**/` before it finds it in every module. The base to search from, and the
/// folder to find under each module (`None`: the base itself).
fn reports(path: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let text = path.to_string_lossy();
    if let Some((base, under)) = text.split_once("**") {
        let under = under.trim_matches(['/', '\\']);
        return Some((PathBuf::from(base), Some(PathBuf::from(under))));
    }
    (text.ends_with('/') || text.ends_with('\\')).then(|| (path.to_path_buf(), None))
}

/// The XML reports this run wrote in a folder of reports, or in that folder of every module
/// under `base` (a few levels deep; hidden folders and installed dependencies aside).
fn read_reports(
    shown: &Path,
    base: &Path,
    under: Option<&Path>,
    started: SystemTime,
) -> Result<Vec<TestCase>, String> {
    let mut folders = Vec::new();
    match under {
        None => folders.push(base.to_path_buf()),
        Some(under) => {
            let mut pending = vec![(base.to_path_buf(), 0)];
            while let Some((dir, depth)) = pending.pop() {
                let candidate = dir.join(under);
                if candidate.is_dir() {
                    folders.push(candidate);
                }
                // sbt 2 writes into `target/out/jvm/scala-<version>/<project>/test-reports`.
                if depth >= 7 {
                    continue;
                }
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    let skipped = name.starts_with('.') || name == "node_modules";
                    if !skipped && entry.file_type().is_ok_and(|t| t.is_dir()) {
                        pending.push((entry.path(), depth + 1));
                    }
                }
            }
        }
    }
    let mut cases = Vec::new();
    let mut read = 0;
    folders.sort();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "xml"))
            .filter(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| m >= started)
            })
            .collect();
        files.sort();
        for file in files {
            cases.extend(read_junit(&file, started)?);
            read += 1;
        }
    }
    if read == 0 {
        return Err(format!(
            "no JUnit XML written in {}: the runner wrote none",
            shown.display()
        ));
    }
    Ok(cases)
}

/// `node --test` writes its test cases straight under `<testsuites>`, with no `<testsuite>`
/// around them, which the parser does not read: they get one (T-120).
fn within_a_suite(text: &str) -> std::borrow::Cow<'_, str> {
    let bare = text.contains("<testsuites")
        && !text.contains("<testsuite ")
        && !text.contains("<testsuite>")
        && text.contains("<testcase");
    let (Some(open), Some(close)) = (text.find("<testsuites"), text.rfind("</testsuites>")) else {
        return text.into();
    };
    let Some(end) = text[open..].find('>').map(|i| open + i + 1) else {
        return text.into();
    };
    if !bare || end > close {
        return text.into();
    }
    format!(
        "{}<testsuite name=\"tests\">{}</testsuite>{}",
        &text[..end],
        &text[end..close],
        &text[close..]
    )
    .into()
}

fn collect(suite: &junit_parser::TestSuite, cases: &mut Vec<TestCase>) {
    for case in &suite.cases {
        let status = match case.status {
            junit_parser::TestStatus::Success => TestStatus::Passed,
            junit_parser::TestStatus::Failure(_) => TestStatus::Failed,
            junit_parser::TestStatus::Error(_) => TestStatus::Error,
            junit_parser::TestStatus::Skipped(_) => TestStatus::Skipped,
        };
        cases.push(TestCase {
            name: case.original_name.clone(),
            classname: case.classname.clone(),
            file: case.file.clone(),
            line: case.line,
            status,
            duration_us: seconds_to_us(case.time),
            started_at_us: case.timestamp.as_deref().and_then(rfc3339_us),
        });
    }
    for inner in &suite.suites {
        collect(inner, cases);
    }
}

/// An RFC 3339 time (`2024-01-09T07:50:12.665+00:00`, as nextest writes a test's
/// `timestamp`), in microseconds since the Unix epoch; `None` when it does not read.
fn rfc3339_us(text: &str) -> Option<u64> {
    let (date, rest) = text.trim().split_once(['T', 't', ' '])?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let (time, offset_s) = match rest.find(['Z', 'z', '+', '-']) {
        Some(at) => {
            let (time, zone) = rest.split_at(at);
            let offset = if zone.eq_ignore_ascii_case("z") {
                0
            } else {
                let sign = if zone.starts_with('-') { -1 } else { 1 };
                let (hours, minutes) = zone[1..].split_once(':').unwrap_or((&zone[1..], "0"));
                sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60)
            };
            (time, offset)
        }
        // No zone: UTC, as the JUnit of some runners writes it.
        None => (rest, 0),
    };
    let mut parts = time.splitn(3, ':');
    let (hour, minute) = (
        parts.next()?.parse::<i64>().ok()?,
        parts.next()?.parse::<i64>().ok()?,
    );
    let (second, fraction) = parts
        .next()?
        .split_once('.')
        .unwrap_or((time.rsplit(':').next()?, ""));
    let second: i64 = second.parse().ok()?;
    let micros: i64 = format!("{fraction:0<6}").get(..6)?.parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // Days since the epoch of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_s;
    u64::try_from(seconds * 1_000_000 + micros).ok()
}

fn seconds_to_us(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        Duration::try_from_secs_f64(seconds)
            .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
    } else {
        0
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn millis(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// What the machine is, for its fingerprint (ADR 0043).
fn machine() -> Machine {
    let output = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let (cpu, memory_bytes) = if cfg!(target_os = "macos") {
        (
            output("sysctl", &["-n", "machdep.cpu.brand_string"]),
            output("sysctl", &["-n", "hw.memsize"]).parse().unwrap_or(0),
        )
    } else {
        let field = |file: &str, key: &str| {
            std::fs::read_to_string(file).ok().and_then(|text| {
                text.lines()
                    .find(|l| l.starts_with(key))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().to_owned())
            })
        };
        let kib: u64 = field("/proc/meminfo", "MemTotal")
            .and_then(|v| v.trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0);
        (
            field("/proc/cpuinfo", "model name").unwrap_or_default(),
            kib.saturating_mul(1024),
        )
    };
    Machine {
        cpu,
        cores: std::thread::available_parallelism()
            .map_or(1, |n| u32::try_from(n.get()).unwrap_or(u32::MAX)),
        memory_bytes,
        os: std::env::consts::OS.to_owned(),
        kernel: output("uname", &["-r"]),
    }
}

/// One line for the developer: time, CPU, memory, tests.
fn summary(record: &RunRecord) -> String {
    let m = &record.measured;
    let seconds = |us: u64| format!("{:.2} s", us as f64 / 1e6);
    let mut parts = vec![seconds(m.wall_us)];
    if m.accounting == Accounting::WallTime {
        parts.push("CPU and memory not measured on this system".to_owned());
    } else {
        match m.instructions {
            Some(count) => parts.push(format!(
                "{:.2} G instructions, CPU {}",
                count as f64 / 1e9,
                seconds(m.user_cpu_us + m.system_cpu_us)
            )),
            None => parts.push(format!("CPU {}", seconds(m.user_cpu_us + m.system_cpu_us))),
        }
        let (bytes, whose) = match (m.accounting, m.tree_peak_bytes) {
            (Accounting::SystemdScope, Some(peak)) => (peak, "all processes"),
            _ => (m.largest_process_bytes, "largest process"),
        };
        parts.push(format!("memory {:.0} MB ({whose})", bytes as f64 / 1e6));
    }
    if !record.tests.is_empty() {
        let failed = record
            .tests
            .iter()
            .filter(|t| matches!(t.status, TestStatus::Failed | TestStatus::Error))
            .count();
        parts.push(format!("{} tests, {failed} failed", record.tests.len()));
    }
    let mut line = parts.join(" · ");
    if let Some(problem) = &record.junit_problem {
        line.push_str(&format!("\n  no test durations: {problem}"));
    }
    if let Some(why) = &m.counter_refused {
        line.push_str(&format!("\n  CPU counted in time: {why}"));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="1" errors="0">
  <testsuite name="timonier-core" tests="3" failures="1">
    <testcase name="parses" classname="timonier-core::parser" time="0.012"/>
    <testcase name="fails" classname="timonier-core::parser" time="1.5">
      <failure message="boom"/>
    </testcase>
    <testcase name="later" classname="timonier-core::parser" time="0"><skipped/></testcase>
  </testsuite>
</testsuites>
"#;

    /// T-120: `node --test` writes its cases with no `<testsuite>`; they are read all the same.
    #[test]
    fn node_test_s_junit_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junit.xml");
        let started = SystemTime::now() - std::time::Duration::from_secs(5);
        std::fs::write(
            &path,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<testsuites>\n\t<testcase name=\"adds\" time=\"0.0005\" classname=\"test\" file=\"/p/a.test.js\"/>\n\t<testcase name=\"fails\" time=\"0.001\" classname=\"test\"><failure message=\"no\"/></testcase>\n\t<!-- tests 2 -->\n</testsuites>\n",
        )
        .unwrap();
        let cases = read_junit(&path, started).unwrap();
        let read: Vec<(&str, TestStatus)> =
            cases.iter().map(|c| (c.name.as_str(), c.status)).collect();
        assert_eq!(
            read,
            [("adds", TestStatus::Passed), ("fails", TestStatus::Failed)]
        );
        // A file that has its suites is left as it is.
        let suited =
            "<testsuites><testsuite name=\"s\"><testcase name=\"x\"/></testsuite></testsuites>";
        assert_eq!(within_a_suite(suited), suited);
    }

    /// T-123: a folder of reports per module, as Gradle and Surefire write them: every report
    /// this run wrote, in every module, and none of an earlier run.
    #[test]
    fn a_folder_of_reports_is_read_in_every_module() {
        let dir = tempfile::tempdir().unwrap();
        let class = |name: &str| {
            format!(
                "<?xml version=\"1.0\"?>\n<testsuite name=\"com.shop.{name}\" tests=\"1\"><testcase name=\"works\" classname=\"com.shop.{name}\" time=\"0.01\"/></testsuite>\n"
            )
        };
        let before = SystemTime::now() - Duration::from_secs(1);
        for (module, name) in [("app", "AppTest"), ("libs/core", "CoreTest")] {
            let folder = dir.path().join(module).join("build/test-results/test");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join(format!("TEST-com.shop.{name}.xml")),
                class(name),
            )
            .unwrap();
        }
        let pattern = dir.path().join("**/build/test-results/test/");
        let tests = read_junit(&pattern, before).unwrap();
        let classes: Vec<_> = tests
            .iter()
            .filter_map(|t| t.classname.as_deref())
            .collect();
        assert_eq!(classes, ["com.shop.AppTest", "com.shop.CoreTest"]);
        let later = SystemTime::now() + Duration::from_secs(60);
        assert!(
            read_junit(&pattern, later)
                .unwrap_err()
                .contains("wrote none")
        );
        let one = dir.path().join("app/build/test-results/test/");
        assert_eq!(read_junit(&one, before).unwrap().len(), 1);
    }

    #[test]
    fn a_junit_report_gives_each_test_and_its_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junit.xml");
        let before = SystemTime::now() - Duration::from_secs(1);
        std::fs::write(&path, REPORT).unwrap();
        let tests = read_junit(&path, before).unwrap();
        let shown: Vec<_> = tests
            .iter()
            .map(|t| (t.name.as_str(), t.status, t.duration_us))
            .collect();
        assert_eq!(
            shown,
            [
                ("parses", TestStatus::Passed, 12_000),
                ("fails", TestStatus::Failed, 1_500_000),
                ("later", TestStatus::Skipped, 0)
            ]
        );
        assert_eq!(tests[0].classname.as_deref(), Some("timonier-core::parser"));
    }

    #[test]
    fn a_bare_testsuite_reads_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junit.xml");
        std::fs::write(
            &path,
            r#"<testsuite name="s" tests="1"><testcase name="a" time="0.5" file="a.ts" line="3"/></testsuite>"#,
        )
        .unwrap();
        let tests = read_junit(&path, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(tests.len(), 1);
        assert_eq!(
            (tests[0].file.as_deref(), tests[0].line),
            (Some("a.ts"), Some(3))
        );
    }

    #[test]
    fn a_report_older_than_the_run_or_missing_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junit.xml");
        std::fs::write(&path, REPORT).unwrap();
        let later = SystemTime::now() + Duration::from_secs(60);
        assert!(
            read_junit(&path, later)
                .unwrap_err()
                .contains("older than the run")
        );
        assert!(
            read_junit(&dir.path().join("none.xml"), SystemTime::UNIX_EPOCH)
                .unwrap_err()
                .contains("no JUnit XML")
        );
    }

    #[test]
    fn a_test_s_timestamp_reads_in_any_zone() {
        // 2024-01-09T07:50:12.665Z: 1704786612 s since the epoch.
        let utc = 1_704_786_612_665_000;
        assert_eq!(rfc3339_us("2024-01-09T07:50:12.665+00:00"), Some(utc));
        assert_eq!(rfc3339_us("2024-01-09T07:50:12.665Z"), Some(utc));
        assert_eq!(rfc3339_us("2024-01-09T09:50:12.665+02:00"), Some(utc));
        assert_eq!(rfc3339_us("2024-01-09T07:50:12"), Some(utc - 665_000));
        assert_eq!(rfc3339_us("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            rfc3339_us("2024-02-29T00:00:00.5Z"),
            Some(1_709_164_800_500_000)
        );
        assert_eq!(rfc3339_us("yesterday"), None);
        assert_eq!(rfc3339_us("2024-13-01T00:00:00Z"), None);
    }

    #[test]
    fn durations_are_never_negative_nor_infinite() {
        assert_eq!(seconds_to_us(-1.0), 0);
        assert_eq!(seconds_to_us(f64::NAN), 0);
        assert_eq!(seconds_to_us(0.000_001), 1);
    }

    #[test]
    fn the_machine_is_described() {
        let machine = machine();
        assert!(machine.cores >= 1);
        assert_eq!(machine.os, std::env::consts::OS);
    }
}
