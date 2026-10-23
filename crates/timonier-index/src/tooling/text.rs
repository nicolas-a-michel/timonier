//! What `timonier init` changes in a file, and `uninit` takes back (ADR 0033, 0077): always its
//! own part, delimited, the rest left as it is. Pure: text in, text out. The blocks and JSON
//! objects every edit uses; each ecosystem's manifest edits are in its module.

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
pub fn split_block<'t>(text: &'t str, begin: &str, end: &str) -> Option<(&'t str, &'t str)> {
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
pub fn line_start(text: &str, marker: &str) -> Option<usize> {
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
pub fn object(text: Option<&str>) -> Result<Json, String> {
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
pub fn written(text: Option<&str>, before: &Json, after: &Json) -> String {
    match text {
        Some(text) if before == after => text.to_owned(),
        _ => after.pretty(),
    }
}

/// A package of the SDK, as the pack names it.
#[derive(Clone, Copy, Debug)]
pub struct Sdk<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub features: &'a [String],
    pub default_features: bool,
    /// For npm: in `dependencies`, not `devDependencies` (Q78).
    pub production: bool,
}

/// A TOML string, quoted.
pub fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
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
