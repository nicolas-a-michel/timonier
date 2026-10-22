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

    /// Whether `program` is in a folder of the `PATH`: as `program.exe` too on Windows.
    pub fn on_path(&self, program: &str) -> bool {
        self.path.iter().any(|dir| {
            dir.join(program).is_file()
                || (cfg!(windows) && dir.join(format!("{program}.exe")).is_file())
        })
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
    /// `rust`, `python`, `scala`, `php`, `csharp`, `swift`, `cpp`, `c`, `kotlin`, `java`, `go`, `typescript`, `javascript`.
    pub languages: Vec<&'static str>,
    /// `cargo`, `uv`, `poetry`, `pdm`, `pip`, `swiftpm`, `vcpkg`, `conan`, `cmake`, `maven`, `gradle`, `go`,
    /// `npm`, `pnpm`, `yarn`, `bun`.
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
    // Python (ADR 0062): a `pyproject.toml`, or a project without one but its requirements.
    if ["pyproject.toml", "requirements.txt", "setup.py"]
        .iter()
        .any(|f| root.join(f).is_file())
    {
        languages.push("python");
        package_managers.push(
            [
                ("uv.lock", "uv"),
                ("poetry.lock", "poetry"),
                ("pdm.lock", "pdm"),
            ]
            .into_iter()
            .find(|(lock, _)| root.join(lock).is_file())
            .map_or("pip", |(_, manager)| manager),
        );
    }
    // Scala (ADR 0071): an sbt build at the root.
    if root.join("build.sbt").is_file() {
        languages.push("scala");
        package_managers.push("sbt");
    }
    // PHP (ADR 0070): a Composer project at the root.
    if root.join("composer.json").is_file() {
        languages.push("php");
        package_managers.push("composer");
    }
    // C# (ADR 0069): a solution or a project at the root or one folder below.
    if dotnet_projects(root) {
        languages.push("csharp");
        package_managers.push("dotnet");
    }
    // Swift (ADR 0067): a package at the root.
    if root.join("Package.swift").is_file() {
        languages.push("swift");
        package_managers.push("swiftpm");
    }
    // C++ (ADR 0065) or C (ADR 0068): a CMake project at the root, with vcpkg or Conan when
    // they are there. One runner either way: the same CTest.
    if root.join("CMakeLists.txt").is_file() {
        languages.push(if cmake_is_c(root) { "c" } else { "cpp" });
        package_managers.push(if root.join("vcpkg.json").is_file() {
            "vcpkg"
        } else if ["conanfile.txt", "conanfile.py"]
            .iter()
            .any(|f| root.join(f).is_file())
        {
            "conan"
        } else {
            "cmake"
        });
    }
    // Java (ADR 0064): a Maven or a Gradle build at the root.
    let gradle = [
        "build.gradle.kts",
        "build.gradle",
        "settings.gradle.kts",
        "settings.gradle",
    ]
    .iter()
    .any(|f| root.join(f).is_file());
    // Kotlin (ADR 0066): a Gradle build that applies the Kotlin plugin, at the root, in a
    // project it includes, or in its version catalog. A build of both is Kotlin's.
    let kotlin = gradle && gradle_texts(root).iter().any(|t| t.contains("kotlin"));
    if kotlin {
        languages.push("kotlin");
        package_managers.push("gradle");
    } else if root.join("pom.xml").is_file() || gradle {
        languages.push("java");
        package_managers.push(if gradle { "gradle" } else { "maven" });
    }
    // Go (ADR 0063): a module at the root.
    if root.join("go.mod").is_file() {
        languages.push("go");
        package_managers.push("go");
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
    // Python again: a folder of scripts without a manifest, when nothing else was found. Its
    // runner is pytest, as for a project with only its requirements (no SDK without a
    // `pyproject.toml`).
    if languages.is_empty() && python_files(root) {
        languages.push("python");
        package_managers.push("pip");
    }
    Detection {
        runners: runners(root, &languages, test_script.as_deref()),
        languages,
        package_managers,
        agents: agents(root, environment),
        git: git(root),
    }
}

/// A Python file at the root.
fn python_files(root: &Path) -> bool {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry.path().extension().is_some_and(|e| e == "py")
                && entry.file_type().is_ok_and(|t| t.is_file())
        })
}

/// A .NET solution (`.sln`, `.slnx`) or project (`.csproj`) at the root, or a project one
/// folder below (`src/Shop/Shop.csproj` is two: also looked for).
pub(crate) fn dotnet_projects(root: &Path) -> bool {
    let has = |dir: &Path, extensions: &[&str]| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                extensions.iter().any(|x| name.ends_with(x))
            })
    };
    if has(root, &[".sln", ".slnx", ".csproj"]) {
        return true;
    }
    let below = |dir: &Path| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| e.path())
            .collect()
    };
    below(root)
        .iter()
        .flat_map(|d| std::iter::once(d.clone()).chain(below(d)))
        .any(|d| has(&d, &[".csproj"]))
}

/// A CMake project in C: its `project()` names C and not CXX, or, naming neither, it has `.c`
/// files and no C++ ones at the root and one folder below.
fn cmake_is_c(root: &Path) -> bool {
    let lists = std::fs::read_to_string(root.join("CMakeLists.txt")).unwrap_or_default();
    let commands = timonier_daemon::cmake_commands(&lists);
    if let Some((_, args)) = commands.iter().find(|(name, _)| name == "project") {
        let languages: Vec<&str> = args
            .iter()
            .skip_while(|a| *a != "LANGUAGES")
            .skip(1)
            .map(String::as_str)
            .collect();
        let named = |l: &str| languages.contains(&l) || args.iter().skip(1).any(|a| a == l);
        if named("CXX") {
            return false;
        }
        if named("C") {
            return true;
        }
    }
    let mut c = false;
    let mut dirs = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        dirs.extend(
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path()),
        );
    }
    for dir in dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if [".cpp", ".cc", ".cxx", ".hpp"]
                .iter()
                .any(|e| name.ends_with(e))
            {
                return false;
            }
            c |= name.ends_with(".c");
        }
    }
    c
}

/// The Gradle files that say which plugins a build applies: settings, root build, version
/// catalog, and the builds of the projects the settings include.
fn gradle_texts(root: &Path) -> Vec<String> {
    let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
    let mut texts: Vec<String> = [
        "settings.gradle.kts",
        "settings.gradle",
        "build.gradle.kts",
        "build.gradle",
        "gradle/libs.versions.toml",
    ]
    .iter()
    .filter_map(|p| read(p))
    .collect();
    let included: Vec<String> = texts
        .iter()
        .take(2)
        .flat_map(|s| s.lines())
        .filter_map(|l| l.trim().strip_prefix("include").map(str::to_owned))
        .flat_map(|rest| {
            rest.split(['"', '\''])
                .enumerate()
                .filter(|(i, p)| i % 2 == 1 && !p.is_empty())
                .map(|(_, p)| p.trim_start_matches(':').replace(':', "/"))
                .collect::<Vec<_>>()
        })
        .collect();
    for project in included {
        for build in ["build.gradle.kts", "build.gradle"] {
            if let Some(text) = read(&format!("{project}/{build}")) {
                texts.push(text);
            }
        }
    }
    texts
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
            let manifests: &[&str] = match pack.manifest.ecosystem.as_str() {
                "cargo" => &["Cargo.toml"],
                // Python was found by its manifest or, without one, by its files.
                "pypi" => return true,
                "gomod" => &["go.mod"],
                "cmake" => &["CMakeLists.txt"],
                "swiftpm" => &["Package.swift"],
                "composer" => &["composer.json"],
                "sbt" => &["build.sbt"],
                "nuget" if dotnet_projects(root) => return true,
                "maven" => &[
                    "pom.xml",
                    "build.gradle.kts",
                    "build.gradle",
                    "settings.gradle.kts",
                    "settings.gradle",
                ],
                _ => &["package.json"],
            };
            manifests.iter().any(|m| root.join(m).is_file())
        })
        .filter_map(|pack| {
            let rule = pack.manifest.tests.runners.iter().find(|rule| {
                let uses = match &rule.uses {
                    None => true,
                    Some(word) => test_script.is_some_and(|script| script.contains(word.as_str())),
                };
                uses && rule.when.as_ref().is_none_or(|file| {
                    let text = std::fs::read_to_string(root.join(file));
                    text.is_ok_and(|t| {
                        rule.when_text
                            .as_ref()
                            .is_none_or(|w| t.contains(w.as_str()))
                    })
                })
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

    /// A folder of Python scripts without a manifest is a Python project run by pytest; a
    /// script beside another language's manifest is not.
    #[test]
    fn python_files_without_a_manifest_are_a_python_project() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.py"), "print(1)\n").unwrap();
        let found = detect(root.path(), &Environment::default());
        assert_eq!(found.languages, ["python"]);
        assert_eq!(found.package_managers, ["pip"]);
        let runners: Vec<&str> = found.runners.iter().map(|r| r.rule.name.as_str()).collect();
        assert_eq!(runners, ["pytest"]);

        std::fs::write(root.path().join("go.mod"), "module example.com/x\n").unwrap();
        let found = detect(root.path(), &Environment::default());
        assert_eq!(found.languages, ["go"]);
    }
}
