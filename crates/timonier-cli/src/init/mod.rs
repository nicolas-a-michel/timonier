//! `timonier init` (PLAN.md 6.8, ADR 0033): finds what the project and the machine hold, asks
//! what that does not settle (Q51), then writes the agents' instructions, their MCP server and
//! their hooks, the git hook, and opens the plan with a first session, through the daemon.

pub mod detect;
pub mod doctor;
pub mod edits;
pub mod ordered;
pub mod plan;
pub mod uninit;

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::{Value, json};
use timonier_core::protocol::methods;
use timonier_daemon::Paths;

use crate::daemon::{self, Target};
use detect::{Agent, Detection, Environment, Git};
use plan::{Choices, Planned};

/// How `timonier init` names itself to the daemon: the agent of the first session.
pub const AGENT: &str = "timonier-init";

#[derive(Debug, Args)]
pub struct InitArgs {
    #[command(flatten)]
    target: Target,
    /// Show the diff of every file it would touch, and change nothing
    #[arg(long)]
    dry_run: bool,
    /// Ask nothing: the agents found are the ones configured
    #[arg(long, short = 'y')]
    yes: bool,
    /// The agents to configure, in place of those found: claude, copilot, cursor, codex
    #[arg(long, value_delimiter = ',', value_name = "AGENTS")]
    agents: Option<Vec<String>>,
}

pub fn run(args: &InitArgs) -> ExitCode {
    match init(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn init(args: &InitArgs) -> Result<()> {
    let paths = daemon::resolve(&args.target)?;
    let root = paths.root.clone();
    let environment = Environment::from_process();
    let detection = detect::detect(&root, &environment);
    print_detection(&root, &detection);
    let agents = choose(args, &detection)?;
    let choices = Choices {
        agents,
        git: detection.git.clone(),
        runners: detection.runners.clone(),
        path: environment.path.clone(),
    };
    let planned = plan::plan(&root, &choices);
    let tracked = tracking_started(&root);
    if args.dry_run {
        print!("{}", plan::diff(&planned));
        if !tracked {
            println!("\nThrough the daemon (the only writer of the tracking files):");
            for step in TRACKING_STEPS {
                println!("  {step}");
            }
        }
        print_notes(&planned);
        if planned.changes.is_empty() && tracked {
            println!("\nNothing to change: Timonier is set up.");
        }
        return Ok(());
    }
    plan::apply(&root, &planned).context("cannot write the configuration")?;
    for change in &planned.changes {
        let verb = if change.before.is_some() {
            "changed"
        } else {
            "created"
        };
        println!("  {verb} {}", change.path.display());
    }
    if !tracked {
        start_tracking(&paths)?;
        println!("  opened the plan, with the milestone « Initialisation » and a first session");
    }
    print_notes(&planned);
    if planned.changes.is_empty() && tracked {
        println!("Nothing to change: Timonier is set up.");
    } else {
        println!("\nTimonier is set up.");
        print_approvals(&choices.agents);
    }
    Ok(())
}

fn print_detection(root: &Path, detection: &Detection) {
    let list = |items: &[&str]| {
        if items.is_empty() {
            "none found".to_owned()
        } else {
            items.join(", ")
        }
    };
    println!("Timonier init in {}", root.display());
    println!("  languages: {}", list(&detection.languages));
    println!("  package managers: {}", list(&detection.package_managers));
    let runners: Vec<String> = detection
        .runners
        .iter()
        .map(|r| format!("{} ({})", r.rule.name, r.language))
        .collect();
    let runners: Vec<&str> = runners.iter().map(String::as_str).collect();
    println!("  tests: {}", list(&runners));
    println!(
        "  git: {}",
        match &detection.git {
            Git::None => "no repository".to_owned(),
            Git::Repository {
                hooks_path: Some(manager),
                ..
            } => format!("repository, hooks managed by `{manager}`"),
            Git::Repository { .. } => "repository".to_owned(),
        }
    );
    if detection.agents.is_empty() {
        println!("  agents: none found");
    }
    for (agent, why) in &detection.agents {
        println!("  agent: {} ({why})", agent.label());
    }
}

/// The agents to configure: those named, else those found, confirmed at the terminal (Q51).
fn choose(args: &InitArgs, detection: &Detection) -> Result<Vec<Agent>> {
    if let Some(named) = &args.agents {
        return named
            .iter()
            .filter(|n| !n.is_empty())
            .map(|n| {
                Agent::parse(n)
                    .with_context(|| format!("no agent `{n}`: claude, copilot, cursor or codex"))
            })
            .collect();
    }
    let found: Vec<Agent> = detection.agents.iter().map(|(a, _)| *a).collect();
    if args.yes || args.dry_run || !std::io::stdin().is_terminal() {
        return Ok(found);
    }
    let question = if found.is_empty() {
        "No agent found. Which ones to configure (claude, copilot, cursor, codex; empty for none)? "
            .to_owned()
    } else {
        let names: Vec<&str> = found.iter().map(|a| a.label()).collect();
        format!(
            "Configure Timonier for {}? [Y/n, or the agents: claude, copilot, cursor, codex] ",
            names.join(", ")
        )
    };
    print!("{question}");
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    answered(&answer, &found)
}

/// The agents an answer at the terminal names: yes keeps those found, no none, or a list.
fn answered(answer: &str, found: &[Agent]) -> Result<Vec<Agent>> {
    let answer = answer.trim().to_lowercase();
    match answer.as_str() {
        "" | "y" | "yes" | "o" | "oui" if !found.is_empty() => Ok(found.to_vec()),
        "" | "n" | "no" | "non" => Ok(Vec::new()),
        _ => answer
            .split([',', ' '])
            .filter(|n| !n.is_empty())
            .map(|n| Agent::parse(n).with_context(|| format!("no agent `{n}`")))
            .collect(),
    }
}

fn print_notes(planned: &Planned) {
    if !planned.notes.is_empty() {
        println!("\nTo do by hand:");
        for note in &planned.notes {
            println!("  - {note}");
        }
    }
    if !planned.info.is_empty() {
        println!("\nGood to know:");
        for said in &planned.info {
            println!("  - {said}");
        }
    }
}

fn print_approvals(agents: &[Agent]) {
    for agent in agents {
        match agent {
            Agent::ClaudeCode => println!(
                "  Claude Code asks once to approve the project's MCP server `timonier` and its hooks."
            ),
            Agent::Codex => println!(
                "  Codex reads .codex/ once the project is trusted, and asks to trust its hooks (/hooks)."
            ),
            Agent::Copilot | Agent::Cursor => {}
        }
    }
}

/// Whether the project has a plan or a session already: then `init` leaves the tracking as it
/// is.
fn tracking_started(root: &Path) -> bool {
    let dir = root.join(".timonier");
    let plan_has_milestones = std::fs::read_to_string(dir.join("plan.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|plan| plan["milestones"].as_array().is_some_and(|m| !m.is_empty()));
    let sessions = std::fs::read_dir(dir.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("S-"));
    plan_has_milestones || sessions
}

const TRACKING_STEPS: [&str; 5] = [
    "session_start « Initialise Timonier »",
    "plan_add_task « Configure Timonier » in a new milestone « Initialisation »",
    "plan_update_task T-1: in progress, then done",
    "todo_add and todo_complete « Configure the agents, the MCP server and the hooks »",
    "session_end",
];

/// The first session, through the daemon's tools, as an agent would call them (ADR 0029).
fn start_tracking(paths: &Paths) -> Result<()> {
    daemon::launch(paths)?;
    let mut client = daemon::connect_as(paths, AGENT).context("the daemon does not answer")?;
    let mut tool = |name: &str, arguments: Value| -> Result<Value> {
        let result: Value = client
            .request(
                methods::TOOLS_CALL,
                &json!({ "name": name, "arguments": arguments }),
            )
            .with_context(|| format!("the daemon did not run `{name}`"))?;
        if result["isError"] == true {
            bail!(
                "`{name}` was refused: {}",
                result["structuredContent"]["error"]["message"]
            );
        }
        Ok(result["structuredContent"]["result"].clone())
    };
    tool(
        "session_start",
        json!({ "objective": "Initialise Timonier" }),
    )?;
    // The ids the tools give, never assumed (review n° 2 of Phase 6).
    let id = |value: &Value| -> Result<String> {
        value["id"]
            .as_str()
            .map(str::to_owned)
            .context("the daemon gave no id")
    };
    let task = id(&tool(
        "plan_add_task",
        json!({
            "title": "Configure Timonier",
            "criterion": "timonier init has configured the agents, the MCP server and the hooks",
            "new_milestone": "Initialisation",
        }),
    )?)?;
    tool(
        "plan_update_task",
        json!({ "task_id": task, "status": "in_progress" }),
    )?;
    let item = id(&tool(
        "todo_add",
        json!({ "text": "Configure the agents, the MCP server and the hooks", "task": task }),
    )?)?;
    tool("todo_complete", json!({ "item_id": item }))?;
    tool(
        "plan_update_task",
        json!({ "task_id": task, "status": "done" }),
    )?;
    tool(
        "session_end",
        json!({ "summary": "Timonier configured by timonier init: the agents' instructions, the MCP server, the hooks." }),
    )?;
    Ok(())
}

#[derive(Debug, Args)]
pub struct UninitArgs {
    #[command(flatten)]
    target: Target,
    /// Show the diff of every file it would touch, and change nothing
    #[arg(long)]
    dry_run: bool,
    /// Remove the project's tracking too: plan, todo, journal and whiteboard (asked first)
    #[arg(long)]
    purge: bool,
    /// With --purge: ask nothing
    #[arg(long, short = 'y')]
    yes: bool,
}

/// `timonier uninit` (PLAN.md 6.9): what `init` made, taken back.
pub fn run_uninit(args: &UninitArgs) -> ExitCode {
    let undo = || -> Result<()> {
        let paths = daemon::resolve(&args.target)?;
        let undone = uninit::plan(&paths.root, args.purge).map_err(anyhow::Error::msg)?;
        // What belongs to the project goes only once confirmed (Q54).
        if args.purge && !args.dry_run && !args.yes && !uninit::only_init_tracking(&paths.root) {
            if !std::io::stdin().is_terminal() {
                bail!(
                    "--purge removes the plan, the todo, the journal and the whiteboard: add --yes to confirm"
                );
            }
            print!(
                "This removes {} in .timonier/, for good. Type `purge` to confirm: ",
                uninit::TRACKING.join(", ")
            );
            std::io::stdout().flush().ok();
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer)?;
            if answer.trim() != "purge" {
                println!("Nothing removed.");
                return Ok(());
            }
        }
        if args.dry_run {
            print!("{}", plan::diff(&undone.planned));
            if undone.remove_folder {
                println!("\n.timonier/ goes whole: `timonier init` made it.");
            }
            for path in &undone.tracking_paths {
                println!("\n{} goes.", path.display());
            }
            print_notes(&undone.planned);
            return Ok(());
        }
        if undone.remove_folder {
            // The daemon keeps its database in .timonier/.
            daemon::stop(&paths)?;
        }
        uninit::apply(&paths.root, &undone).context("cannot take the configuration back")?;
        for change in &undone.planned.changes {
            let verb = if change.after.is_some() {
                "restored"
            } else {
                "removed"
            };
            println!("  {verb} {}", change.path.display());
        }
        for path in &undone.tracking_paths {
            println!("  removed {}", path.display());
        }
        if undone.remove_folder {
            println!("  removed .timonier/");
        }
        print_notes(&undone.planned);
        println!("\nTimonier is no longer set up here.");
        Ok(())
    };
    match undo() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(flatten)]
    target: Target,
    /// Repair what can be, without asking
    #[arg(long, conflicts_with = "check")]
    fix: bool,
    /// Repair nothing, ask nothing: the exit status says whether all is in order (for CI)
    #[arg(long)]
    check: bool,
    /// Skip the smoke test, which builds and runs the instrumented test of each language
    #[arg(long)]
    no_smoke: bool,
}

/// `timonier doctor` (PLAN.md 6.10): the setup checked, and repaired once asked.
#[tracing::instrument(skip_all)]
pub fn run_doctor(args: &DoctorArgs) -> ExitCode {
    let examine = || -> Result<bool> {
        let paths = daemon::resolve(&args.target)?;
        let root = paths.root.clone();
        let git = detect::detect(&root, &Environment::default()).git;
        let mut checked = doctor::check(&root, &Environment::from_process(), git);
        if root.join(plan::RECORD).is_file() {
            checked.findings.extend(daemon_findings(&paths));
        }
        // The chain, end to end (spec « Initialisation », 7): a trace attached to the graph.
        let mut smoked = Vec::new();
        if !args.no_smoke && root.join(plan::RECORD).is_file() {
            let (findings, said) = smoke(&paths);
            checked.findings.extend(findings);
            smoked = said;
        }
        let paranoid = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid").ok();
        let counters = doctor::counters_note(paranoid.as_deref());
        if checked.findings.is_empty() {
            println!("Timonier is set up and in order in {}.", root.display());
            for said in checked.repair.info.iter().chain(&smoked).chain(&counters) {
                println!("  - {said}");
            }
            return Ok(true);
        }
        println!("Timonier in {}:", root.display());
        for said in &smoked {
            println!("  - {said}");
        }
        if let Some(note) = &counters {
            println!("  - {note} (a note)");
        }
        for finding in &checked.findings {
            let mark = if finding.repairable {
                "repairable"
            } else {
                "to do by hand"
            };
            println!("  - {} ({mark})", finding.what);
        }
        let repairable = checked.findings.iter().filter(|f| f.repairable).count();
        if repairable == 0 || args.check {
            return Ok(false);
        }
        let repair = args.fix || {
            if std::io::stdin().is_terminal() {
                print!("Repair the {repairable} repairable finding(s)? [Y/n] ");
                std::io::stdout().flush().ok();
                let mut answer = String::new();
                std::io::stdin().lock().read_line(&mut answer)?;
                matches!(
                    answer.trim().to_lowercase().as_str(),
                    "" | "y" | "yes" | "o" | "oui"
                )
            } else {
                println!("Run `timonier doctor --fix` to repair them.");
                false
            }
        };
        if !repair {
            return Ok(false);
        }
        plan::apply(&root, &checked.repair).context("cannot repair the configuration")?;
        for change in &checked.repair.changes {
            println!("  repaired {}", change.path.display());
        }
        Ok(checked.findings.iter().all(|f| f.repairable))
    };
    match examine() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// What the daemon says: whether it starts and answers, and the tracking files that do not read.
/// Runs the smoke test of each language `timonier init` instrumented (ADR 0050): what is wrong,
/// and what went well.
#[tracing::instrument(skip_all)]
fn smoke(paths: &Paths) -> (Vec<doctor::Finding>, Vec<String>) {
    let by_hand = |what: String| doctor::Finding {
        what,
        repairable: false,
    };
    let runners = std::fs::read_to_string(paths.root.join(plan::TESTS))
        .ok()
        .and_then(|text| serde_json::from_str::<plan::Tests>(&text).ok())
        .map(|tests| tests.runners)
        .unwrap_or_default();
    let mut findings = Vec::new();
    let mut said = Vec::new();
    for runner in runners {
        let Some(command) = &runner.smoke else {
            continue;
        };
        let language = &runner.language;
        eprintln!("timonier: the smoke test ({language}): {command}");
        let outcome = crate::test::smoke(paths, command);
        let Some(run) = outcome.run else {
            findings.push(by_hand(format!(
                "the smoke test ({language}) was not recorded: is the daemon there?"
            )));
            continue;
        };
        if outcome.code != 0 {
            findings.push(by_hand(format!(
                "the smoke test ({language}) failed (exit status {}, run {run}): `{command}`; \
                 the OpenTelemetry SDK may need installing",
                outcome.code
            )));
            continue;
        }
        let report = daemon::connect_as(paths, "timonier-doctor").and_then(|mut client| {
            client
                .request::<serde_json::Value>(
                    timonier_core::protocol::methods::TOOLS_CALL,
                    &serde_json::json!({
                        "name": timonier_core::tools::GET_INSTRUMENTATION_GAPS,
                        "arguments": { "run": run, "limit": 1 },
                    }),
                )
                .ok()
        });
        let result = report
            .as_ref()
            .map(|r| r["structuredContent"]["result"].clone())
            .unwrap_or_default();
        let spans = result["spans"].as_u64().unwrap_or(0);
        let unattached = result["unattached"].as_u64().unwrap_or(0);
        if spans == 0 {
            findings.push(by_hand(format!(
                "the smoke test ({language}) sent no span to Timonier (run {run})"
            )));
        } else if unattached >= spans {
            findings.push(by_hand(format!(
                "the spans of the smoke test ({language}) attach to no node of the graph (run {run})"
            )));
        } else {
            said.push(format!(
                "the smoke test ({language}) produced a trace attached to the graph: {} of {spans} \
                 spans (run {run})",
                spans - unattached
            ));
        }
    }
    (findings, said)
}

fn daemon_findings(paths: &Paths) -> Vec<doctor::Finding> {
    let by_hand = |what: String| doctor::Finding {
        what,
        repairable: false,
    };
    if let Err(error) = daemon::launch(paths) {
        return vec![by_hand(format!("the daemon does not start: {error:#}"))];
    }
    let Some(mut client) = daemon::connect_as(paths, "timonier-doctor") else {
        return vec![by_hand("the daemon does not answer".to_owned())];
    };
    let snapshot: Result<timonier_core::protocol::TrackingSnapshot, _> =
        client.request(methods::TRACKING_SUBSCRIBE, &Value::Null);
    match snapshot {
        Ok(snapshot) => snapshot
            .tracking
            .problems
            .into_iter()
            .map(|p| by_hand(format!(".timonier/{} does not read: {}", p.file, p.message)))
            .collect(),
        Err(error) => vec![by_hand(format!(
            "the daemon does not give the tracking: {error}"
        ))],
    }
}

/// The project's root, as `--root` or the current directory give it, for the hooks.
pub fn here() -> Result<Paths> {
    daemon::resolve(&Target::new(None::<PathBuf>))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_keeps_names_or_drops_the_agents_found() {
        let found = [Agent::ClaudeCode, Agent::Cursor];
        assert_eq!(answered("\n", &found).unwrap(), found);
        assert_eq!(answered("Oui\n", &found).unwrap(), found);
        assert_eq!(answered("n", &found).unwrap(), []);
        assert_eq!(
            answered("codex, copilot", &found).unwrap(),
            [Agent::Codex, Agent::Copilot]
        );
        assert_eq!(answered("", &[]).unwrap(), []);
        assert!(answered("gemini", &found).is_err());
    }
}
