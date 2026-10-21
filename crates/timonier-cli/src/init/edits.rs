//! What `timonier init` changes in a file, and `uninit` takes back (ADR 0033): always its own
//! part, delimited, the rest left as it is. Pure: text in, text out.

use std::fmt::Write;

use super::ordered::Json;

/// The markers of the managed block in Markdown (`AGENTS.md`, `CLAUDE.md`).
pub const MD_BEGIN: &str = "<!-- timonier:begin -->";
pub const MD_END: &str = "<!-- timonier:end -->";
/// The markers of a block in TOML and shell files.
pub const HASH_BEGIN: &str = "# >>> timonier >>>";
pub const HASH_END: &str = "# <<< timonier <<<";

/// `text` with the block between `begin` and `end` holding `body`: replaced where it is, or added
/// at the end after a blank line; a file of its own when there is none.
pub fn with_block(text: Option<&str>, begin: &str, end: &str, body: &str) -> String {
    let block = format!("{begin}\n{}\n{end}\n", body.trim_end());
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return block;
    };
    if let Some((before, after)) = split_block(text, begin, end) {
        return format!("{before}{block}{after}");
    }
    let mut out = text.trim_end_matches('\n').to_owned();
    out.push_str("\n\n");
    out.push_str(&block);
    out
}

/// `text` without its block, as it was before [`with_block`] added it; `None` when nothing else
/// is left. The block not found, the text as it is.
pub fn without_block(text: &str, begin: &str, end: &str) -> Option<String> {
    let Some((before, after)) = split_block(text, begin, end) else {
        return Some(text.to_owned());
    };
    let rest = if before.is_empty() {
        // At the top: the blank line that parted it from what follows goes with it.
        after.trim_start_matches('\n').to_owned()
    } else if after.is_empty() {
        // Added at the end, after a blank line.
        before
            .strip_suffix("\n\n")
            .map_or(before.to_owned(), |b| format!("{b}\n"))
    } else {
        format!("{before}{after}")
    };
    (!rest.trim().is_empty()).then_some(rest)
}

/// What comes before the block's first line and after its last line (and its newline).
fn split_block<'t>(text: &'t str, begin: &str, end: &str) -> Option<(&'t str, &'t str)> {
    let start = line_start(text, begin)?;
    let stop = text[start..].find(end)? + start + end.len();
    let stop = if text[stop..].starts_with('\n') {
        stop + 1
    } else {
        stop
    };
    Some((&text[..start], &text[stop..]))
}

/// Where the line that is exactly `marker` starts.
fn line_start(text: &str, marker: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == marker {
            return Some(at);
        }
        at += line.len();
    }
    None
}

/// A JSON object read from `text`: empty when there is none. An error says why it cannot be
/// read (JSON with comments, not an object).
fn object(text: Option<&str>) -> Result<Json, String> {
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return Ok(Json::Object(Vec::new()));
    };
    match Json::parse(text) {
        Ok(json @ Json::Object(_)) => Ok(json),
        Ok(_) => Err("it is no JSON object".to_owned()),
        Err(error) => Err(format!("it is not plain JSON ({error})")),
    }
}

/// The file's text, unchanged when its value did not change: the developer's layout stays.
fn written(text: Option<&str>, before: &Json, after: &Json) -> String {
    match text {
        Some(text) if before == after => text.to_owned(),
        _ => after.pretty(),
    }
}

/// `text` with `entry` under `section` (`mcpServers`, `servers`) at `name`: added, or put back
/// as Timonier writes it.
pub fn json_with_entry(
    text: Option<&str>,
    section: &str,
    name: &str,
    entry: Json,
) -> Result<String, String> {
    let before = object(text)?;
    let mut after = before.clone();
    let servers = after
        .entry(section, || Json::Object(Vec::new()))
        .ok_or_else(|| "it is no JSON object".to_owned())?;
    if !matches!(servers, Json::Object(_)) {
        return Err(format!("its `{section}` is no object"));
    }
    if servers.get(name).map(Json::value) != Some(entry.value()) {
        servers.insert(name, entry);
    }
    Ok(written(text, &before, &after))
}

/// `text` without `name` under `section`, the section gone when empty; `None` when nothing is
/// left.
pub fn json_without_entry(text: &str, section: &str, name: &str) -> Result<Option<String>, String> {
    let before = object(Some(text))?;
    let mut after = before.clone();
    if let Some(servers @ Json::Object(_)) = after.entry(section, || Json::Null) {
        servers.remove(name);
        if servers.is_empty_object() {
            after.remove(section);
        }
    } else {
        after.remove(section);
        if before.get(section).is_some() {
            after = before.clone();
        }
    }
    Ok((!after.is_empty_object()).then(|| written(Some(text), &before, &after)))
}

/// The shapes of a stop hook: Claude Code and Codex nest the commands in a matcher group; the
/// VSCode file lists them directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookShape {
    Grouped,
    Flat,
}

/// As the agents' documentations write them: the type, then the command.
fn hook_entry(command: &str, shape: HookShape) -> Json {
    let hook = Json::Object(vec![
        ("type".to_owned(), Json::String("command".to_owned())),
        ("command".to_owned(), Json::String(command.to_owned())),
    ]);
    match shape {
        HookShape::Grouped => Json::Object(vec![("hooks".to_owned(), Json::Array(vec![hook]))]),
        HookShape::Flat => hook,
    }
}

fn holds(entry: &Json, command: &str) -> bool {
    let entry = entry.value();
    entry["command"] == command
        || entry["hooks"]
            .as_array()
            .is_some_and(|hooks| hooks.iter().any(|h| h["command"] == command))
}

/// `text` with `command` among the hooks of `event`, once.
pub fn json_with_hook(
    text: Option<&str>,
    event: &str,
    command: &str,
    shape: HookShape,
) -> Result<String, String> {
    let before = object(text)?;
    let mut after = before.clone();
    let hooks = after
        .entry("hooks", || Json::Object(Vec::new()))
        .ok_or_else(|| "it is no JSON object".to_owned())?;
    let Some(entries) = hooks.entry(event, || Json::Array(Vec::new())) else {
        return Err("its `hooks` is no object".to_owned());
    };
    let Json::Array(entries) = entries else {
        return Err(format!("its `hooks.{event}` is no list"));
    };
    if !entries.iter().any(|e| holds(e, command)) {
        entries.push(hook_entry(command, shape));
    }
    Ok(written(text, &before, &after))
}

/// `text` without the hooks of `event` that run `command`; `None` when nothing is left.
pub fn json_without_hook(text: &str, event: &str, command: &str) -> Result<Option<String>, String> {
    let before = object(Some(text))?;
    let mut after = before.clone();
    if let Some(hooks @ Json::Object(_)) = after.entry("hooks", || Json::Null) {
        if let Some(Json::Array(entries)) = hooks.entry(event, || Json::Null) {
            entries.retain(|e| !holds(e, command));
        }
        if matches!(hooks.get(event), Some(Json::Array(e)) if e.is_empty())
            || matches!(hooks.get(event), Some(Json::Null))
        {
            hooks.remove(event);
        }
        if hooks.is_empty_object() {
            after.remove("hooks");
        }
    } else if before.get("hooks").is_none() {
        after.remove("hooks");
    }
    Ok((!after.is_empty_object()).then(|| written(Some(text), &before, &after)))
}

/// A `pre-commit` hook with Timonier's block right after its first line (the interpreter), so
/// that an `exec` or an `exit` further down does not skip it. A hook in another language than
/// the shell is refused: its lines would break it.
pub fn shell_with_block(text: Option<&str>, body: &str) -> Result<String, String> {
    let block = format!("{HASH_BEGIN}\n{}\n{HASH_END}\n", body.trim_end());
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return Ok(format!("#!/bin/sh\n{block}"));
    };
    if let Some((before, after)) = split_block(text, HASH_BEGIN, HASH_END) {
        return Ok(format!("{before}{block}{after}"));
    }
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    let shell = first.starts_with("#!")
        && [
            "/sh", "/bash", "/zsh", "/dash", "env sh", "env bash", "env zsh",
        ]
        .iter()
        .any(|s| first.contains(s));
    if !shell {
        return Err(format!("it is no shell script (`{first}`)"));
    }
    Ok(format!("{first}\n{block}{rest}"))
}

/// The markers of the two blocks of the OpenTelemetry SDK in a `Cargo.toml` (ADR 0050).
pub const FEATURE_BEGIN: &str = "# >>> timonier: feature >>>";
pub const FEATURE_END: &str = "# <<< timonier: feature <<<";
pub const DEPENDENCIES_BEGIN: &str = "# >>> timonier: dependencies >>>";
pub const DEPENDENCIES_END: &str = "# <<< timonier: dependencies <<<";

/// A package of the SDK, as the pack names it.
pub struct Sdk<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub features: &'a [String],
    pub default_features: bool,
    /// For npm: in `dependencies`, not `devDependencies` (Q78).
    pub production: bool,
}

/// A TOML string, quoted.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `Cargo.toml` with the SDK (ADR 0050): the packages the root package lacks as optional
/// dependencies, `[dependencies.<name>]` tables at the end, and `feature` turning them on, right
/// after `[features]` (or under a `[features]` of its own). `None` for a workspace without a
/// root package, whose crates take it as the conventions say; a feature of that name already
/// there is refused, with what to do.
pub fn cargo_with_sdk(
    text: &str,
    feature: &str,
    sdk: &[Sdk<'_>],
) -> Result<Option<String>, String> {
    let bare = cargo_without_sdk(text).unwrap_or_default();
    let manifest: toml::Table = bare
        .parse()
        .map_err(|e: toml::de::Error| format!("it does not read as TOML ({})", e.message()))?;
    if !manifest.contains_key("package") {
        return Ok(None);
    }
    let has = |table: &str, key: &str| {
        manifest
            .get(table)
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key(key))
    };
    if has("features", feature) {
        return Err(format!("a feature `{feature}` is already there"));
    }
    let added: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !has("dependencies", p.name))
        .collect();
    let mut dependencies = String::new();
    for package in &added {
        let _ = writeln!(dependencies, "[dependencies.{}]", package.name);
        let _ = writeln!(dependencies, "version = {}", quoted(package.version));
        dependencies.push_str("optional = true\n");
        if !package.default_features {
            dependencies.push_str("default-features = false\n");
        }
        if !package.features.is_empty() {
            let features: Vec<String> = package.features.iter().map(|f| quoted(f)).collect();
            let _ = writeln!(dependencies, "features = [{}]", features.join(", "));
        }
        dependencies.push('\n');
    }
    let turned_on: Vec<String> = added
        .iter()
        .map(|p| quoted(&format!("dep:{}", p.name)))
        .collect();
    let line = format!("{feature} = [{}]", turned_on.join(", "));
    let header = bare.lines().any(|l| l.trim() == "[features]");
    let mut out = if header {
        // Right after the developer's `[features]`.
        let mut out = String::new();
        let mut placed = false;
        for piece in bare.split_inclusive('\n') {
            out.push_str(piece);
            if !placed && piece.trim() == "[features]" {
                if !piece.ends_with('\n') {
                    out.push('\n');
                }
                let _ = write!(out, "{FEATURE_BEGIN}\n{line}\n{FEATURE_END}\n");
                placed = true;
            }
        }
        out
    } else {
        with_block(
            Some(&bare),
            FEATURE_BEGIN,
            FEATURE_END,
            &format!("[features]\n{line}"),
        )
    };
    if !added.is_empty() {
        out = with_block(
            Some(&out),
            DEPENDENCIES_BEGIN,
            DEPENDENCIES_END,
            &dependencies,
        );
    }
    out.parse::<toml::Table>()
        .map_err(|e| format!("the result would not read as TOML ({})", e.message()))?;
    Ok(Some(out))
}

/// `Cargo.toml` without the SDK's two blocks; `None` when nothing else is left.
pub fn cargo_without_sdk(text: &str) -> Option<String> {
    // The last added first: each block takes back the blank line it came with.
    let rest = without_block(text, DEPENDENCIES_BEGIN, DEPENDENCIES_END)?;
    without_block(&rest, FEATURE_BEGIN, FEATURE_END)
}

/// The names under `dependencies` and `devDependencies` of a `package.json`.
fn npm_dependencies(manifest: &Json) -> Vec<String> {
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|section| match manifest.get(section) {
            Some(Json::Object(entries)) => Some(entries.iter().map(|(name, _)| name.clone())),
            _ => None,
        })
        .flatten()
        .collect()
}

/// `package.json` with the SDK's packages it lacks as development dependencies (ADR 0050).
pub fn npm_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let manifest = object(Some(text))?;
    let present = npm_dependencies(&manifest);
    let mut out = text.to_owned();
    for package in sdk.iter().filter(|p| !present.iter().any(|n| n == p.name)) {
        out = json_with_entry(
            Some(&out),
            if package.production {
                "dependencies"
            } else {
                "devDependencies"
            },
            package.name,
            Json::String(package.version.to_owned()),
        )?;
    }
    Ok(out)
}

/// `package.json` without the SDK's packages that `original`, the file before `init`, did not
/// have.
pub fn npm_without_sdk(
    text: &str,
    original: &str,
    names: &[&str],
) -> Result<Option<String>, String> {
    let before = npm_dependencies(&object(Some(original))?);
    let mut out = Some(text.to_owned());
    for name in names.iter().filter(|n| !before.iter().any(|b| b == *n)) {
        for section in ["devDependencies", "dependencies"] {
            let Some(current) = &out else { break };
            out = json_without_entry(current, section, name)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn a_markdown_block_is_added_once_replaced_in_place_and_taken_back_exactly() {
        let mine = "# Mine\n\nMy rules.\n";
        let added = with_block(Some(mine), MD_BEGIN, MD_END, "Use Timonier.");
        assert_eq!(
            added,
            format!("{mine}\n{MD_BEGIN}\nUse Timonier.\n{MD_END}\n")
        );
        assert_eq!(
            with_block(Some(&added), MD_BEGIN, MD_END, "Use Timonier."),
            added
        );
        let changed = with_block(Some(&added), MD_BEGIN, MD_END, "Use Timonier, v2.");
        assert!(changed.contains("v2") && !changed.contains("Timonier.\n"));
        assert_eq!(
            without_block(&added, MD_BEGIN, MD_END).as_deref(),
            Some(mine)
        );
        // A block of its own file: nothing left.
        let alone = with_block(None, MD_BEGIN, MD_END, "Use Timonier.");
        assert_eq!(without_block(&alone, MD_BEGIN, MD_END), None);
        // Moved into the middle by the developer: taken from there.
        let middle = format!("{MD_BEGIN}\nx\n{MD_END}\n# After\n");
        assert_eq!(
            without_block(&middle, MD_BEGIN, MD_END).as_deref(),
            Some("# After\n")
        );
    }

    #[test]
    fn a_json_entry_joins_the_others_and_goes_alone() {
        let theirs =
            "{\n  \"mcpServers\": {\n    \"other\": {\n      \"command\": \"o\"\n    }\n  }\n}\n";
        let entry = Json::from_value(&json!({"command": "timonier", "args": ["mcp"]}));
        let added = json_with_entry(Some(theirs), "mcpServers", "timonier", entry.clone()).unwrap();
        let read: Value = serde_json::from_str(&added).unwrap();
        assert_eq!(read["mcpServers"]["timonier"], entry.value());
        assert_eq!(read["mcpServers"]["other"]["command"], "o");
        // Once.
        assert_eq!(
            json_with_entry(Some(&added), "mcpServers", "timonier", entry.clone()).unwrap(),
            added
        );
        assert_eq!(
            json_without_entry(&added, "mcpServers", "timonier")
                .unwrap()
                .as_deref(),
            Some(theirs)
        );
        let alone = json_with_entry(None, "servers", "timonier", entry).unwrap();
        assert_eq!(
            json_without_entry(&alone, "servers", "timonier").unwrap(),
            None
        );
        // With comments: left alone, and said why.
        assert!(
            json_with_entry(
                Some("{ // mine\n}"),
                "servers",
                "timonier",
                Json::Object(Vec::new())
            )
            .is_err()
        );
    }

    #[test]
    fn a_stop_hook_joins_the_others_once() {
        let theirs = r#"{"permissions": {"allow": []}, "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine"}]}]}}"#;
        let added = json_with_hook(
            Some(theirs),
            "Stop",
            "timonier hook stop",
            HookShape::Grouped,
        )
        .unwrap();
        let read: Value = serde_json::from_str(&added).unwrap();
        assert_eq!(read["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(
            read["hooks"]["Stop"][1]["hooks"][0]["command"],
            "timonier hook stop"
        );
        assert_eq!(
            json_with_hook(
                Some(&added),
                "Stop",
                "timonier hook stop",
                HookShape::Grouped
            )
            .unwrap(),
            added
        );
        let back: Value = serde_json::from_str(
            &json_without_hook(&added, "Stop", "timonier hook stop")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(theirs).unwrap());
        let flat = json_with_hook(None, "Stop", "t", HookShape::Flat).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&flat).unwrap()["hooks"]["Stop"][0]["command"],
            "t"
        );
        assert_eq!(json_without_hook(&flat, "Stop", "t").unwrap(), None);
    }

    /// The developer's settings keep their order and layout: adding the hook only adds lines.
    #[test]
    fn a_hook_added_to_settings_only_adds_lines() {
        // This repository's settings, as they are without Timonier's hook.
        let file = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/settings.json"
        ))
        .unwrap();
        let theirs = json_without_hook(&file, "Stop", "timonier hook stop --agent claude")
            .unwrap()
            .unwrap();
        let added = json_with_hook(
            Some(&theirs),
            "Stop",
            "timonier hook stop --agent claude",
            HookShape::Grouped,
        )
        .unwrap();
        let diff = similar::TextDiff::from_lines(theirs.as_str(), added.as_str());
        let removed: Vec<String> = diff
            .iter_all_changes()
            .filter(|c| c.tag() == similar::ChangeTag::Delete)
            .map(|c| c.to_string())
            .collect();
        // At most the closing line before the new entry gains a comma.
        assert!(removed.len() <= 1, "{removed:?}");
        let back = json_without_hook(&added, "Stop", "timonier hook stop --agent claude")
            .unwrap()
            .unwrap();
        assert_eq!(back, theirs);
    }

    #[test]
    fn the_git_hook_block_comes_first_and_only_in_shell_scripts() {
        let created = shell_with_block(None, "timonier hook pre-commit").unwrap();
        assert!(created.starts_with("#!/bin/sh\n# >>> timonier >>>\n"));
        let theirs = "#!/usr/bin/env bash\nnpm run lint\nexec something\n";
        let added = shell_with_block(Some(theirs), "timonier hook pre-commit").unwrap();
        assert!(added.starts_with("#!/usr/bin/env bash\n# >>> timonier >>>\n"));
        assert!(added.ends_with("npm run lint\nexec something\n"));
        assert_eq!(
            shell_with_block(Some(&added), "timonier hook pre-commit").unwrap(),
            added
        );
        assert_eq!(
            without_block(&added, HASH_BEGIN, HASH_END).as_deref(),
            Some(theirs)
        );
        assert!(shell_with_block(Some("#!/usr/bin/env python3\nprint()\n"), "x").is_err());
    }

    fn sdk() -> Vec<Sdk<'static>> {
        static FEATURES: [String; 0] = [];
        vec![
            Sdk {
                name: "tracing",
                version: "0.1.44",
                features: &FEATURES,
                default_features: true,
                production: true,
            },
            Sdk {
                name: "opentelemetry",
                version: "0.33.1",
                features: &FEATURES,
                default_features: false,
                production: false,
            },
        ]
    }

    #[test]
    fn the_sdk_goes_into_cargo_toml_and_comes_out_as_it_was() {
        let plain = "[package]\nname = \"demo\"\n\n[dependencies]\ntracing = \"0.1\"\n";
        let with = cargo_with_sdk(plain, "otel", &sdk()).unwrap().unwrap();
        let manifest: toml::Table = with.parse().unwrap();
        assert_eq!(
            manifest["features"]["otel"].as_array().unwrap(),
            &vec![toml::Value::from("dep:opentelemetry")],
            "the developer's tracing stays theirs: {with}"
        );
        let otel = &manifest["dependencies"]["opentelemetry"];
        assert_eq!(otel["optional"].as_bool(), Some(true));
        assert_eq!(otel["default-features"].as_bool(), Some(false));
        assert_eq!(manifest["dependencies"]["tracing"].as_str(), Some("0.1"));
        // Again: the same; then taken back, as it was.
        assert_eq!(
            cargo_with_sdk(&with, "otel", &sdk()).unwrap(),
            Some(with.clone())
        );
        assert_eq!(cargo_without_sdk(&with).as_deref(), Some(plain));

        // A `[features]` of the developer's: the feature goes right under it.
        let featured = "[package]\nname = \"demo\"\n\n[features]\nfast = []\n";
        let with = cargo_with_sdk(featured, "otel", &sdk()).unwrap().unwrap();
        assert!(
            with.contains("[features]\n# >>> timonier: feature >>>\notel = ["),
            "{with}"
        );
        let manifest: toml::Table = with.parse().unwrap();
        assert!(manifest["features"].get("fast").is_some());
        assert_eq!(cargo_without_sdk(&with).as_deref(), Some(featured));
    }

    #[test]
    fn a_cargo_toml_the_sdk_cannot_go_into_says_why() {
        let workspace = "[workspace]\nmembers = [\"a\"]\n";
        assert_eq!(cargo_with_sdk(workspace, "otel", &sdk()), Ok(None));
        let taken = "[package]\nname = \"d\"\n[features]\notel = []\n";
        assert!(
            cargo_with_sdk(taken, "otel", &sdk())
                .unwrap_err()
                .contains("already")
        );
        assert!(
            cargo_with_sdk("[package", "otel", &sdk())
                .unwrap_err()
                .contains("TOML")
        );
    }

    #[test]
    fn the_sdk_goes_into_package_json_and_only_it_comes_out() {
        let original =
            "{\n  \"name\": \"demo\",\n  \"devDependencies\": {\n    \"tracing\": \"1\"\n  }\n}\n";
        let with = npm_with_sdk(original, &sdk()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(value["devDependencies"]["opentelemetry"], "0.33.1");
        assert_eq!(
            value["dependencies"],
            serde_json::Value::Null,
            "tracing was there already"
        );
        assert_eq!(
            value["devDependencies"]["tracing"], "1",
            "the developer's stays"
        );
        let back = npm_without_sdk(&with, original, &["tracing", "opentelemetry"])
            .unwrap()
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&back).unwrap();
        assert_eq!(
            value,
            serde_json::from_str::<serde_json::Value>(original).unwrap()
        );
        // Q78: the one the shipped code imports goes into `dependencies`, and comes out of it.
        let bare = "{\n  \"name\": \"demo\"\n}\n";
        let with = npm_with_sdk(bare, &sdk()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(value["dependencies"]["tracing"], "0.1.44");
        assert_eq!(value["devDependencies"]["opentelemetry"], "0.33.1");
        let back = npm_without_sdk(&with, bare, &["tracing", "opentelemetry"])
            .unwrap()
            .unwrap();
        assert_eq!(back, bare);
    }
}
