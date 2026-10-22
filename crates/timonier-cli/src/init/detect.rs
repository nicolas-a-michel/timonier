//! What `timonier init` finds before it asks anything (ADR 0033): the languages and their
//! package managers, the test runners, git, and the agents on this machine or in the project.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The agents `timonier init` configures (Q48).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Agent {
    ClaudeCode,
    Copilot,
    Cursor,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 4] = [
        Agent::ClaudeCode,
        Agent::Copilot,
        Agent::Cursor,
        Agent::Codex,
    ];

    /// The name `--agents` takes.
    pub fn key(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude",
            Agent::Copilot => "copilot",
            Agent::Cursor => "cursor",
            Agent::Codex => "codex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Copilot => "GitHub Copilot (VSCode)",
            Agent::Cursor => "Cursor",
            Agent::Codex => "Codex",
        }
    }

    pub fn parse(key: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.key() == key)
    }
}

/// Where to look on this machine: the home directory and the `PATH`. Tests give their own.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub home: Option<PathBuf>,
    pub path: Vec<PathBuf>,
    /// `$CODEX_HOME`, when set.
    pub codex_home: Option<PathBuf>,
}

impl Environment {
    pub fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME").map(PathBuf::from),
            path: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
            codex_home: std::env::var_os("CODEX_HOME").map(PathBuf::from),
        }
    }

    fn on_path(&self, program: &str) -> bool {
        self.path.iter().any(|dir| dir.join(program).is_file())
    }

    fn in_home(&self, relative: &str) -> bool {
        self.home
            .as_ref()
            .is_some_and(|h| h.join(relative).exists())
    }

    /// A VSCode extension whose folder starts with `id` (`anthropic.claude-code-2.1.301-…`).
    fn extension(&self, id: &str) -> bool {
        let Some(home) = &self.home else {
            return false;
        };
        std::fs::read_dir(home.join(".vscode/extensions"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&format!("{id}-"))
            })
    }
}

/// A language's test runner, and its conventions, from its pack.
#[derive(Clone, Debug)]
pub struct Runner {
    pub language: String,
    /// `cargo` or `npm`: which manifest takes the SDK (ADR 0050).
    pub ecosystem: String,
    pub rule: timonier_daemon::RunnerRule,
    /// `conventions/<name>.md` of the pack, by name.
    pub conventions: Vec<(String, String)>,
    /// The OpenTelemetry SDK and module the pack adds, and its templates by name.
    pub instrumentation: Option<timonier_daemon::InstrumentationRules>,
    pub templates: std::collections::BTreeMap<String, String>,
}

impl PartialEq for Runner {
    fn eq(&self, other: &Self) -> bool {
        self.language == other.language && self.rule.name == other.rule.name
    }
}

impl Eq for Runner {}

/// Git, as the project's working tree has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Git {
    /// Not a git repository, or no git program.
    None,
    Repository {
        /// Where git runs the hooks from (`git rev-parse --git-path hooks`).
        hooks: PathBuf,
        /// `core.hooksPath`, when a hook manager (husky, lefthook…) set it.
        hooks_path: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    /// `rust`, `typescript`, `javascript`.
    pub languages: Vec<&'static str>,
    /// `cargo`, `npm`, `pnpm`, `yarn`, `bun`.
    pub package_managers: Vec<&'static str>,
    /// The test runner of each language, as its pack finds it (ADR 0034).
    pub runners: Vec<Runner>,
    /// The agents found, each with why.
    pub agents: Vec<(Agent, String)>,
    pub git: Git,
}

pub fn detect(root: &Path, environment: &Environment) -> Detection {
    let mut languages = Vec::new();
    let mut package_managers = Vec::new();
    let mut test_script = None;
    if root.join("Cargo.toml").is_file() {
        languages.push("rust");
        package_managers.push("cargo");
    }
    if let Ok(manifest) = std::fs::read_to_string(root.join("package.json")) {
        let typescript = ["tsconfig.json", "tsconfig.base.json"]
            .iter()
            .any(|f| root.join(f).is_file())
            || serde_json::from_str::<serde_json::Value>(&manifest).is_ok_and(|m| {
                ["dependencies", "devDependencies"]
                    .iter()
                    .any(|d| m[d].get("typescript").is_some())
            });
        languages.push(if typescript {
            "typescript"
        } else {
            "javascript"
        });
        let manager = [
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "yarn"),
            ("bun.lock", "bun"),
            ("bun.lockb", "bun"),
        ]
        .into_iter()
        .find(|(lock, _)| root.join(lock).is_file())
        .map_or("npm", |(_, manager)| manager);
        package_managers.push(manager);
        test_script = serde_json::from_str::<serde_json::Value>(&manifest)
            .ok()
            .and_then(|m| m["scripts"]["test"].as_str().map(str::to_owned));
    }
    Detection {
        runners: runners(root, &languages, test_script.as_deref()),
        languages,
        package_managers,
        agents: agents(root, environment),
        git: git(root),
    }
}

/// The first runner of each pack whose ecosystem the project uses, and that the `test` script
/// names when the rule asks for a word.
/// The test runner of each language found: a TypeScript project and a JavaScript one share
/// `package.json`, and only the language detected takes it (T-120).
fn runners(root: &Path, languages: &[&str], test_script: Option<&str>) -> Vec<Runner> {
    let Ok(packs) = timonier_daemon::Packs::builtin() else {
        return Vec::new();
    };
    packs
        .iter()
        .filter(|pack| languages.contains(&pack.language.as_str()))
        .filter(|pack| {
            root.join(match pack.manifest.ecosystem.as_str() {
                "cargo" => "Cargo.toml",
                _ => "package.json",
            })
            .is_file()
        })
        .filter_map(|pack| {
            let rule = pack
                .manifest
                .tests
                .runners
                .iter()
                .find(|rule| match &rule.uses {
                    None => true,
                    Some(word) => test_script.is_some_and(|script| script.contains(word.as_str())),
                })?;
            Some(Runner {
                language: pack.language.to_string(),
                ecosystem: pack.manifest.ecosystem.as_str().to_owned(),
                instrumentation: pack.manifest.instrumentation.clone(),
                templates: pack.templates.clone(),
                rule: rule.clone(),
                conventions: pack
                    .conventions
                    .iter()
                    .map(|(name, text)| (name.clone(), text.clone()))
                    .collect(),
            })
        })
        .collect()
}

fn agents(root: &Path, environment: &Environment) -> Vec<(Agent, String)> {
    let mut found = Vec::new();
    let mut note = |agent: Agent, why: Option<&str>| {
        if let Some(why) = why {
            found.push((agent, why.to_owned()));
        }
    };
    let first = |reasons: &[(bool, &'static str)]| {
        reasons.iter().find(|(yes, _)| *yes).map(|(_, why)| *why)
    };
    note(
        Agent::ClaudeCode,
        first(&[
            (
                root.join("CLAUDE.md").is_file() || root.join(".claude").is_dir(),
                "configured in the project",
            ),
            (environment.on_path("claude"), "`claude` is on the PATH"),
            (environment.in_home(".claude"), "~/.claude exists"),
            (
                environment.extension("anthropic.claude-code"),
                "its VSCode extension is installed",
            ),
        ]),
    );
    note(
        Agent::Copilot,
        first(&[
            (
                root.join(".vscode").is_dir(),
                "the project has a .vscode folder",
            ),
            (
                environment.on_path("code"),
                "VSCode (`code`) is on the PATH",
            ),
            (environment.in_home(".vscode"), "~/.vscode exists"),
        ]),
    );
    note(
        Agent::Cursor,
        first(&[
            (root.join(".cursor").is_dir(), "configured in the project"),
            (environment.in_home(".cursor"), "~/.cursor exists"),
        ]),
    );
    let codex_home = environment.codex_home.as_ref().is_some_and(|h| h.exists());
    note(
        Agent::Codex,
        first(&[
            (root.join(".codex").is_dir(), "configured in the project"),
            (environment.on_path("codex"), "`codex` is on the PATH"),
            (
                codex_home || environment.in_home(".codex"),
                "~/.codex exists",
            ),
        ]),
    );
    found
}

fn git(root: &Path) -> Git {
    let run = |args: &[&str]| -> Option<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let Some(hooks) = run(&["rev-parse", "--git-path", "hooks"]) else {
        return Git::None;
    };
    let hooks = PathBuf::from(hooks);
    Git::Repository {
        hooks: if hooks.is_absolute() {
            hooks
        } else {
            root.join(hooks)
        },
        hooks_path: run(&["config", "--get", "core.hooksPath"]).filter(|p| !p.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_languages_runners_and_agents() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"scripts": {"test": "vitest run"}}"#,
        )
        .unwrap();
        std::fs::write(root.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::write(root.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::create_dir(root.path().join(".cursor")).unwrap();
        std::fs::create_dir(home.path().join(".codex")).unwrap();
        std::fs::create_dir_all(
            home.path()
                .join(".vscode/extensions/anthropic.claude-code-2.1.301-linux-x64"),
        )
        .unwrap();
        std::fs::write(bin.path().join("code"), "").unwrap();
        let environment = Environment {
            home: Some(home.path().to_path_buf()),
            path: vec![bin.path().to_path_buf()],
            codex_home: None,
        };
        let found = detect(root.path(), &environment);
        assert_eq!(found.languages, ["rust", "typescript"]);
        assert_eq!(found.package_managers, ["cargo", "pnpm"]);
        let runners: Vec<(&str, &str)> = found
            .runners
            .iter()
            .map(|r| (r.language.as_str(), r.rule.name.as_str()))
            .collect();
        assert_eq!(
            runners,
            [("rust", "cargo-nextest"), ("typescript", "vitest")]
        );
        assert!(
            found.runners[0]
                .conventions
                .iter()
                .any(|(name, _)| name == "instrumentation.md")
        );
        let agents: Vec<Agent> = found.agents.iter().map(|(a, _)| *a).collect();
        assert_eq!(agents, Agent::ALL);
        assert_eq!(found.agents[0].1, "its VSCode extension is installed");
        assert_eq!(found.agents[2].1, "configured in the project");
        // A temporary directory is no git repository.
        assert_eq!(found.git, Git::None);
    }

    #[test]
    fn nothing_found_is_nothing_configured() {
        let root = tempfile::tempdir().unwrap();
        let found = detect(root.path(), &Environment::default());
        assert!(found.languages.is_empty() && found.agents.is_empty());
    }
}
