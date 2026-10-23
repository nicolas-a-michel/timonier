//! Each language in one place (ADR 0077, PLAN.md 11.7): the core, the daemon and the CLI name
//! no language, no ecosystem and none of their manifests; what they need of one comes from the
//! packs and from `timonier_index::tooling`. Checked on the string literals of their code, out
//! of the tests, as `App.test.tsx` checks the design tokens.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use timonier_index::Packs;

/// Words a literal may hold although a pack is named so: too common to be a language.
const COMMON: [&str; 2] = ["c", "go"];

/// The manifests, lockfiles and tools of the ecosystems, which only their modules read.
const FILES: [&str; 23] = [
    "cargo.toml",
    "cargo.lock",
    "package.json",
    "package-lock.json",
    "pyproject.toml",
    "uv.lock",
    "go.mod",
    "go.sum",
    "pom.xml",
    "build.gradle",
    "settings.gradle",
    "cmakelists.txt",
    "package.swift",
    "package.resolved",
    "composer.json",
    "composer.lock",
    "build.sbt",
    "vcpkg.json",
    "conanfile.txt",
    "requirements.txt",
    ".csproj",
    "node_modules",
    "pyvenv.cfg",
];

/// The names a literal of the core, the daemon or the CLI may not hold.
fn forbidden() -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for manifest in Packs::builtin_manifests().expect("the packs") {
        names.insert(manifest.language.clone());
        names.insert(manifest.ecosystem.as_str().to_owned());
        if let Some(tooling) = timonier_index::tooling::get(manifest.ecosystem.as_str()) {
            for packages in tooling.packages() {
                names.insert(packages.ecosystem.name().to_owned());
            }
        }
    }
    // The package managers the ecosystems detect.
    for manager in [
        "uv", "pip", "poetry", "pdm", "pnpm", "yarn", "bun", "gradle", "dotnet", "rustup",
        "nextest", "pytest",
    ] {
        names.insert(manager.to_owned());
    }
    for common in COMMON {
        names.remove(common);
    }
    names
}

/// The string literals of a file of Rust, out of its comments and of its `mod tests`, with
/// the line each starts on; a literal may span lines.
fn literals(text: &str) -> Vec<(usize, String)> {
    let code = &text[..tests_at(text)];
    let mut out = Vec::new();
    let mut line = 1;
    let mut chars = code.chars().peekable();
    let mut current: Option<(usize, String)> = None;
    while let Some(c) = chars.next() {
        if c == '\n' {
            line += 1;
        }
        match (&mut current, c) {
            (None, '/') if chars.peek() == Some(&'/') => {
                // A comment, to the end of its line.
                for next in chars.by_ref() {
                    if next == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            (None, '"') => current = Some((line, String::new())),
            (None, '\'') => {
                // A char literal (`'"'`) or a lifetime: skip what could close it.
                if chars.peek() == Some(&'\\') {
                    chars.next();
                }
                chars.next();
            }
            (Some(_), '"') => out.extend(current.take()),
            (Some((_, text)), '\\') => {
                if let Some(next) = chars.next() {
                    if next == '\n' {
                        line += 1;
                    }
                    text.push('\\');
                    text.push(next);
                }
            }
            (Some((_, text)), c) => text.push(c),
            (None, _) => {}
        }
    }
    out
}

/// Where the file's `mod tests` starts, with its `#[cfg(test)]` or `#[cfg(all(test, unix))]`;
/// the end of the file without one.
fn tests_at(text: &str) -> usize {
    let Some(module) = text.find("\nmod tests {") else {
        return text.len();
    };
    let before = &text[..module];
    match before.rfind('\n') {
        Some(line)
            if before[line + 1..].starts_with("#[cfg(") && before[line..].contains("test") =>
        {
            line + 1
        }
        _ => module + 1,
    }
}

/// Literals about Timonier itself, a Rust program, that name one of its crates: how it installs
/// (`cargo install … timonier-cli`), how its own files are made again.
fn about_timonier(literal: &str) -> bool {
    literal.contains("timonier-")
}

fn sources(krate: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(krate)
        .join("src");
    let mut out = Vec::new();
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("a source folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_core_the_daemon_and_the_cli_name_no_language() {
    let names = forbidden();
    assert!(names.contains("rust") && names.contains("npm") && names.contains("maven"));
    let mut found = Vec::new();
    for krate in ["timonier-core", "timonier-daemon", "timonier-cli"] {
        for path in sources(krate) {
            let text = std::fs::read_to_string(&path).expect("a source");
            for (line, literal) in literals(&text) {
                if about_timonier(&literal) {
                    continue;
                }
                let low = literal.to_lowercase();
                let words: BTreeSet<&str> = low
                    .split(|c: char| !(c.is_ascii_alphanumeric() || "_+#-".contains(c)))
                    .collect();
                let named: Vec<&String> = names
                    .iter()
                    .filter(|n| words.contains(n.as_str()))
                    .collect();
                let files: Vec<&str> = FILES.iter().copied().filter(|f| low.contains(f)).collect();
                if !named.is_empty() || !files.is_empty() {
                    found.push(format!(
                        "{}:{line}: {named:?} {files:?} in \"{literal}\"",
                        path.display()
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "what a language needs goes in its pack or its module of `tooling` (ADR 0077):\n{}",
        found.join("\n")
    );
}

#[test]
fn a_test_module_is_left_out_whatever_its_cfg() {
    for cfg in ["#[cfg(test)]", "#[cfg(all(test, unix))]"] {
        let text = format!("let a = \"rust\";\n{cfg}\nmod tests {{ \"cargo\" }}\n");
        let read: Vec<String> = literals(&text).into_iter().map(|(_, l)| l).collect();
        assert_eq!(read, ["rust"], "{cfg}");
    }
}

#[test]
fn the_literals_are_read_out_of_comments_and_tests() {
    let text = "let a = \"rust\"; // \"npm\"\n// \"maven\"\nlet q = '\"'; let b = \"x\\\"y\";\nlet c = \"one\n  cargo\";\n#[cfg(test)]\nmod tests { \"cargo\" }";
    let read: Vec<(usize, String)> = literals(text);
    assert_eq!(
        read,
        [
            (1, "rust".to_owned()),
            (3, "x\\\"y".to_owned()),
            (4, "one\n  cargo".to_owned())
        ]
    );
}
