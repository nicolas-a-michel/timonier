//! Which node of the graph a span is about (ADR 0048): by its function's ID, by its name in its
//! file, by its line, or by its file alone.

use std::path::Path;

use timonier_core::perf::{Attachment, ReceivedSpan};
use timonier_core::{NodeId, Position};
use timonier_store::Reader;

/// The node and the rule that found it; `None` when the span says no file nor function the
/// graph knows.
pub(crate) fn attach(
    reader: &Reader,
    root: &Path,
    span: &ReceivedSpan,
) -> Option<(String, Attachment)> {
    let code = &span.code;
    let file = code.file.as_deref().map(|f| relative(root, f));
    let rust = span.language.as_deref() == Some("rust")
        || file.as_deref().is_some_and(|f| f.ends_with(".rs"));
    if let Some(function) = &code.function
        && rust
        && let Ok(id) = NodeId::parse(format!("rust:{function}"))
        && reader.exists(&id).unwrap_or(false)
    {
        return Some((id.as_str().to_owned(), Attachment::Id));
    }
    let file = NodeId::parse(format!("file:{}", file?)).ok()?;
    if let Some(function) = &code.function
        && let Some(id) = by_name(reader, &file, function, code.line)
    {
        return Some((id, Attachment::Name));
    }
    if let Some(line) = code
        .line
        .and_then(|l| u32::try_from(l).ok())
        .filter(|l| *l > 0)
    {
        // Lines are 1-based in `code.line.number`, 0-based in the graph (ADR 0008); the whole
        // line, whatever its column.
        if let Ok(Some(id)) = reader.symbol_at(&file, Position::new(line - 1, u32::MAX)) {
            return Some((id.as_str().to_owned(), Attachment::Line));
        }
    }
    reader
        .exists(&file)
        .unwrap_or(false)
        .then(|| (file.as_str().to_owned(), Attachment::File))
}

/// The function of `file` whose ID ends with the last name of `function`; of several, the one
/// that starts nearest `line`.
fn by_name(reader: &Reader, file: &NodeId, function: &str, line: Option<u64>) -> Option<String> {
    // `a::b::name`, `A.name`, `pkg/path#A.name`: the name after the last separator.
    let last = function
        .rsplit([':', '.', '#'])
        .next()
        .filter(|l| !l.is_empty())?;
    let candidates: Vec<(NodeId, Option<u32>)> = reader
        .callables_in(file)
        .ok()?
        .into_iter()
        .filter(|(id, _)| {
            let id = id.as_str();
            ["::", "#", "."]
                .iter()
                .any(|separator| id.ends_with(&format!("{separator}{last}")))
        })
        .collect();
    let wanted = line.map(|l| l.saturating_sub(1));
    candidates
        .into_iter()
        .min_by_key(|(_, start)| match (wanted, start) {
            (Some(wanted), Some(start)) => wanted.abs_diff(u64::from(*start)),
            _ => u64::MAX,
        })
        .map(|(id, _)| id.as_str().to_owned())
}

/// A path as the graph names it: relative to the root, with `/`.
fn relative(root: &Path, file: &str) -> String {
    let path = Path::new(file);
    let path = path.strip_prefix(root).unwrap_or(path);
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}
