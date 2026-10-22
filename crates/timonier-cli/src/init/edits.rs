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

/// What marks the lines `init` adds inside `pyproject.toml`'s own tables (ADR 0062).
const PYPROJECT_MARK: &str = "# timonier";

/// `pyproject.toml` with the SDK (ADR 0062): the production packages in `[project]`'s
/// `dependencies` (the code that ships imports them, Q78), the others in a PEP 735 group
/// `timonier`. Each line it adds ends with `# timonier`; `[dependency-groups]`, when it adds the
/// table, is a block between markers. Packages already declared are left. `Err` without a
/// `[project]` table (Poetry's alone): the API goes in by hand.
pub fn pyproject_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pyproject_without_sdk(text);
    let manifest: toml::Table = bare
        .parse()
        .map_err(|e: toml::de::Error| format!("it does not read as TOML ({})", e.message()))?;
    let project = manifest
        .get("project")
        .and_then(toml::Value::as_table)
        .ok_or("it has no [project] table: add opentelemetry-api to the dependencies by hand")?;
    let declared: Vec<String> = project
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            manifest
                .get("dependency-groups")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|groups| groups.values())
                .filter_map(toml::Value::as_array)
                .flatten(),
        )
        .filter_map(toml::Value::as_str)
        .map(|r| {
            r.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                .collect::<String>()
                .to_lowercase()
                .replace('_', "-")
        })
        .collect();
    let requirement = |p: &Sdk<'_>| quoted(&format!("{}=={}", p.name, p.version));
    let missing = |p: &&Sdk<'_>| !declared.contains(&p.name.to_lowercase());
    let production: Vec<String> = sdk
        .iter()
        .filter(|p| p.production)
        .filter(missing)
        .map(requirement)
        .collect();
    let group: Vec<String> = sdk
        .iter()
        .filter(|p| !p.production)
        .filter(missing)
        .map(requirement)
        .collect();
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let header = |lines: &[String], name: &str| lines.iter().position(|l| l.trim() == name);
    if !production.is_empty() {
        let at =
            header(&lines, "[project]").ok_or("its [project] table is not on a line of its own")?;
        let end = lines[at + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(lines.len(), |i| at + 1 + i);
        let key = (at + 1..end).find(|&i| {
            let line = lines[i].trim_start();
            line.starts_with("dependencies")
                && line["dependencies".len()..].trim_start().starts_with('=')
        });
        match key {
            Some(i) => {
                let Some(open) = lines[i].find('[') else {
                    return Err("its dependencies are not written as an array".to_owned());
                };
                let (head, tail) = lines[i].split_at(open + 1);
                let added: Vec<String> = production
                    .iter()
                    .map(|r| format!("    {r},  {PYPROJECT_MARK}"))
                    .collect();
                let rest = tail.trim_start();
                let mut replacement = vec![head.to_owned()];
                replacement.extend(added);
                if !rest.is_empty() {
                    replacement.push(format!("    {rest}"));
                }
                lines.splice(i..=i, replacement);
            }
            None => lines.insert(
                at + 1,
                format!(
                    "dependencies = [{}]  {PYPROJECT_MARK}",
                    production.join(", ")
                ),
            ),
        }
    }
    if !group.is_empty() {
        let entry = format!("timonier = [{}]  {PYPROJECT_MARK}", group.join(", "));
        match header(&lines, "[dependency-groups]") {
            Some(at) => lines.insert(at + 1, entry),
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.extend([
                    HASH_BEGIN.to_owned(),
                    "[dependency-groups]".to_owned(),
                    entry,
                    HASH_END.to_owned(),
                ]);
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// The markers of the block `init` adds to `go.mod` (ADR 0063).
pub const GO_BEGIN: &str = "// >>> timonier >>>";
pub const GO_END: &str = "// <<< timonier <<<";

/// `go.mod` with the SDK (ADR 0063): a `require` block between markers, at the end, with the
/// modules the file does not require yet. `go mod tidy` then fills `go.sum`. `Err` without a
/// `module` line.
pub fn go_mod_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    if !text
        .lines()
        .any(|l| l.split_whitespace().next() == Some("module"))
    {
        return Err("it has no `module` line".to_owned());
    }
    // Every module required already, in the block or not: the file as it is, since
    // `go mod tidy` sorts the block and adds its own lines to it.
    let required = go_requires(text);
    if sdk.iter().all(|p| required.iter().any(|r| r == p.name)) {
        return Ok(text.to_owned());
    }
    let bare = go_mod_without_sdk(text);
    let required = go_requires(&bare);
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !required.iter().any(|r| r == p.name))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut body = String::from("require (\n");
    for p in missing {
        let _ = writeln!(body, "\t{} {}", p.name, p.version);
    }
    body.push(')');
    Ok(with_block(Some(&bare), GO_BEGIN, GO_END, &body))
}

/// The modules a `go.mod` requires: the first word of a `require` line or of a line of its block.
fn go_requires(text: &str) -> Vec<String> {
    let mut required = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let code = line.split("//").next().unwrap_or("");
        let words: Vec<&str> = code.split_whitespace().collect();
        match words[..] {
            ["require", "("] => in_block = true,
            [")"] => in_block = false,
            ["require", name, ..] => required.push(name.trim_matches('"').to_owned()),
            [name, _, ..] if in_block => required.push(name.trim_matches('"').to_owned()),
            _ => {}
        }
    }
    required
}

/// `go.mod` without the block [`go_mod_with_sdk`] added; what `go mod tidy` put in it goes too,
/// and comes back at the next `go mod tidy`.
pub fn go_mod_without_sdk(text: &str) -> String {
    without_block(text, GO_BEGIN, GO_END).unwrap_or_default()
}

/// The markers of the block `init` adds inside `pom.xml` (ADR 0064).
pub const XML_BEGIN: &str = "<!-- >>> timonier >>> -->";
pub const XML_END: &str = "<!-- <<< timonier <<< -->";

/// `pom.xml` with the SDK (ADR 0064): the artifacts it does not declare yet, between markers,
/// at the top of the project's `<dependencies>` (a `<dependencies>` of their own before
/// `</project>` when it has none); the API at compile scope, the others at test scope.
pub fn pom_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pom_without_sdk(text);
    let document =
        roxmltree::Document::parse(&bare).map_err(|e| format!("it does not read as XML ({e})"))?;
    let project = document.root_element();
    let dependencies = project
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "dependencies");
    let text_of = |node: roxmltree::Node<'_, '_>, name: &str| {
        node.children()
            .find(|c| c.is_element() && c.tag_name().name() == name)
            .and_then(|c| c.text())
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let declared: Vec<String> = dependencies
        .into_iter()
        .flat_map(|d| d.children().filter(|c| c.tag_name().name() == "dependency"))
        .map(|d| format!("{}:{}", text_of(d, "groupId"), text_of(d, "artifactId")))
        .collect();
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !declared.iter().any(|d| d == p.name))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let indent_of = |line: &str| line[..line.len() - line.trim_start().len()].to_owned();
    let entries = |inner: &str, unit: &str| -> Vec<String> {
        let mut out = Vec::new();
        for p in &missing {
            let (group, artifact) = p.name.split_once(':').unwrap_or((p.name, p.name));
            out.push(format!("{inner}<dependency>"));
            out.push(format!("{inner}{unit}<groupId>{group}</groupId>"));
            out.push(format!("{inner}{unit}<artifactId>{artifact}</artifactId>"));
            out.push(format!("{inner}{unit}<version>{}</version>", p.version));
            if !p.production {
                out.push(format!("{inner}{unit}<scope>test</scope>"));
            }
            out.push(format!("{inner}</dependency>"));
        }
        out
    };
    let line_of = |offset: usize| bare[..offset].matches('\n').count();
    match dependencies {
        Some(node) => {
            let at = line_of(node.range().start);
            if lines[at].trim() != "<dependencies>" {
                return Err("its <dependencies> is not on a line of its own".to_owned());
            }
            let outer = indent_of(&lines[at]);
            let unit = if outer.is_empty() {
                "  ".to_owned()
            } else {
                outer.clone()
            };
            let inner = format!("{outer}{unit}");
            let mut block = vec![format!("{inner}{XML_BEGIN}")];
            block.extend(entries(&inner, &unit));
            block.push(format!("{inner}{XML_END}"));
            lines.splice(at + 1..at + 1, block);
        }
        None => {
            let at = lines
                .iter()
                .rposition(|l| l.trim() == "</project>")
                .ok_or("its </project> is not on a line of its own")?;
            let unit = lines
                .iter()
                .find(|l| l.starts_with([' ', '\t']))
                .map_or("  ".to_owned(), |l| indent_of(l));
            let inner = format!("{unit}{unit}");
            let mut block = vec![
                format!("{unit}{XML_BEGIN}"),
                format!("{unit}<dependencies>"),
            ];
            block.extend(entries(&inner, &unit));
            block.push(format!("{unit}</dependencies>"));
            block.push(format!("{unit}{XML_END}"));
            lines.splice(at..at, block);
        }
    }
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// A `.csproj` with the SDK (ADR 0069): an `<ItemGroup>` of its own between markers before
/// `</Project>`, with the packages the project does not reference yet. Taken back by
/// [`pom_without_sdk`], whose markers it shares.
pub fn csproj_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = pom_without_sdk(text);
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !bare.contains(&format!("Include=\"{}\"", p.name)))
        .collect();
    if missing.is_empty() {
        return Ok(bare);
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let at = lines
        .iter()
        .rposition(|l| l.trim() == "</Project>")
        .ok_or("its </Project> is not on a line of its own")?;
    let mut block = vec![format!("  {XML_BEGIN}"), "  <ItemGroup>".to_owned()];
    for p in missing {
        block.push(format!(
            "    <PackageReference Include=\"{}\" Version=\"{}\" />",
            p.name, p.version
        ));
    }
    block.push("  </ItemGroup>".to_owned());
    block.push(format!("  {XML_END}"));
    lines.splice(at..at, block);
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// `pom.xml` without the block [`pom_with_sdk`] added: the lines from its first marker to its
/// last.
pub fn pom_without_sdk(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.trim() == XML_BEGIN);
    let end = lines.iter().position(|l| l.trim() == XML_END);
    let (Some(begin), Some(end)) = (begin, end) else {
        return text.to_owned();
    };
    if end < begin {
        return text.to_owned();
    }
    let mut kept: Vec<&str> = lines[..begin].to_vec();
    kept.extend(&lines[end + 1..]);
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The markers of the block `init` adds to a Gradle build (ADR 0064).
pub const GRADLE_BEGIN: &str = "// >>> timonier >>>";
pub const GRADLE_END: &str = "// <<< timonier <<<";

/// A Gradle build (`build.gradle.kts` or `build.gradle`) with the SDK (ADR 0064): a
/// `dependencies` block of its own at the end, between markers, which Gradle adds to the
/// others; the API in `implementation`, the others in `testImplementation`. A module the build
/// names already is left.
pub fn gradle_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default();
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| {
            !(bare.contains(&format!("\"{}:", p.name)) || bare.contains(&format!("'{}:", p.name)))
        })
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let mut body = String::from("dependencies {\n");
    for p in missing {
        let configuration = if p.production {
            "implementation"
        } else {
            "testImplementation"
        };
        let _ = writeln!(body, "    {configuration}(\"{}:{}\")", p.name, p.version);
    }
    body.push('}');
    with_block(Some(&bare), GRADLE_BEGIN, GRADLE_END, &body)
}

/// `build.sbt` with the SDK (ADR 0071): a `libraryDependencies ++= Seq(…)` of its own at the
/// end, between the markers of Gradle's (same comments), the API for the code, the others
/// `% Test`; Maven coordinates, `group:artifact`, written `"group" % "artifact" % "version"`.
/// Taken back by [`gradle_without_sdk`].
pub fn sbt_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default();
    let missing: Vec<String> = sdk
        .iter()
        .filter_map(|p| {
            let (group, artifact) = p.name.split_once(':')?;
            if bare.contains(&format!("\"{artifact}\"")) {
                return None;
            }
            let test = if p.production { "" } else { " % Test" };
            Some(format!(
                "  \"{group}\" % \"{artifact}\" % \"{}\"{test},",
                p.version
            ))
        })
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let body = format!("libraryDependencies ++= Seq(\n{}\n)", missing.join("\n"));
    with_block(Some(&bare), GRADLE_BEGIN, GRADLE_END, &body)
}

/// A Gradle build without the block [`gradle_with_sdk`] added.
pub fn gradle_without_sdk(text: &str) -> String {
    without_block(text, GRADLE_BEGIN, GRADLE_END).unwrap_or_default()
}

/// `pyproject.toml` without what [`pyproject_with_sdk`] added: its marked lines, its block, and
/// the dependencies array it opened, put back on one line.
pub fn pyproject_without_sdk(text: &str) -> String {
    let rest = without_block(text, HASH_BEGIN, HASH_END).unwrap_or_default();
    let mut lines: Vec<&str> = Vec::new();
    let mut opened_by_us = false;
    for line in rest.lines() {
        if line.trim_end().ends_with(PYPROJECT_MARK) {
            // An element `init` put in an array the developer wrote: that array was split.
            opened_by_us |= line.trim_start().starts_with('"');
            continue;
        }
        lines.push(line);
    }
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        // `dependencies = [` followed by one moved-down line, as `init` split it: one line again.
        if opened_by_us
            && lines[i].trim_end().ends_with('[')
            && lines[i].trim_start().starts_with("dependencies")
            && i + 1 < lines.len()
            && lines[i + 1].starts_with("    ")
            && lines[i + 1].trim_end().ends_with(']')
        {
            out.push(format!("{}{}", lines[i], lines[i + 1].trim_start()));
            i += 2;
            continue;
        }
        out.push(lines[i].to_owned());
        i += 1;
    }
    let mut text = out.join("\n");
    if rest.ends_with('\n') {
        text.push('\n');
    }
    text
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

/// What marks the lines `init` adds to `Package.swift` (ADR 0067).
const SWIFT_MARK: &str = "// timonier";

/// `Package.swift` with the SDK (ADR 0067): each package it does not depend on yet, at the top
/// of the package's `dependencies` (an array of their own before `targets` when it has none),
/// and a test target `TimonierTests` that takes their products (`features`), at the top of
/// `targets`. Every line it adds ends with `// timonier`. `Err` when `targets` is not an array
/// opened on its own line.
pub fn swift_package_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let bare = swift_package_without_sdk(text);
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    let indent_of = |line: &str| line[..line.len() - line.trim_start().len()].to_owned();
    // The arguments of `Package(`: the indentation of its `name:`.
    let name = lines
        .iter()
        .position(|l| l.trim_start().starts_with("name:"))
        .ok_or("it has no `name:` line")?;
    let outer = indent_of(&lines[name]);
    let unit = if outer.is_empty() {
        "    ".to_owned()
    } else {
        outer.clone()
    };
    let inner = format!("{outer}{unit}");
    let at = |lines: &[String], key: &str| {
        lines
            .iter()
            .position(|l| indent_of(l) == outer && l.trim_start().starts_with(&format!("{key}:")))
    };
    let identity = |url: &str| {
        url.trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .to_owned()
    };
    let missing: Vec<&Sdk<'_>> = sdk
        .iter()
        .filter(|p| !bare.contains(p.name.trim_end_matches(".git")))
        .collect();
    let packages: Vec<String> = missing
        .iter()
        .map(|p| {
            format!(
                "{inner}.package(url: \"{}\", from: \"{}\"),  {SWIFT_MARK}",
                p.name, p.version
            )
        })
        .collect();
    if !packages.is_empty() {
        match at(&lines, "dependencies") {
            Some(i) if lines[i].trim_end().ends_with('[') => {
                lines.splice(i + 1..i + 1, packages);
            }
            Some(_) => {
                return Err("its dependencies are not an array opened on its own line".to_owned());
            }
            None => {
                let targets = at(&lines, "targets").ok_or("it has no `targets:` line")?;
                let mut block = vec![format!("{outer}dependencies: [  {SWIFT_MARK}")];
                block.extend(packages);
                block.push(format!("{outer}],  {SWIFT_MARK}"));
                lines.splice(targets..targets, block);
            }
        }
    }
    if !bare.contains("\"TimonierTests\"") {
        let targets = at(&lines, "targets")
            .filter(|&i| lines[i].trim_end().ends_with('['))
            .ok_or("its targets are not an array opened on its own line")?;
        let products: Vec<String> = sdk
            .iter()
            .flat_map(|p| {
                p.features.iter().map(move |product| {
                    format!(
                        ".product(name: \"{product}\", package: \"{}\")",
                        identity(p.name)
                    )
                })
            })
            .collect();
        lines.insert(
            targets + 1,
            format!(
                "{inner}.testTarget(name: \"TimonierTests\", dependencies: [{}]),  {SWIFT_MARK}",
                products.join(", ")
            ),
        );
    }
    let mut out = lines.join("\n");
    if bare.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// `Package.swift` without the lines [`swift_package_with_sdk`] added.
pub fn swift_package_without_sdk(text: &str) -> String {
    let mut out: String = text
        .lines()
        .filter(|l| !l.trim_end().ends_with(SWIFT_MARK))
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// `conanfile.txt` with the SDK (ADR 0065): the references it does not require yet
/// (`opentelemetry-cpp/1.26.0`), between markers at the top of `[requires]` (a `[requires]` of
/// their own at the end when it has none). Taken back by [`conanfile_without_sdk`].
pub fn conanfile_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> String {
    let bare = conanfile_without_sdk(text);
    let missing: Vec<String> = sdk
        .iter()
        .filter(|p| {
            !bare
                .lines()
                .any(|l| l.trim().starts_with(&format!("{}/", p.name)))
        })
        .map(|p| format!("{}/{}", p.name, p.version))
        .collect();
    if missing.is_empty() {
        return bare;
    }
    let mut lines: Vec<String> = bare.lines().map(str::to_owned).collect();
    match lines.iter().position(|l| l.trim() == "[requires]") {
        Some(at) => {
            let mut block = vec![HASH_BEGIN.to_owned()];
            block.extend(missing);
            block.push(HASH_END.to_owned());
            lines.splice(at + 1..at + 1, block);
        }
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(HASH_BEGIN.to_owned());
            lines.push("[requires]".to_owned());
            lines.extend(missing);
            lines.push(HASH_END.to_owned());
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// `conanfile.txt` without the block [`conanfile_with_sdk`] added.
pub fn conanfile_without_sdk(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.trim() == HASH_BEGIN);
    let end = lines.iter().position(|l| l.trim() == HASH_END);
    let (Some(begin), Some(end)) = (begin, end) else {
        return text.to_owned();
    };
    let mut kept: Vec<&str> = lines[..begin].to_vec();
    kept.extend(&lines[end + 1..]);
    // The `[requires]` block came after a blank line it adds.
    if begin == kept.len() && kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The block `init` adds to the root `CMakeLists.txt` (ADR 0065): the folder `timonier/`, and
/// CTest, which its smoke test needs.
pub fn cmake_with_sdk(text: &str) -> String {
    with_block(
        Some(text),
        HASH_BEGIN,
        HASH_END,
        "# Timonier's OpenTelemetry and its smoke test (timonier init), when opentelemetry-cpp is found.\nenable_testing()\nadd_subdirectory(timonier)",
    )
}

/// `vcpkg.json` with the SDK (ADR 0065): each port it does not depend on yet, with its
/// features (`opentelemetry-cpp[otlp-http]`).
pub fn vcpkg_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let mut manifest = object(Some(text))?;
    let named = |entry: &Json, name: &str| match entry {
        Json::String(s) => s == name,
        _ => matches!(entry.get("name"), Some(Json::String(s)) if s == name),
    };
    let Some(Json::Array(dependencies)) =
        manifest.entry("dependencies", || Json::Array(Vec::new()))
    else {
        return Err("its dependencies are not a list".to_owned());
    };
    let before = dependencies.len();
    for port in sdk {
        if dependencies.iter().any(|d| named(d, port.name)) {
            continue;
        }
        dependencies.push(if port.features.is_empty() {
            Json::String(port.name.to_owned())
        } else {
            Json::Object(vec![
                ("name".to_owned(), Json::String(port.name.to_owned())),
                (
                    "features".to_owned(),
                    Json::Array(
                        port.features
                            .iter()
                            .map(|f| Json::String(f.clone()))
                            .collect(),
                    ),
                ),
            ])
        });
    }
    if dependencies.len() == before {
        return Ok(text.to_owned());
    }
    Ok(manifest.pretty())
}

/// `vcpkg.json` without the SDK's ports that `original`, the file before `init`, did not have.
pub fn vcpkg_without_sdk(text: &str, original: &str, names: &[&str]) -> Result<String, String> {
    let named = |entry: &Json, name: &str| match entry {
        Json::String(s) => s == name,
        _ => matches!(entry.get("name"), Some(Json::String(s)) if s == name),
    };
    let had = |name: &str| {
        matches!(object(Some(original)).ok().and_then(|o| o.get("dependencies").cloned()),
            Some(Json::Array(list)) if list.iter().any(|d| named(d, name)))
    };
    let mut manifest = object(Some(text))?;
    let mut emptied = false;
    if let Some(Json::Array(dependencies)) =
        manifest.entry("dependencies", || Json::Array(Vec::new()))
    {
        dependencies.retain(|d| !names.iter().any(|n| named(d, n) && !had(n)));
        emptied = dependencies.is_empty();
    }
    // A list `init` made for its port alone goes with it.
    let made = object(Some(original)).is_ok_and(|o| o.get("dependencies").is_none());
    if emptied && made {
        manifest.remove("dependencies");
    }
    Ok(manifest.pretty())
}

/// `composer.json` with the SDK (ADR 0070): the production packages in `require`, the others in
/// `require-dev`, each with its constraint; a package declared already is left.
pub fn composer_with_sdk(text: &str, sdk: &[Sdk<'_>]) -> Result<String, String> {
    let manifest = object(Some(text))?;
    let declared = |name: &str| {
        ["require", "require-dev"]
            .iter()
            .any(|s| manifest.get(s).and_then(|m| m.get(name)).is_some())
    };
    let mut out = text.to_owned();
    for package in sdk.iter().filter(|p| !declared(p.name)) {
        out = json_with_entry(
            Some(&out),
            if package.production {
                "require"
            } else {
                "require-dev"
            },
            package.name,
            Json::String(package.version.to_owned()),
        )?;
    }
    Ok(out)
}

/// `composer.json` without the SDK's packages that `original`, the file before `init`, did
/// not have.
pub fn composer_without_sdk(
    text: &str,
    original: &str,
    names: &[&str],
) -> Result<Option<String>, String> {
    let before = object(Some(original))?;
    let had = |name: &str| {
        ["require", "require-dev"]
            .iter()
            .any(|s| before.get(s).and_then(|m| m.get(name)).is_some())
    };
    let mut out = Some(text.to_owned());
    for name in names.iter().filter(|n| !had(n)) {
        for section in ["require-dev", "require"] {
            let Some(current) = &out else { break };
            out = json_without_entry(current, section, name)?;
        }
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

    /// T-121: the SDK goes into `pyproject.toml` and comes out, the file as it was; a second
    /// `init` changes nothing; a package declared already is left.
    #[test]
    fn the_python_sdk_goes_in_and_out_of_pyproject() {
        let sdk = [
            Sdk {
                name: "opentelemetry-api",
                version: "1.45.1",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "opentelemetry-sdk",
                version: "1.45.1",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let one_line = "[project]\nname = \"shop\"\ndependencies = [\"fastapi>=0.115\"]\n";
        let many = "[project]\nname = \"shop\"\ndependencies = [\n    \"fastapi>=0.115\",\n]\n\n[dependency-groups]\ndev = [\"pytest\"]\n";
        let none = "[project]\nname = \"shop\"\nversion = \"1.0\"\n";
        for original in [one_line, many, none] {
            let with = pyproject_with_sdk(original, &sdk).unwrap();
            let parsed: toml::Table = with.parse().unwrap_or_else(|e| panic!("{e}\n{with}"));
            let dependencies = parsed["project"]["dependencies"].as_array().unwrap();
            assert!(
                dependencies
                    .iter()
                    .any(|d| d.as_str() == Some("opentelemetry-api==1.45.1")),
                "{with}"
            );
            assert_eq!(
                parsed["dependency-groups"]["timonier"].as_array().unwrap()[0].as_str(),
                Some("opentelemetry-sdk==1.45.1"),
                "{with}"
            );
            assert_eq!(pyproject_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(pyproject_without_sdk(&with), original, "{with}");
        }
        let declared = "[project]\nname = \"x\"\ndependencies = [\"opentelemetry-api>=1\"]\n";
        let with = pyproject_with_sdk(declared, &sdk).unwrap();
        assert!(!with.contains("opentelemetry-api==1.45.1"), "{with}");
        assert!(
            pyproject_with_sdk("[tool.poetry]\nname = \"x\"\n", &sdk)
                .unwrap_err()
                .contains("[project]")
        );
    }

    /// T-122: the SDK goes into `go.mod` as a marked `require` block and comes out, the file
    /// as it was; a second `init` changes nothing; a module required already is left.
    #[test]
    fn the_go_sdk_goes_in_and_out_of_go_mod() {
        let sdk = [
            Sdk {
                name: "go.opentelemetry.io/otel",
                version: "v1.47.0",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "go.opentelemetry.io/otel/sdk",
                version: "v1.47.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let bare = "module example.com/shop\n\ngo 1.27\n";
        let requiring = "module example.com/shop\n\ngo 1.27\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.11.0\n\tgo.opentelemetry.io/otel v1.40.0 // indirect\n)\n";
        for original in [bare, requiring] {
            let with = go_mod_with_sdk(original, &sdk).unwrap();
            assert!(
                with.contains("// >>> timonier >>>\nrequire (\n\t"),
                "{with}"
            );
            assert!(
                with.contains("\tgo.opentelemetry.io/otel/sdk v1.47.0\n"),
                "{with}"
            );
            assert_eq!(
                with.contains("\tgo.opentelemetry.io/otel v1.47.0"),
                original == bare,
                "{with}"
            );
            assert_eq!(go_mod_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(go_mod_without_sdk(&with), original, "{with}");
        }
        // Tidied: the block sorted, with lines of its own. Nothing to repair.
        let tidied = "module example.com/shop\n\n// >>> timonier >>>\nrequire (\n\tgo.opentelemetry.io/otel v1.47.0\n\tgo.opentelemetry.io/otel/sdk v1.47.0\n)\n\nrequire github.com/go-logr/logr v1.4.4 // indirect\n\n// <<< timonier <<<\n";
        assert_eq!(go_mod_with_sdk(tidied, &sdk).unwrap(), tidied);
        assert!(
            go_mod_with_sdk("go 1.27\n", &sdk)
                .unwrap_err()
                .contains("module")
        );
    }

    fn java_sdk() -> [Sdk<'static>; 2] {
        [
            Sdk {
                name: "io.opentelemetry:opentelemetry-api",
                version: "1.67.0",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "io.opentelemetry:opentelemetry-sdk",
                version: "1.67.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ]
    }

    /// T-123: the SDK goes into `pom.xml`, inside its `<dependencies>` or in one of its own,
    /// and comes out, the file as it was; a second `init` changes nothing; an artifact declared
    /// already is left.
    #[test]
    fn the_java_sdk_goes_in_and_out_of_pom_xml() {
        let sdk = java_sdk();
        let with_dependencies = "<project>\n    <artifactId>shop</artifactId>\n    <dependencies>\n        <dependency>\n            <groupId>io.opentelemetry</groupId>\n            <artifactId>opentelemetry-api</artifactId>\n        </dependency>\n    </dependencies>\n</project>\n";
        let without = "<project>\n  <artifactId>shop</artifactId>\n</project>\n";
        for original in [with_dependencies, without] {
            let with = pom_with_sdk(original, &sdk).unwrap();
            roxmltree::Document::parse(&with).unwrap_or_else(|e| panic!("{e}\n{with}"));
            assert!(
                with.contains("<artifactId>opentelemetry-sdk</artifactId>"),
                "{with}"
            );
            assert!(with.contains("<scope>test</scope>"), "{with}");
            assert_eq!(
                with.matches("<artifactId>opentelemetry-api</artifactId>")
                    .count(),
                1,
                "{with}"
            );
            assert_eq!(pom_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(pom_without_sdk(&with), original, "{with}");
        }
        assert!(
            with_dependencies.contains("    <dependencies>\n")
                && pom_with_sdk(with_dependencies, &sdk)
                    .unwrap()
                    .contains("    <dependencies>\n        <!-- >>> timonier >>> -->\n        <dependency>\n            <groupId>io.opentelemetry</groupId>")
        );
    }

    /// T-123: the SDK goes into a Gradle build as a `dependencies` block of its own and comes
    /// out; a module named already is left.
    #[test]
    fn the_java_sdk_goes_in_and_out_of_a_gradle_build() {
        let sdk = java_sdk();
        let original = "plugins {\n    java\n}\n\ndependencies {\n    implementation(\"io.opentelemetry:opentelemetry-api:1.60.0\")\n}\n";
        let with = gradle_with_sdk(original, &sdk);
        assert!(
            with.ends_with("// >>> timonier >>>\ndependencies {\n    testImplementation(\"io.opentelemetry:opentelemetry-sdk:1.67.0\")\n}\n// <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(gradle_with_sdk(&with, &sdk), with, "twice");
        assert_eq!(gradle_without_sdk(&with), original);
    }

    /// T-124: the root `CMakeLists.txt` takes the folder `timonier/` in a block, and gives it
    /// back; `vcpkg.json` takes opentelemetry-cpp with its feature, once, and gives it back.
    #[test]
    fn the_cpp_sdk_goes_in_and_out_of_cmake_and_vcpkg() {
        let lists = "cmake_minimum_required(VERSION 3.20)\nproject(shop CXX)\nadd_executable(shop main.cpp)\n";
        let with = cmake_with_sdk(lists);
        assert!(
            with.ends_with("enable_testing()\nadd_subdirectory(timonier)\n# <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(cmake_with_sdk(&with), with);
        assert_eq!(
            without_block(&with, HASH_BEGIN, HASH_END).as_deref(),
            Some(lists)
        );
        let features = ["otlp-http".to_owned()];
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.29.0",
            features: &features,
            default_features: true,
            production: false,
        }];
        let vcpkg = "{\n  \"name\": \"shop\",\n  \"dependencies\": [\"fmt\"]\n}\n";
        let with = vcpkg_with_sdk(vcpkg, &sdk).unwrap();
        let parsed: Value = serde_json::from_str(&with).unwrap();
        assert_eq!(
            parsed["dependencies"],
            json!(["fmt", {"name": "opentelemetry-cpp", "features": ["otlp-http"]}])
        );
        assert_eq!(vcpkg_with_sdk(&with, &sdk).unwrap(), with, "twice");
        let without = vcpkg_without_sdk(&with, vcpkg, &["opentelemetry-cpp"]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&without).unwrap(),
            serde_json::from_str::<Value>(vcpkg).unwrap()
        );
        // Declared before `init`: it stays.
        assert_eq!(
            vcpkg_without_sdk(&with, &with, &["opentelemetry-cpp"]).unwrap(),
            Json::parse(&with).unwrap().pretty()
        );
    }

    /// T-128: `Package.swift` takes the OpenTelemetry packages and the test target, and gives
    /// them back; without a `dependencies` array, one of its own; twice changes nothing.
    #[test]
    fn the_swift_sdk_goes_in_and_out_of_package_swift() {
        let core = ["OpenTelemetryApi".to_owned(), "OpenTelemetrySdk".to_owned()];
        let sdk = [Sdk {
            name: "https://github.com/open-telemetry/opentelemetry-swift-core.git",
            version: "2.6.0",
            features: &core,
            default_features: true,
            production: false,
        }];
        let with_dependencies = "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    dependencies: [\n        .package(url: \"https://github.com/apple/swift-log.git\", from: \"1.6.0\"),\n    ],\n    targets: [\n        .target(name: \"Shop\"),\n    ]\n)\n";
        let without = "// swift-tools-version: 6.0\nimport PackageDescription\n\nlet package = Package(\n    name: \"Shop\",\n    targets: [\n        .target(name: \"Shop\"),\n    ]\n)\n";
        for original in [with_dependencies, without] {
            let with = swift_package_with_sdk(original, &sdk).unwrap();
            assert!(
                with.contains("        .package(url: \"https://github.com/open-telemetry/opentelemetry-swift-core.git\", from: \"2.6.0\"),  // timonier\n"),
                "{with}"
            );
            assert!(
                with.contains("        .testTarget(name: \"TimonierTests\", dependencies: [.product(name: \"OpenTelemetryApi\", package: \"opentelemetry-swift-core\"), .product(name: \"OpenTelemetrySdk\", package: \"opentelemetry-swift-core\")]),  // timonier\n"),
                "{with}"
            );
            assert_eq!(swift_package_with_sdk(&with, &sdk).unwrap(), with, "twice");
            assert_eq!(swift_package_without_sdk(&with), original, "{with}");
        }
        assert!(
            swift_package_with_sdk(without, &sdk)
                .unwrap()
                .contains("    dependencies: [  // timonier\n")
        );
    }

    /// T-130: a test project takes the SDK and the JUnit logger in an `<ItemGroup>` of its own,
    /// and gives them back; a package referenced already is left.
    #[test]
    fn the_csharp_sdk_goes_in_and_out_of_a_csproj() {
        let sdk = [
            Sdk {
                name: "OpenTelemetry",
                version: "1.20.1",
                features: &[],
                default_features: true,
                production: false,
            },
            Sdk {
                name: "JunitXml.TestLogger",
                version: "8.0.0",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let project = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <ItemGroup>\n    <PackageReference Include=\"xunit\" Version=\"2.9.3\" />\n    <PackageReference Include=\"JunitXml.TestLogger\" Version=\"7.0.0\" />\n  </ItemGroup>\n</Project>\n";
        let with = csproj_with_sdk(project, &sdk).unwrap();
        assert!(with.contains("    <PackageReference Include=\"OpenTelemetry\" Version=\"1.20.1\" />\n  </ItemGroup>\n  <!-- <<< timonier <<< -->\n</Project>\n"), "{with}");
        assert_eq!(with.matches("JunitXml.TestLogger").count(), 1, "{with}");
        assert_eq!(csproj_with_sdk(&with, &sdk).unwrap(), with, "twice");
        assert_eq!(pom_without_sdk(&with), project);
    }

    /// T-131: `composer.json` takes the API in `require`, the SDK in `require-dev`, and gives
    /// them back as the file was.
    #[test]
    fn the_php_sdk_goes_in_and_out_of_composer_json() {
        let sdk = [
            Sdk {
                name: "open-telemetry/api",
                version: "^1.10",
                features: &[],
                default_features: true,
                production: true,
            },
            Sdk {
                name: "open-telemetry/sdk",
                version: "^1.15",
                features: &[],
                default_features: true,
                production: false,
            },
        ];
        let original =
            "{\n  \"name\": \"shop/shop\",\n  \"require\": {\n    \"php\": \">=8.4\"\n  }\n}\n";
        let with = composer_with_sdk(original, &sdk).unwrap();
        let parsed: Value = serde_json::from_str(&with).unwrap();
        assert_eq!(parsed["require"]["open-telemetry/api"], "^1.10");
        assert_eq!(parsed["require-dev"]["open-telemetry/sdk"], "^1.15");
        assert_eq!(composer_with_sdk(&with, &sdk).unwrap(), with, "twice");
        let back = composer_without_sdk(
            &with,
            original,
            &["open-telemetry/api", "open-telemetry/sdk"],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&back).unwrap(),
            serde_json::from_str::<Value>(original).unwrap()
        );
    }

    /// T-132: `build.sbt` takes a block of its own, the API for the code and the SDK `% Test`,
    /// and gives it back.
    #[test]
    fn the_scala_sdk_goes_in_and_out_of_build_sbt() {
        let sdk = java_sdk();
        let original = "ThisBuild / scalaVersion := \"3.10.0\"\n\nlazy val root = (project in file(\".\"))\n  .settings(name := \"shop\")\n";
        let with = sbt_with_sdk(original, &sdk);
        assert!(
            with.ends_with("// >>> timonier >>>\nlibraryDependencies ++= Seq(\n  \"io.opentelemetry\" % \"opentelemetry-api\" % \"1.67.0\",\n  \"io.opentelemetry\" % \"opentelemetry-sdk\" % \"1.67.0\" % Test,\n)\n// <<< timonier <<<\n"),
            "{with}"
        );
        assert_eq!(sbt_with_sdk(&with, &sdk), with, "twice");
        assert_eq!(gradle_without_sdk(&with), original);
    }

    /// A `vcpkg.json` without dependencies, indented as its author did, gets its list for
    /// opentelemetry-cpp, and loses the list again with the port.
    #[test]
    fn a_dependency_list_vcpkg_had_not_goes_with_the_port() {
        let features = ["otlp-http".to_owned()];
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.29.0",
            features: &features,
            default_features: true,
            production: false,
        }];
        let original = "{\n    \"name\": \"shop\",\n    \"version\": \"1.0.0\"\n}\n";
        let with = vcpkg_with_sdk(original, &sdk).unwrap();
        let back = vcpkg_without_sdk(&with, original, &["opentelemetry-cpp"]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&back).unwrap(),
            serde_json::from_str::<Value>(original).unwrap()
        );
    }

    /// `conanfile.txt` takes opentelemetry-cpp at the top of `[requires]`, or in a `[requires]`
    /// of its own, and gives it back as it was.
    #[test]
    fn the_cpp_sdk_goes_in_and_out_of_conanfile_txt() {
        let sdk = [Sdk {
            name: "opentelemetry-cpp",
            version: "1.26.0",
            features: &[],
            default_features: true,
            production: false,
        }];
        let with_requires = "[requires]\nfmt/12.2.0\n\n[generators]\nCMakeDeps\nCMakeToolchain\n";
        let without = "[generators]\nCMakeDeps\n";
        for original in [with_requires, without] {
            let with = conanfile_with_sdk(original, &sdk);
            assert!(with.contains("opentelemetry-cpp/1.26.0\n"), "{with}");
            assert_eq!(conanfile_with_sdk(&with, &sdk), with, "twice");
            assert_eq!(conanfile_without_sdk(&with), original, "{with}");
        }
        assert!(conanfile_with_sdk(with_requires, &sdk).starts_with("[requires]\n# >>> timonier >>>\nopentelemetry-cpp/1.26.0\n# <<< timonier <<<\nfmt/12.2.0\n"));
    }
}
