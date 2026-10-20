//! What `timonier init` would change, file by file, from what the project holds now and what
//! was chosen (ADR 0033). Nothing is written here: `--dry-run` shows the plan, `init` applies
//! it, and a second run finds nothing to change.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::detect::{Agent, Git, Runner};
use super::edits::{self, HASH_BEGIN, HASH_END, HookShape, MD_BEGIN, MD_END};
use super::ordered::Json;

/// What the agents read, in `AGENTS.md` (spec « Faire respecter le protocole »).
pub const INSTRUCTIONS: &str = "\
## Timonier

This project is steered with Timonier. Its MCP server, `timonier`, holds the project's plan, \
the todo of the session in progress and the journal of the sessions; the developer follows them \
live in the IDE.

- Before changing code, open a session: `session_start` with the objective of the work, and the \
plan task it serves when there is one.
- Split the work into todo items with `todo_add`, and mark each one done with `todo_complete` as \
you go.
- Keep the plan current: `plan_add_task` for new work, `plan_update_task` when a task starts, is \
blocked, done or dropped.
- When the work is done, call `session_end` with a summary of what was done and what is left.
- Never edit `.timonier/` by hand: the tools write `plan.json`, `todo.json` and `sessions/`, and \
`intent.json`, the whiteboard, belongs to the developer.
- Instrument the code as `.timonier/conventions/<language>/instrumentation.md` says.
- Every tool result starts with a mailbox header: read the messages it announces.";

/// What `CLAUDE.md` holds when it exists: Claude Code then reads it instead of `AGENTS.md`.
pub const CLAUDE_IMPORT: &str = "@AGENTS.md";

/// What the git hook runs (Q50): never in the way of a commit.
pub const PRE_COMMIT: &str = "\
# Writes .timonier/actual.json and adds it to the commit (timonier init).
if command -v timonier >/dev/null 2>&1; then timonier hook pre-commit || true; fi";

/// The stop hook of each agent that can hold a stop (Q49).
pub fn stop_command(agent: Agent) -> String {
    format!("timonier hook stop --agent {}", agent.key())
}

/// `.timonier/init.json`: what `init` made, for `uninit` and `doctor`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u32,
    /// The agents configured.
    pub agents: Vec<String>,
    /// The files `init` created, relative to the root (or absolute, outside it).
    pub created: Vec<String>,
    /// The files it changed, each as it was before.
    pub modified: Vec<Modified>,
    /// `.timonier/` did not exist before the first `init`: `uninit` may take it all back.
    #[serde(default)]
    pub fresh: bool,
    /// A plan or a session was there before the first `init`: the tracking is not `init`'s
    /// (Q54). A folder the daemon made (its database, its ignore file) is no tracking. Left
    /// out when false: a record written before the field stays as it is (review n° 3 of Phase 6).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tracked_before: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modified {
    pub path: String,
    pub original: String,
}

pub const RECORD: &str = ".timonier/init.json";

/// One file, before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// Relative to the root, or absolute (a git hook outside the working tree).
    pub path: PathBuf,
    pub before: Option<String>,
    /// `None`: the file goes (`uninit`).
    pub after: Option<String>,
    /// A script git runs.
    pub executable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub changes: Vec<Change>,
    /// What `init` could not do itself, and how to do it by hand.
    pub notes: Vec<String>,
    /// What is good to know, with nothing to do.
    pub info: Vec<String>,
}

pub struct Choices {
    pub agents: Vec<Agent>,
    pub git: Git,
    /// The test runner of each language, with its conventions (ADR 0034).
    pub runners: Vec<Runner>,
    /// Where programs are looked for, as the `PATH` says.
    pub path: Vec<PathBuf>,
}

/// `.timonier/tests.json`: how the tests of each language run and write JUnit XML, for
/// `timonier test` (ADR 0034).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub version: u32,
    pub runners: Vec<TestRunner>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRunner {
    pub language: String,
    pub runner: String,
    /// `null` when the runner cannot write JUnit XML as it is (see the note of `init`).
    pub command: Option<String>,
    pub junit: Option<String>,
}

pub const TESTS: &str = ".timonier/tests.json";

fn read(root: &Path, path: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(path)).ok()
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

pub fn plan(root: &Path, choices: &Choices) -> Planned {
    let mut changes = Vec::new();
    let mut notes = Vec::new();
    let mut change = |path: PathBuf, after: Result<String, String>, executable: bool, how: &str| {
        let before = read(root, &path);
        match after {
            Ok(after) if before.as_deref() != Some(after.as_str()) => changes.push(Change {
                path,
                before,
                after: Some(after),
                executable,
            }),
            Ok(_) => {}
            Err(why) => notes.push(format!(
                "{} was left as it is: {why}. Add {how} by hand.",
                display(&path)
            )),
        }
    };

    // The instructions: one block in AGENTS.md, read by the four agents (ADR 0033).
    let agents_md = PathBuf::from("AGENTS.md");
    let current = read(root, &agents_md);
    change(
        agents_md,
        Ok(edits::with_block(
            current.as_deref(),
            MD_BEGIN,
            MD_END,
            INSTRUCTIONS,
        )),
        false,
        "the block",
    );
    let has = |agent: Agent| choices.agents.contains(&agent);
    let claude_md = PathBuf::from("CLAUDE.md");
    if has(Agent::ClaudeCode)
        && let Some(current) = read(root, &claude_md)
    {
        change(
            claude_md,
            Ok(edits::with_block(
                Some(&current),
                MD_BEGIN,
                MD_END,
                CLAUDE_IMPORT,
            )),
            false,
            "`@AGENTS.md`",
        );
    }

    // The MCP server, in each agent's own format.
    // As the agents' documentations write it: the type when they ask for it, the command, its
    // arguments.
    let server = |root_arg: bool, typed: bool| {
        let text = |s: &str| Json::String(s.to_owned());
        let args: Vec<Json> = if root_arg {
            vec![text("mcp"), text("--root"), text("${workspaceFolder}")]
        } else {
            vec![text("mcp")]
        };
        let mut entries = Vec::new();
        if typed {
            entries.push(("type".to_owned(), text("stdio")));
        }
        entries.push(("command".to_owned(), text("timonier")));
        entries.push(("args".to_owned(), Json::Array(args)));
        Json::Object(entries)
    };
    let mut json_entry = |path: &str, section: &str, entry: Json| {
        let path = PathBuf::from(path);
        let current = read(root, &path);
        let how = format!("`\"timonier\": {}` under `{section}`", entry.value());
        change(
            path,
            edits::json_with_entry(current.as_deref(), section, "timonier", entry),
            false,
            &how,
        );
    };
    if has(Agent::ClaudeCode) {
        json_entry(".mcp.json", "mcpServers", server(false, false));
    }
    if has(Agent::Copilot) {
        json_entry(".vscode/mcp.json", "servers", server(true, true));
    }
    if has(Agent::Cursor) {
        json_entry(".cursor/mcp.json", "mcpServers", server(true, true));
    }
    if has(Agent::Codex) {
        let path = PathBuf::from(".codex/config.toml");
        let current = read(root, &path);
        let body = "[mcp_servers.timonier]\ncommand = \"timonier\"\nargs = [\"mcp\"]";
        let declared = current.as_deref().is_some_and(|text| {
            !text.contains(HASH_BEGIN) && text.contains("[mcp_servers.timonier]")
        });
        change(
            path,
            if declared {
                Err("it declares `mcp_servers.timonier` already".to_owned())
            } else {
                Ok(edits::with_block(
                    current.as_deref(),
                    HASH_BEGIN,
                    HASH_END,
                    body,
                ))
            },
            false,
            "nothing",
        );
    }

    // The stop hooks (Q49); Cursor cannot hold a stop.
    let mut json_hook = |path: &str, agent: Agent, shape: HookShape| {
        let path = PathBuf::from(path);
        let current = read(root, &path);
        let command = stop_command(agent);
        let how = format!("a `Stop` hook that runs `{command}`");
        change(
            path,
            edits::json_with_hook(current.as_deref(), "Stop", &command, shape),
            false,
            &how,
        );
    };
    if has(Agent::ClaudeCode) {
        json_hook(
            ".claude/settings.json",
            Agent::ClaudeCode,
            HookShape::Grouped,
        );
    }
    if has(Agent::Copilot) {
        json_hook(
            ".github/hooks/timonier.json",
            Agent::Copilot,
            HookShape::Flat,
        );
    }
    if has(Agent::Codex) {
        json_hook(".codex/hooks.json", Agent::Codex, HookShape::Grouped);
    }

    // The test runners and the conventions of each language (ADR 0034).
    let mut runner_notes = Vec::new();
    let mut info = Vec::new();
    for runner in &choices.runners {
        let rule = &runner.rule;
        if let Some(config) = &rule.config {
            let path = PathBuf::from(&config.file);
            let current = read(root, &path);
            change(
                path,
                Ok(edits::with_block(
                    current.as_deref(),
                    HASH_BEGIN,
                    HASH_END,
                    &config.text,
                )),
                false,
                "the block",
            );
        }
        let missing = rule
            .program
            .as_ref()
            .is_some_and(|program| !choices.path.iter().any(|dir| dir.join(program).is_file()));
        if let Some(note) = &rule.note
            && (rule.program.is_none() || missing)
        {
            runner_notes.push(format!("{} ({}): {note}", rule.name, runner.language));
        }
        if let Some(said) = &rule.info {
            info.push(format!("{} ({}): {said}", rule.name, runner.language));
        }
        for (name, text) in &runner.conventions {
            let path = PathBuf::from(format!(".timonier/conventions/{}/{name}", runner.language));
            change(path, Ok(text.clone()), false, "");
        }
    }
    if !choices.runners.is_empty() {
        let tests = Tests {
            version: timonier_core::tracking::FORMAT_VERSION,
            runners: choices
                .runners
                .iter()
                .map(|r| TestRunner {
                    language: r.language.clone(),
                    runner: r.rule.name.clone(),
                    command: r.rule.command.clone(),
                    junit: r.rule.junit.clone(),
                })
                .collect(),
        };
        let mut text = serde_json::to_string_pretty(&tests).unwrap_or_default();
        text.push('\n');
        change(PathBuf::from(TESTS), Ok(text), false, "");
    }

    // `.timonier/`: the database stays out of git.
    let ignore = PathBuf::from(".timonier/.gitignore");
    if read(root, &ignore).is_none() {
        change(
            ignore,
            Ok(timonier_daemon::runtime::GITIGNORE.to_owned()),
            false,
            "",
        );
    }

    // The git hook that writes actual.json before each commit (Q50).
    match &choices.git {
        Git::None => notes.push(
            "No git repository: no hook writes .timonier/actual.json at each commit. Run \
             `timonier init` again once the project is in git."
                .to_owned(),
        ),
        Git::Repository {
            hooks_path: Some(manager),
            ..
        } => notes.push(format!(
            "core.hooksPath is `{manager}`: a hook manager runs the git hooks. Add \
             `timonier hook pre-commit` to its pre-commit hook."
        )),
        Git::Repository { hooks, .. } => {
            let path = hooks.join("pre-commit");
            let path = path
                .strip_prefix(root)
                .map_or(path.clone(), Path::to_path_buf);
            let current = read(root, &path);
            change(
                path,
                edits::shell_with_block(current.as_deref(), PRE_COMMIT),
                true,
                "`timonier hook pre-commit`",
            );
        }
    }

    // What was made, for `uninit`.
    let record_path = PathBuf::from(RECORD);
    let previous: Record = read(root, &record_path)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let first = read(root, &record_path).is_none();
    let fresh = if first {
        !root.join(".timonier").exists()
    } else {
        previous.fresh
    };
    let tracked_before = if first {
        let dir = root.join(".timonier");
        dir.join("plan.json").exists()
            || std::fs::read_dir(dir.join("sessions"))
                .into_iter()
                .flatten()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("S-"))
    } else {
        previous.tracked_before
    };
    let mut record = record_of(previous, &changes, &choices.agents);
    record.fresh = fresh;
    record.tracked_before = tracked_before;
    let mut text = serde_json::to_string_pretty(&record).unwrap_or_default();
    text.push('\n');
    if read(root, &record_path).as_deref() != Some(text.as_str()) {
        changes.push(Change {
            before: read(root, &record_path),
            path: record_path,
            after: Some(text),
            executable: false,
        });
    }
    notes.extend(runner_notes);
    Planned {
        changes,
        notes,
        info,
    }
}

/// The record after `changes`: a file first created by `init` stays created, one it first
/// changed keeps its original.
fn record_of(previous: Record, changes: &[Change], agents: &[Agent]) -> Record {
    let mut created: BTreeSet<String> = previous.created.into_iter().collect();
    let mut modified = previous.modified;
    for change in changes {
        let path = display(&change.path);
        if created.contains(&path) || modified.iter().any(|m| m.path == path) {
            continue;
        }
        match &change.before {
            None => {
                created.insert(path);
            }
            Some(original) => modified.push(Modified {
                path,
                original: original.clone(),
            }),
        }
    }
    // The record itself is no file of the developer's.
    created.insert(RECORD.to_owned());
    modified.sort_by(|a, b| a.path.cmp(&b.path));
    let mut configured: BTreeSet<String> = previous.agents.into_iter().collect();
    configured.extend(agents.iter().map(|a| a.key().to_owned()));
    Record {
        version: timonier_core::tracking::FORMAT_VERSION,
        agents: configured.into_iter().collect(),
        created: created.into_iter().collect(),
        modified,
        fresh: previous.fresh,
        tracked_before: previous.tracked_before,
    }
}

/// Applies the plan: each file written whole, scripts made executable.
pub fn apply(root: &Path, planned: &Planned) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for change in &planned.changes {
        let path = root.join(&change.path);
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let Some(after) = &change.after else {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            continue;
        };
        std::fs::write(&path, after)?;
        if change.executable {
            let mut permissions = std::fs::metadata(&path)?.permissions();
            permissions.set_mode(permissions.mode() | 0o755);
            std::fs::set_permissions(&path, permissions)?;
        }
    }
    Ok(())
}

/// The plan as a unified diff, file by file.
pub fn diff(planned: &Planned) -> String {
    let mut out = String::new();
    for change in &planned.changes {
        let name = display(&change.path);
        let before = change.before.as_deref().unwrap_or("");
        let old = if change.before.is_some() {
            format!("a/{name}")
        } else {
            "/dev/null".to_owned()
        };
        let after = change.after.as_deref().unwrap_or("");
        let new = if change.after.is_some() {
            format!("b/{name}")
        } else {
            "/dev/null".to_owned()
        };
        let text = similar::TextDiff::from_lines(before, after);
        out.push_str(
            &text
                .unified_diff()
                .context_radius(3)
                .header(&old, &new)
                .to_string(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices(agents: &[Agent], root: &Path) -> Choices {
        Choices {
            agents: agents.to_vec(),
            git: Git::Repository {
                hooks: root.join(".git/hooks"),
                hooks_path: None,
            },
            runners: Vec::new(),
            path: Vec::new(),
        }
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("CLAUDE.md"), "# Mine\n").unwrap();
        let chosen = choices(&Agent::ALL, root.path());
        let first = plan(root.path(), &chosen);
        let paths: Vec<String> = first.changes.iter().map(|c| display(&c.path)).collect();
        assert_eq!(
            paths,
            [
                "AGENTS.md",
                "CLAUDE.md",
                ".mcp.json",
                ".vscode/mcp.json",
                ".cursor/mcp.json",
                ".codex/config.toml",
                ".claude/settings.json",
                ".github/hooks/timonier.json",
                ".codex/hooks.json",
                ".timonier/.gitignore",
                ".git/hooks/pre-commit",
                ".timonier/init.json",
            ]
        );
        assert!(first.notes.is_empty(), "{:?}", first.notes);
        apply(root.path(), &first).unwrap();
        let second = plan(root.path(), &chosen);
        assert_eq!(second.changes, Vec::new(), "{}", diff(&second));
        let record: Record =
            serde_json::from_str(&std::fs::read_to_string(root.path().join(RECORD)).unwrap())
                .unwrap();
        assert_eq!(
            record.modified,
            [Modified {
                path: "CLAUDE.md".into(),
                original: "# Mine\n".into()
            }]
        );
        assert!(record.created.contains(&".mcp.json".to_owned()));
        assert_eq!(record.agents, ["claude", "codex", "copilot", "cursor"]);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.path().join(".git/hooks/pre-commit"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }

    #[test]
    fn what_cannot_be_read_is_left_and_said() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".vscode")).unwrap();
        std::fs::write(root.path().join(".vscode/mcp.json"), "{ // mine\n}\n").unwrap();
        let mut chosen = choices(&[Agent::Copilot], root.path());
        chosen.git = Git::Repository {
            hooks: root.path().join(".husky/_"),
            hooks_path: Some(".husky/_".into()),
        };
        let planned = plan(root.path(), &chosen);
        assert!(
            !planned
                .changes
                .iter()
                .any(|c| c.path == Path::new(".vscode/mcp.json"))
        );
        assert_eq!(planned.notes.len(), 2, "{:?}", planned.notes);
        assert!(planned.notes[0].contains(".vscode/mcp.json was left as it is"));
        assert!(planned.notes[1].contains("core.hooksPath is `.husky/_`"));
    }

    #[test]
    fn the_dry_run_shows_the_exact_diff() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("AGENTS.md"), "# Mine\n").unwrap();
        let planned = plan(root.path(), &choices(&[], root.path()));
        let shown = diff(&planned);
        assert!(
            shown.contains("--- a/AGENTS.md\n+++ b/AGENTS.md\n"),
            "{shown}"
        );
        assert!(shown.contains("+<!-- timonier:begin -->"));
        assert!(shown.contains("--- /dev/null\n+++ b/.timonier/init.json"));
        // Nothing written.
        assert_eq!(
            std::fs::read_to_string(root.path().join("AGENTS.md")).unwrap(),
            "# Mine\n"
        );
    }
}
