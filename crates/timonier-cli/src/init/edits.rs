//! What `timonier init` changes in a file, and `uninit` takes back (ADR 0033): always its own
//! part, delimited, the rest left as it is. Pure: text in, text out. The blocks, the JSON
//! entries and the manifests' edits are `timonier-index`'s (ADR 0077); the agents' hooks and
//! the git hook are here.

use timonier_daemon::tooling::ordered::Json;
pub use timonier_daemon::tooling::text::{
    HASH_BEGIN, HASH_END, MD_BEGIN, MD_END, json_with_entry, json_without_entry, with_block,
    without_block,
};
use timonier_daemon::tooling::text::{object, split_block, written};

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
}
