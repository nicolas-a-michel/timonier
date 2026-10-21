//! Claude Code's telemetry, turned on for the AI cost (ADR 0058, Q85). Claude Code reads it only
//! from the user's settings, so `init` writes there, outside the project: four variables in the
//! `env` of `~/.claude/settings.json`, and the project in the machine's list. Nothing of it goes
//! into `.timonier/init.json`, which is versioned.

use std::path::{Path, PathBuf};

use super::edits;
use super::ordered::Json;

/// What Claude Code needs to send its requests to the agents' port.
pub fn variables(port: u16) -> [(&'static str, String); 4] {
    [
        ("CLAUDE_CODE_ENABLE_TELEMETRY", "1".to_owned()),
        ("OTEL_LOGS_EXPORTER", "otlp".to_owned()),
        (
            "OTEL_EXPORTER_OTLP_LOGS_PROTOCOL",
            "http/protobuf".to_owned(),
        ),
        (
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            format!("http://127.0.0.1:{port}/v1/logs"),
        ),
    ]
}

pub fn settings(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

/// What `init` would do to the user's settings.
#[derive(Debug, PartialEq, Eq)]
pub enum Planned {
    /// They say it already.
    Done,
    /// The file, before (absent: none) and after.
    Write {
        before: Option<String>,
        after: String,
    },
    /// The developer set one of the variables otherwise, or the file does not read: left as it
    /// is, and said.
    Left(String),
}

pub fn plan(home: &Path, port: u16) -> Planned {
    let path = settings(home);
    let before = std::fs::read_to_string(&path).ok();
    let env = before
        .as_deref()
        .and_then(|text| Json::parse(text).ok())
        .and_then(|json| json.get("env").map(Json::value));
    let mut text = before.clone();
    for (name, value) in variables(port) {
        match env.as_ref().and_then(|env| env.get(name)) {
            Some(serde_json::Value::String(set)) if *set == value => continue,
            Some(set) => {
                return Planned::Left(format!(
                    "{} sets {name} to {set}: Timonier leaves it, and Claude Code's tokens are not \
                     measured (ADR 0058).",
                    path.display()
                ));
            }
            None => {}
        }
        match edits::json_with_entry(text.as_deref(), "env", name, Json::String(value)) {
            Ok(after) => text = Some(after),
            Err(why) => {
                return Planned::Left(format!(
                    "{} cannot take Claude Code's telemetry: {why}.",
                    path.display()
                ));
            }
        }
    }
    match text {
        Some(after) if Some(&after) != before.as_ref() => Planned::Write { before, after },
        _ => Planned::Done,
    }
}

/// The user's settings without the four variables, when they hold Timonier's values: `None`
/// when nothing changes.
pub fn without(home: &Path, port: u16) -> Option<String> {
    let text = std::fs::read_to_string(settings(home)).ok()?;
    let ours = |name: &str, value: &str| {
        Json::parse(&text)
            .ok()
            .and_then(|json| json.get("env").map(Json::value))
            .and_then(|env| env.get(name).cloned())
            == Some(serde_json::Value::String(value.to_owned()))
    };
    let mut after = text.clone();
    for (name, value) in variables(port) {
        if ours(name, &value) {
            after = match edits::json_without_entry(&after, "env", name) {
                Ok(Some(after)) => after,
                Ok(None) => "{}\n".to_owned(),
                Err(_) => return None,
            };
        }
    }
    (after != text).then_some(after)
}

pub fn write(home: &Path, text: &str) -> std::io::Result<()> {
    let path = settings(home);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variables_go_in_beside_the_developer_s_and_come_out_alone() {
        let home = tempfile::tempdir().unwrap();
        let original = "{\n  \"env\": { \"FOO\": \"bar\" },\n  \"model\": \"opus\"\n}\n";
        write(home.path(), original).unwrap();
        let Planned::Write { before, after } = plan(home.path(), 4319) else {
            panic!("a change")
        };
        assert_eq!(before.as_deref(), Some(original));
        let json: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(json["env"]["FOO"], "bar");
        assert_eq!(json["model"], "opus");
        assert_eq!(json["env"]["CLAUDE_CODE_ENABLE_TELEMETRY"], "1");
        assert_eq!(
            json["env"]["OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"],
            "http://127.0.0.1:4319/v1/logs"
        );
        write(home.path(), &after).unwrap();
        assert_eq!(plan(home.path(), 4319), Planned::Done);
        let back: serde_json::Value =
            serde_json::from_str(&without(home.path(), 4319).unwrap()).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(original).unwrap()
        );
    }

    #[test]
    fn a_variable_the_developer_set_otherwise_is_left_and_said() {
        let home = tempfile::tempdir().unwrap();
        write(
            home.path(),
            "{ \"env\": { \"OTEL_LOGS_EXPORTER\": \"console\" } }\n",
        )
        .unwrap();
        let Planned::Left(why) = plan(home.path(), 4319) else {
            panic!("left")
        };
        assert!(why.contains("OTEL_LOGS_EXPORTER"), "{why}");
        // Nothing of Timonier's to take out.
        assert_eq!(without(home.path(), 4319), None);
    }

    #[test]
    fn no_settings_yet_makes_the_file() {
        let home = tempfile::tempdir().unwrap();
        let Planned::Write {
            before: None,
            after,
        } = plan(home.path(), 4319)
        else {
            panic!("a new file")
        };
        write(home.path(), &after).unwrap();
        assert_eq!(without(home.path(), 4319).as_deref(), Some("{}\n"));
    }
}
