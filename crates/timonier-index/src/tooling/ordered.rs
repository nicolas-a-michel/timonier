//! JSON as the developer wrote it (ADR 0033): keys keep their order, and the file is written the
//! way Prettier lays JSON out — objects one key per line, a list of plain values on one line
//! when it fits in 80 columns — so that adding Timonier's entry changes only that entry.

use std::fmt::Write;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

const WIDTH: usize = 80;

impl Json {
    pub fn parse(text: &str) -> Result<Json, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// The same value, unordered: for comparisons.
    pub fn value(&self) -> Value {
        match self {
            Json::Null => Value::Null,
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => Value::Number(n.clone()),
            Json::String(s) => Value::String(s.clone()),
            Json::Array(items) => Value::Array(items.iter().map(Json::value).collect()),
            Json::Object(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.value()))
                    .collect(),
            ),
        }
    }

    pub fn from_value(value: &Value) -> Json {
        match value {
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Number(n) => Json::Number(n.clone()),
            Value::String(s) => Json::String(s.clone()),
            Value::Array(items) => Json::Array(items.iter().map(Json::from_value).collect()),
            Value::Object(map) => Json::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Json::from_value(v)))
                    .collect(),
            ),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The entry `key`, made when missing (at the end), as `make` gives it.
    pub fn entry(&mut self, key: &str, make: impl FnOnce() -> Json) -> Option<&mut Json> {
        let Json::Object(entries) = self else {
            return None;
        };
        let at = match entries.iter().position(|(k, _)| k == key) {
            Some(at) => at,
            None => {
                entries.push((key.to_owned(), make()));
                entries.len() - 1
            }
        };
        Some(&mut entries[at].1)
    }

    /// Sets `key`: in place when it is there, else at the end.
    pub fn insert(&mut self, key: &str, value: Json) {
        if let Some(slot) = self.entry(key, || Json::Null) {
            *slot = value;
        }
    }

    pub fn remove(&mut self, key: &str) {
        if let Json::Object(entries) = self {
            entries.retain(|(k, _)| k != key);
        }
    }

    pub fn is_empty_object(&self) -> bool {
        matches!(self, Json::Object(entries) if entries.is_empty())
    }

    /// The text of the file: two spaces, a final newline.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn scalar(&self) -> Option<String> {
        match self {
            Json::Null => Some("null".to_owned()),
            Json::Bool(b) => Some(b.to_string()),
            Json::Number(n) => Some(n.to_string()),
            Json::String(s) => Some(Value::String(s.clone()).to_string()),
            Json::Array(_) | Json::Object(_) => None,
        }
    }

    fn write(&self, out: &mut String, indent: usize) {
        if let Some(scalar) = self.scalar() {
            out.push_str(&scalar);
            return;
        }
        let pad = "  ".repeat(indent + 1);
        let close = "  ".repeat(indent);
        match self {
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Array(items) => {
                // Plain values on one line, when the line holding them fits.
                let inline: Option<Vec<String>> = items.iter().map(Json::scalar).collect();
                if let Some(inline) = inline {
                    let line = format!("[{}]", inline.join(", "));
                    let column = out.rsplit('\n').next().map_or(0, str::len);
                    if column + line.len() < WIDTH {
                        out.push_str(&line);
                        return;
                    }
                }
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad);
                    item.write(out, indent + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&close);
                out.push(']');
            }
            Json::Object(entries) => {
                out.push_str("{\n");
                for (i, (key, value)) in entries.iter().enumerate() {
                    let _ = write!(out, "{pad}{}: ", Value::String(key.clone()));
                    value.write(out, indent + 1);
                    out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
                }
                out.push_str(&close);
                out.push('}');
            }
            _ => {}
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Reader;
        impl<'de> Visitor<'de> for Reader {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<Json, E> {
                Ok(Json::Bool(b))
            }
            fn visit_i64<E>(self, n: i64) -> Result<Json, E> {
                Ok(Json::Number(n.into()))
            }
            fn visit_u64<E>(self, n: u64) -> Result<Json, E> {
                Ok(Json::Number(n.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, n: f64) -> Result<Json, E> {
                serde_json::Number::from_f64(n)
                    .map(Json::Number)
                    .ok_or_else(|| E::custom("a number JSON cannot hold"))
            }
            fn visit_str<E>(self, s: &str) -> Result<Json, E> {
                Ok(Json::String(s.to_owned()))
            }
            fn visit_string<E>(self, s: String) -> Result<Json, E> {
                Ok(Json::String(s))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Json::Array(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
                let mut entries: Vec<(String, Json)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Json>()? {
                    entries.retain(|(k, _)| *k != key);
                    entries.push((key, value));
                }
                Ok(Json::Object(entries))
            }
        }
        deserializer.deserialize_any(Reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_laid_out_as_prettier_does_comes_back_byte_for_byte() {
        let text = r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "deny": ["Edit(**/.timonier/intent.json)"]
  },
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "node",
            "args": ["${CLAUDE_PROJECT_DIR}/.claude/hooks/session-start.mjs"],
            "timeout": 10
          }
        ]
      }
    ]
  },
  "empty": [],
  "long": [
    "a long value that does not fit on one line with the others",
    "another long value"
  ]
}
"#;
        assert_eq!(Json::parse(text).unwrap().pretty(), text);
    }

    #[test]
    fn an_entry_goes_at_the_end_and_others_keep_their_place() {
        let mut json = Json::parse(r#"{"b": 1, "a": {"x": true}}"#).unwrap();
        json.insert("c", Json::Null);
        json.insert("b", Json::Number(2.into()));
        json.remove("a");
        assert_eq!(json.pretty(), "{\n  \"b\": 2,\n  \"c\": null\n}\n");
    }
}
