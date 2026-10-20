//! `timonier hook stop|pre-commit`: what the hooks `timonier init` installs run (ADR 0033).
//! Neither ever gets in the way: a daemon that does not answer holds nobody and blocks no
//! commit.

use std::io::Read;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use timonier_core::protocol::{ActualWritten, AgentStop, StatusResult, methods};

use crate::daemon;

#[derive(Debug, Subcommand)]
pub enum Hook {
    /// An agent wants to stop: hold it once if its session is not in order (Q49)
    Stop {
        /// Which agent asks, for the format of the answer
        #[arg(long, value_enum)]
        agent: StopFormat,
    },
    /// Before a commit: write .timonier/actual.json and add it to the commit (Q50)
    PreCommit,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum StopFormat {
    Claude,
    Copilot,
    Codex,
}

/// How a hook names itself to the daemon.
const CLIENT: &str = "timonier-hook";

/// How long the pre-commit hook waits for a daemon it started to index the project.
const INDEXING: Duration = Duration::from_secs(30);

pub fn run(hook: &Hook) -> ExitCode {
    match hook {
        Hook::Stop { agent } => stop(*agent),
        Hook::PreCommit => pre_commit(),
    }
    ExitCode::SUCCESS
}

/// Reads what the agent says on stdin, and answers in its format when it is held.
fn stop(format: StopFormat) {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let input: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    if let Some(answer) = decide(&input, format, reason) {
        println!("{answer}");
    }
}

/// Held once (Q49): an agent already held (`stop_hook_active`) goes; otherwise the daemon's
/// reason, if any, in the agent's format.
fn decide(
    input: &Value,
    format: StopFormat,
    reason: impl FnOnce() -> Option<String>,
) -> Option<Value> {
    if input["stop_hook_active"] == true {
        return None;
    }
    let reason = reason()?;
    Some(match format {
        StopFormat::Claude | StopFormat::Codex => json!({ "decision": "block", "reason": reason }),
        StopFormat::Copilot => json!({
            "hookSpecificOutput": { "hookEventName": "Stop", "decision": "block", "reason": reason }
        }),
    })
}

fn reason() -> Option<String> {
    let paths = crate::init::here().ok()?;
    let mut client = daemon::connect_as(&paths, CLIENT)?;
    let stop: AgentStop = client.request(methods::AGENT_STOP, &Value::Null).ok()?;
    stop.reason
}

fn pre_commit() {
    if let Err(error) = write_actual() {
        eprintln!("timonier: .timonier/actual.json not updated: {error:#}");
    }
}

fn write_actual() -> anyhow::Result<()> {
    use anyhow::Context;
    let paths = crate::init::here()?;
    daemon::launch(&paths)?;
    let mut client = daemon::connect_as(&paths, CLIENT).context("the daemon does not answer")?;
    let deadline = Instant::now() + INDEXING;
    loop {
        let status: StatusResult = client.request(methods::STATUS, &Value::Null)?;
        if status.indexed {
            break;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "the project is still being indexed"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let written: ActualWritten = client.request(methods::ACTUAL_WRITE, &Value::Null)?;
    let added = Command::new("git")
        .args(["add", "--", &written.path])
        .current_dir(&paths.root)
        .status()
        .context("cannot run git")?;
    anyhow::ensure!(added.success(), "git add {} failed", written.path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held() -> Option<String> {
        Some("open a session".to_owned())
    }

    #[test]
    fn an_agent_is_held_once_in_its_format() {
        let first = json!({"stop_hook_active": false});
        assert_eq!(
            decide(&first, StopFormat::Claude, held),
            Some(json!({"decision": "block", "reason": "open a session"}))
        );
        assert_eq!(
            decide(&first, StopFormat::Codex, held).unwrap()["decision"],
            "block"
        );
        assert_eq!(
            decide(&first, StopFormat::Copilot, held).unwrap()["hookSpecificOutput"],
            json!({"hookEventName": "Stop", "decision": "block", "reason": "open a session"})
        );
        // Held already: it goes, and the daemon is not even asked.
        let again = json!({"stop_hook_active": true});
        assert_eq!(decide(&again, StopFormat::Claude, || unreachable!()), None);
        // Nothing to say: it goes. An input that does not read was not held yet: the daemon says.
        assert_eq!(decide(&first, StopFormat::Claude, || None), None);
        assert_eq!(
            decide(&Value::Null, StopFormat::Claude, held).unwrap()["decision"],
            "block"
        );
    }
}
