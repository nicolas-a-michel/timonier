//! `timonier doctor` (PLAN.md 6.10, ADR 0033): checks the setup `init` made, and repairs it. What
//! `init`'s plan would still change is a finding, and applying it the repair; what it cannot
//! write is said with how to do it by hand. Then what no file says: `timonier` on the `PATH`,
//! the daemon, the tracking files.

use std::path::Path;

use super::detect::{Agent, Environment, Git};
use super::plan::{self, Change, Choices, Planned, RECORD, Record};

/// A thing wrong, and whether `--fix` repairs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub what: String,
    pub repairable: bool,
}

pub struct Checked {
    pub findings: Vec<Finding>,
    /// What repairs the repairable findings.
    pub repair: Planned,
}

/// What a change of `init`'s plan says is wrong.
fn finding(change: &Change) -> Option<String> {
    let path = change.path.display().to_string();
    let missing = change.before.is_none();
    let agent = |agent: Agent| agent.label();
    Some(match path.as_str() {
        "AGENTS.md" if missing => "AGENTS.md is missing, with the agents' instructions".to_owned(),
        "AGENTS.md" => "the managed block of AGENTS.md is missing or out of date".to_owned(),
        "CLAUDE.md" => {
            "CLAUDE.md does not import AGENTS.md: Claude Code does not read the instructions"
                .to_owned()
        }
        ".mcp.json" => format!(
            "the MCP server is not declared for {} (.mcp.json)",
            agent(Agent::ClaudeCode)
        ),
        ".vscode/mcp.json" => format!(
            "the MCP server is not declared for {} (.vscode/mcp.json)",
            agent(Agent::Copilot)
        ),
        ".cursor/mcp.json" => format!(
            "the MCP server is not declared for {} (.cursor/mcp.json)",
            agent(Agent::Cursor)
        ),
        ".codex/config.toml" => format!(
            "the MCP server is not declared for {} (.codex/config.toml)",
            agent(Agent::Codex)
        ),
        ".claude/settings.json" => format!(
            "the stop hook of {} is missing (.claude/settings.json)",
            agent(Agent::ClaudeCode)
        ),
        ".github/hooks/timonier.json" => format!(
            "the stop hook of {} is missing (.github/hooks/timonier.json)",
            agent(Agent::Copilot)
        ),
        ".codex/hooks.json" => format!(
            "the stop hook of {} is missing (.codex/hooks.json)",
            agent(Agent::Codex)
        ),
        ".timonier/.gitignore" => {
            "nothing keeps the database .timonier/graph.sqlite out of git".to_owned()
        }
        // The record: bookkeeping, repaired with the rest.
        RECORD => return None,
        ".config/nextest.toml" => {
            "the nextest profile that writes the tests' JUnit XML is missing".to_owned()
        }
        plan::TESTS => {
            "the record of the test runners (.timonier/tests.json) is missing or out of date"
                .to_owned()
        }
        _ if path.starts_with(".timonier/conventions/") => {
            let language = path.split('/').nth(2).unwrap_or("?");
            format!("the {language} conventions are missing or out of date ({path})")
        }
        _ if path.ends_with("pre-commit") => {
            "the git hook that writes .timonier/actual.json is missing".to_owned()
        }
        ".timonier/intent.json" => {
            "the whiteboard (.timonier/intent.json) is missing: `--fix` puts an empty one"
                .to_owned()
        }
        _ => format!("{path} is not as Timonier writes it"),
    })
}

/// The setup of the project at `root`, checked against its record.
pub fn check(root: &Path, environment: &Environment, git: Git) -> Checked {
    let record: Option<Record> = std::fs::read_to_string(root.join(RECORD))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let Some(record) = record else {
        return Checked {
            findings: vec![Finding {
                what: "Timonier is not set up here: run `timonier init`".to_owned(),
                repairable: false,
            }],
            repair: Planned {
                changes: Vec::new(),
                notes: Vec::new(),
                info: Vec::new(),
            },
        };
    };
    let agents: Vec<Agent> = record
        .agents
        .iter()
        .filter_map(|a| Agent::parse(a))
        .collect();
    let runners = super::detect::detect(root, environment).runners;
    let repair = plan::plan(
        root,
        &Choices {
            agents,
            git,
            runners,
            path: environment.path.clone(),
        },
    );
    let mut findings: Vec<Finding> = repair
        .changes
        .iter()
        .filter_map(finding)
        .map(|what| Finding {
            what,
            repairable: true,
        })
        .collect();
    findings.extend(repair.notes.iter().map(|note| Finding {
        what: note.clone(),
        repairable: false,
    }));
    let on_path = environment
        .path
        .iter()
        .any(|dir| dir.join("timonier").is_file());
    if !on_path {
        findings.push(Finding {
            what: "`timonier` is not on the PATH: the agents cannot start the MCP server, nor the \
                   hooks run. Install it (`cargo install --path crates/timonier-cli`) or add its \
                   folder to the PATH"
                .to_owned(),
            repairable: false,
        });
    }
    Checked { findings, repair }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path) -> Git {
        Git::Repository {
            hooks: root.join(".git/hooks"),
            hooks_path: None,
        }
    }

    fn on_path() -> (tempfile::TempDir, Environment) {
        let bin = tempfile::tempdir().unwrap();
        std::fs::write(bin.path().join("timonier"), "").unwrap();
        let environment = Environment {
            path: vec![bin.path().to_path_buf()],
            ..Environment::default()
        };
        (bin, environment)
    }

    fn set_up(root: &Path, agents: &[Agent]) {
        let planned = plan::plan(
            root,
            &Choices {
                agents: agents.to_vec(),
                git: git(root),
                runners: Vec::new(),
                path: Vec::new(),
            },
        );
        plan::apply(root, &planned).unwrap();
    }

    fn whats(checked: &Checked) -> Vec<String> {
        checked.findings.iter().map(|f| f.what.clone()).collect()
    }

    #[test]
    fn a_project_never_set_up_is_told_to_run_init() {
        let root = tempfile::tempdir().unwrap();
        let (_bin, environment) = on_path();
        let checked = check(root.path(), &environment, git(root.path()));
        assert_eq!(
            whats(&checked),
            ["Timonier is not set up here: run `timonier init`"]
        );
        assert!(!checked.findings[0].repairable);
    }

    #[test]
    fn a_setup_in_order_has_nothing_to_say() {
        let root = tempfile::tempdir().unwrap();
        set_up(root.path(), &Agent::ALL);
        let (_bin, environment) = on_path();
        assert_eq!(
            whats(&check(root.path(), &environment, git(root.path()))),
            Vec::<String>::new()
        );
    }

    /// Each finding, then its repair: after it, nothing to say.
    #[test]
    fn each_finding_is_repaired() {
        let root = tempfile::tempdir().unwrap();
        let path = |p: &str| root.path().join(p);
        set_up(root.path(), &Agent::ALL);
        // Broken by hand, one by one.
        std::fs::write(path("AGENTS.md"), "# Mine\n").unwrap();
        std::fs::write(path("CLAUDE.md"), "# Created after init\n").unwrap();
        std::fs::write(path(".mcp.json"), "{\"mcpServers\": {}}\n").unwrap();
        std::fs::remove_file(path(".vscode/mcp.json")).unwrap();
        std::fs::write(path(".cursor/mcp.json"), "{}\n").unwrap();
        std::fs::write(path(".codex/config.toml"), "model = \"x\"\n").unwrap();
        std::fs::write(path(".claude/settings.json"), "{}\n").unwrap();
        std::fs::remove_file(path(".github/hooks/timonier.json")).unwrap();
        std::fs::write(path(".codex/hooks.json"), "{}\n").unwrap();
        std::fs::remove_file(path(".timonier/.gitignore")).unwrap();
        std::fs::write(path(".git/hooks/pre-commit"), "#!/bin/sh\nmake lint\n").unwrap();
        let (_bin, environment) = on_path();
        let checked = check(root.path(), &environment, git(root.path()));
        assert_eq!(
            whats(&checked),
            [
                "the managed block of AGENTS.md is missing or out of date",
                "CLAUDE.md does not import AGENTS.md: Claude Code does not read the instructions",
                "the MCP server is not declared for Claude Code (.mcp.json)",
                "the MCP server is not declared for GitHub Copilot (VSCode) (.vscode/mcp.json)",
                "the MCP server is not declared for Cursor (.cursor/mcp.json)",
                "the MCP server is not declared for Codex (.codex/config.toml)",
                "the stop hook of Claude Code is missing (.claude/settings.json)",
                "the stop hook of GitHub Copilot (VSCode) is missing (.github/hooks/timonier.json)",
                "the stop hook of Codex is missing (.codex/hooks.json)",
                "nothing keeps the database .timonier/graph.sqlite out of git",
                "the git hook that writes .timonier/actual.json is missing",
            ]
        );
        assert!(checked.findings.iter().all(|f| f.repairable));
        plan::apply(root.path(), &checked.repair).unwrap();
        assert_eq!(
            whats(&check(root.path(), &environment, git(root.path()))),
            Vec::<String>::new()
        );
        // What was the developer's stays.
        assert!(
            std::fs::read_to_string(path("AGENTS.md"))
                .unwrap()
                .starts_with("# Mine\n")
        );
        assert!(
            std::fs::read_to_string(path(".git/hooks/pre-commit"))
                .unwrap()
                .ends_with("make lint\n")
        );
    }

    /// ADR 0034: the runner's profile, the record of the runners and the conventions.
    #[test]
    fn the_runner_and_the_conventions_are_checked_and_repaired() {
        let root = tempfile::tempdir().unwrap();
        let path = |p: &str| root.path().join(p);
        std::fs::write(path("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();
        let (_bin, mut environment) = on_path();
        std::fs::write(environment.path[0].join("cargo-nextest"), "").unwrap();
        let runners = super::super::detect::detect(root.path(), &environment).runners;
        let planned = plan::plan(
            root.path(),
            &Choices {
                agents: Vec::new(),
                git: git(root.path()),
                runners,
                path: environment.path.clone(),
            },
        );
        plan::apply(root.path(), &planned).unwrap();
        assert_eq!(
            whats(&check(root.path(), &environment, git(root.path()))),
            Vec::<String>::new()
        );
        std::fs::remove_file(path(".timonier/conventions/rust/instrumentation.md")).unwrap();
        std::fs::write(path(".config/nextest.toml"), "").unwrap();
        std::fs::remove_file(path(".timonier/tests.json")).unwrap();
        let checked = check(root.path(), &environment, git(root.path()));
        assert_eq!(
            whats(&checked),
            [
                "the nextest profile that writes the tests' JUnit XML is missing",
                "the rust conventions are missing or out of date (.timonier/conventions/rust/instrumentation.md)",
                "the record of the test runners (.timonier/tests.json) is missing or out of date",
            ]
        );
        plan::apply(root.path(), &checked.repair).unwrap();
        assert_eq!(
            whats(&check(root.path(), &environment, git(root.path()))),
            Vec::<String>::new()
        );
        // Without nextest: to do by hand, and said how.
        environment.path.clear();
        let missing = whats(&check(root.path(), &environment, git(root.path())));
        assert!(
            missing
                .iter()
                .any(|w| w.contains("cargo install cargo-nextest")),
            "{missing:?}"
        );
    }

    #[test]
    fn what_cannot_be_repaired_is_said_with_how() {
        let root = tempfile::tempdir().unwrap();
        set_up(root.path(), &[Agent::Copilot]);
        std::fs::write(root.path().join(".vscode/mcp.json"), "{ // mine\n}\n").unwrap();
        let checked = check(root.path(), &Environment::default(), git(root.path()));
        let found = whats(&checked);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found[0].contains(".vscode/mcp.json was left as it is") && found[0].contains("by hand")
        );
        assert!(found[1].contains("`timonier` is not on the PATH"));
        assert!(checked.findings.iter().all(|f| !f.repairable));
    }
}
