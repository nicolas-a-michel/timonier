//! `timonier scip` (ADR 0073): each language's SCIP indexer, whether it is found, and what came
//! of the last commit's index; `--install` installs the missing ones with their packs'
//! commands, which the click of « Install the SCIP indexers » approves (Q94); `--now` asks the
//! daemon to index the last commit at once.

use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use timonier_core::protocol::methods;
use timonier_daemon::scip::{Status, usable};

use crate::daemon;

#[derive(Debug, Args)]
pub struct ScipArgs {
    /// Install the indexers that are missing, with the commands their packs give
    #[arg(long)]
    install: bool,
    /// The commands are approved: run them without asking
    #[arg(long, requires = "install")]
    approve: bool,
    /// Ask the daemon to index the last commit now
    #[arg(long)]
    now: bool,
    /// Say it as JSON: what the extension shows before the click
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    target: daemon::Target,
}

/// One language of the project and its indexer.
#[derive(Debug, Serialize)]
struct Line {
    language: String,
    indexer: String,
    found: bool,
    /// The command that installs it, when it is missing and its pack has one.
    install: Option<String>,
    /// What to do by hand, when it is missing and no command installs it.
    note: Option<String>,
    /// The last index: `applied`, `missing`, `incomplete`…
    state: Option<String>,
    detail: Option<String>,
    seconds: Option<f64>,
    confirmed: Option<f64>,
    /// Where its installer puts it, under the home: made before it installs.
    #[serde(skip)]
    bin_dirs: Vec<String>,
}

/// The user's folder, as the packs' install commands name it (`{home}`).
fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// An install command with `{home}` replaced; none when it names a home this machine lacks.
fn install_command(command: &str) -> Option<String> {
    if !command.contains("{home}") {
        return Some(command.to_owned());
    }
    let home = home()?.to_string_lossy().replace('\\', "/");
    Some(command.replace("{home}", &home))
}

pub fn run(args: &ScipArgs) -> ExitCode {
    match scip(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn scip(args: &ScipArgs) -> Result<bool> {
    let paths = daemon::resolve(&args.target)?;
    let lines = lines(&paths.root)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&lines)?);
        return Ok(true);
    }
    if lines.is_empty() {
        println!(
            "No language of this project has a SCIP indexer: its map stays tree-sitter's and the language servers'."
        );
        return Ok(true);
    }
    for line in &lines {
        println!("{}", words(line));
    }
    let mut ok = true;
    if args.install {
        let missing: Vec<&Line> = lines.iter().filter(|l| !l.found).collect();
        // Once each: two languages of one indexer give the same command (review of Phase 11).
        let mut commands: Vec<&str> = Vec::new();
        for command in missing.iter().filter_map(|l| l.install.as_deref()) {
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
        for line in missing.iter().filter(|l| l.install.is_none()) {
            println!(
                "{}: no command installs {}; {}",
                line.language,
                line.indexer,
                line.note.as_deref().unwrap_or("see its documentation")
            );
            ok = false;
        }
        if !commands.is_empty() && (args.approve || confirm(&commands)?) {
            // The folders the installers write into: some (coursier's) make none.
            if let Some(home) = home() {
                for dir in missing.iter().flat_map(|l| &l.bin_dirs) {
                    let _ = std::fs::create_dir_all(home.join(dir));
                }
            }
            for command in commands {
                println!("timonier: {command}");
                let mut words = crate::test::shell(command.to_owned());
                let status = std::process::Command::new(words.remove(0))
                    .args(&words)
                    .status()
                    .with_context(|| format!("cannot run `{command}`"))?;
                if !status.success() {
                    eprintln!("timonier: `{command}` failed ({status})");
                    ok = false;
                }
            }
        }
    }
    if args.now || args.install {
        daemon::launch(&paths)?;
        let mut client =
            daemon::connect_as(&paths, "timonier-scip").context("the daemon does not answer")?;
        let _: serde_json::Value = client.request(methods::SCIP_RUN, &serde_json::Value::Null)?;
        println!(
            "The daemon indexes the last commit now, one language at a time: `timonier scip` says how it went, `timonier doctor` too."
        );
    }
    Ok(ok)
}

/// The project's languages that have an indexer, and the last commit's status: the languages
/// the daemon indexed, else those `init` detects. Only the packs' manifests are read.
fn lines(root: &std::path::Path) -> Result<Vec<Line>> {
    let manifests = timonier_daemon::Packs::builtin_manifests()?;
    let status = Status::read(root);
    let detected = if status.languages.is_empty() {
        crate::init::detect::languages(root)
    } else {
        Vec::new()
    };
    let mut lines = Vec::new();
    for manifest in &manifests {
        let language = manifest.language.as_str();
        let Some(rule) = &manifest.scip else {
            continue;
        };
        let last = status.languages.get(language);
        if !detected.contains(&language) && last.is_none() {
            continue;
        }
        let found = usable(&rule.program).is_some();
        lines.push(Line {
            language: language.to_owned(),
            indexer: rule.name.clone(),
            found,
            install: (!found)
                .then(|| rule.install.as_deref().and_then(install_command))
                .flatten(),
            note: (!found).then(|| rule.note.clone()).flatten(),
            state: last
                .and_then(|l| serde_json::to_value(l.state).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            detail: last.and_then(|l| l.detail.clone()),
            seconds: last.and_then(|l| l.seconds),
            confirmed: last.and_then(|l| l.confirmed),
            bin_dirs: rule.bin_dirs.clone(),
        });
    }
    Ok(lines)
}

/// What `doctor` says of SCIP (ADR 0073): notes, never findings, since the map holds without
/// it. A missing indexer, a last index refused; an index applied as information.
pub(crate) fn doctor_notes(root: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let Ok(lines) = lines(root) else {
        return (Vec::new(), Vec::new());
    };
    let mut notes = Vec::new();
    let mut info = Vec::new();
    for line in &lines {
        let applied = line.state.as_deref() == Some("applied");
        let said = format!("SCIP, {}", words(line));
        if line.found
            && (applied || line.state.is_none() || line.state.as_deref() == Some("running"))
        {
            info.push(said);
        } else {
            notes.push(said);
        }
    }
    (notes, info)
}

/// One language in a line, as a terminal shows it.
fn words(line: &Line) -> String {
    let mut out = format!("{}: {}", line.language, line.indexer);
    if !line.found {
        out.push_str(" is missing");
        if let Some(install) = &line.install {
            out.push_str(&format!(" (`{install}`, or `timonier scip --install`)"));
        } else if let Some(note) = &line.note {
            out.push_str(&format!(" ({note})"));
        }
    }
    if let Some(state) = &line.state {
        out.push_str(&format!("; last index: {}", state.replace('_', " ")));
        if let Some(seconds) = line.seconds {
            out.push_str(&format!(", {seconds:.1} s"));
        }
        if let Some(confirmed) = line.confirmed {
            out.push_str(&format!(
                ", {:.0} % of tree-sitter's links confirmed",
                confirmed * 100.0
            ));
        }
        if let Some(detail) = &line.detail {
            out.push_str(&format!(": {detail}"));
        }
    }
    out
}

fn confirm(commands: &[&str]) -> Result<bool> {
    use std::io::{BufRead, IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        println!("Run `timonier scip --install --approve` to install them.");
        return Ok(false);
    }
    println!("To install the missing indexers, Timonier runs:");
    for command in commands {
        println!("  {command}");
    }
    print!("Run them? [Y/n] ");
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "" | "y" | "yes" | "o" | "oui"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(found: bool, state: Option<&str>) -> Line {
        Line {
            language: "demo".to_owned(),
            indexer: "scip-demo".to_owned(),
            found,
            install: (!found).then(|| "demo install".to_owned()),
            note: None,
            state: state.map(str::to_owned),
            detail: None,
            seconds: state.map(|_| 2.5),
            confirmed: (state == Some("applied")).then_some(0.91),
            bin_dirs: Vec::new(),
        }
    }

    #[test]
    fn a_line_says_what_is_missing_and_how_the_last_index_went() {
        assert_eq!(
            words(&line(false, None)),
            "demo: scip-demo is missing (`demo install`, or `timonier scip --install`)"
        );
        assert_eq!(
            words(&line(true, Some("applied"))),
            "demo: scip-demo; last index: applied, 2.5 s, 91 % of tree-sitter's links confirmed"
        );
        assert_eq!(
            words(&line(true, Some("not_ready"))),
            "demo: scip-demo; last index: not ready, 2.5 s"
        );
    }

    #[test]
    fn an_install_command_names_the_user_s_folder() {
        assert_eq!(install_command("npm i -g x").as_deref(), Some("npm i -g x"));
        let home = home().expect("a home in the tests");
        assert_eq!(
            install_command("cs bootstrap x -o {home}/.local/bin/x"),
            Some(format!(
                "cs bootstrap x -o {}/.local/bin/x",
                home.to_string_lossy().replace('\\', "/")
            ))
        );
    }

    /// The languages of the project, by its manifests or by the last commit's statuses, with
    /// what the daemon wrote of their indexes; `doctor` says a refused index as a note.
    #[test]
    fn the_project_s_languages_and_their_last_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(lines(root).unwrap().is_empty(), "no language");
        let manifests = timonier_daemon::Packs::builtin_manifests().unwrap();
        let (language, indexer) = manifests
            .iter()
            .find_map(|m| Some((m.language.clone(), m.scip.as_ref()?.name.clone())))
            .expect("a language with an indexer");
        let status = serde_json::json!({
            "sha": "c0ffee",
            "languages": {language.clone(): {
                "indexer": indexer, "state": "incomplete", "seconds": 1.0, "confirmed": 0.4,
                "detail": "the build failed", "at_ms": 1}},
        });
        std::fs::create_dir_all(root.join(".timonier/scip")).unwrap();
        std::fs::write(Status::path(root), status.to_string()).unwrap();
        let lines = lines(root).unwrap();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].language, language);
        assert_eq!(lines[0].state.as_deref(), Some("incomplete"));
        assert_eq!(lines[0].confirmed, Some(0.4));
        let (notes, info) = doctor_notes(root);
        assert!(info.is_empty(), "{info:?}");
        assert!(
            notes[0].starts_with(&format!("SCIP, {language}: {indexer}"))
                && notes[0].ends_with("40 % of tree-sitter's links confirmed: the build failed"),
            "{notes:?}"
        );
    }
}
