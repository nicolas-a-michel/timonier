//! `.timonier/intent.json`, the whiteboard (spec « Tableau blanc », ADR 0039): what the developer
//! wants, written by the whiteboard after each of their gestures (Q66) and read by the agent.
//! The semantics (layers, components, edges, notes, budgets) are apart from the geometry
//! (`sketch`), which only serves to draw it again. Ids are local and stable: `L-`, `C-`, `E-`,
//! `N-`, and `S-` for the strokes. The file is written in one stable order and layout, for
//! readable git diffs.

use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::ser::Formatter;
use ts_rs::TS;

use crate::tracking::{Outcome, Refusal};

pub const INTENT_VERSION: u32 = 1;

/// At most as many elements as a whiteboard holds, and letters in a text.
pub const MAX_SHAPES: usize = 2_000;
pub const MAX_STROKES: usize = 10_000;
pub const MAX_POINTS: usize = 5_000;
pub const MAX_NAME: usize = 200;
pub const MAX_TEXT: usize = 2_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    /// By id.
    #[serde(default)]
    pub layers: Vec<IntentLayer>,
    #[serde(default)]
    pub components: Vec<IntentComponent>,
    #[serde(default)]
    pub edges: Vec<IntentEdge>,
    #[serde(default)]
    pub notes: Vec<IntentNote>,
    #[serde(default)]
    pub budgets: Vec<IntentBudget>,
    /// Where things are drawn: for the whiteboard only, never a constraint.
    #[serde(default)]
    pub sketch: Sketch,
}

/// A layer or a boundary (a zone of the whiteboard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IntentLayer {
    /// `L-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^L-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    #[schemars(length(max = MAX_NAME))]
    pub name: String,
}

/// An intended component (a box of the whiteboard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IntentComponent {
    /// `C-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^C-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    #[schemars(length(max = MAX_NAME))]
    pub name: String,
    /// The layer it is drawn in (Q62); `null` outside every layer.
    pub layer: Option<String>,
    /// What it does, in a sentence (Q63).
    #[schemars(length(max = MAX_TEXT))]
    pub role: String,
}

/// What an edge says of the dependency between its ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Allowed,
    Forbidden,
    Required,
}

/// The kind of an intended edge: any dependency of the code (Q64).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum IntentEdgeKind {
    DependsOn,
}

/// An arrow between two components or layers; between layers, it holds for all they contain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IntentEdge {
    /// `E-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^E-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    pub src: String,
    pub dst: String,
    pub kind: IntentEdgeKind,
    pub policy: Policy,
}

/// A constraint in words, attached to a component, a layer or an edge, or to nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IntentNote {
    /// `N-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^N-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    #[schemars(length(max = MAX_TEXT))]
    pub text: String,
    pub attached_to: Option<String>,
}

/// A measurable limit on a component or a path (spec « Budgets de performance »); every limit
/// is optional.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IntentBudget {
    /// `N-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^N-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    pub attached_to: Option<String>,
    /// Latency, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 0.0, max = 1e12))]
    pub p50_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 0.0, max = 1e12))]
    pub p95_ms: Option<f64>,
    /// Peak memory, in megabytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 0.0, max = 1e12))]
    pub memory_mb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(max = 1_000_000_000_000_u64))]
    pub allocations: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(max = 1_000_000_000_000_u64))]
    pub calls: Option<u64>,
}

/// The geometry of the whiteboard: where each element is, and the freehand strokes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sketch {
    /// By id: the top-left corner of each layer, component, note and budget, and the size of
    /// layers and components.
    #[serde(default)]
    pub shapes: BTreeMap<String, Place>,
    /// In drawing order: the last one is on top.
    #[serde(default)]
    pub strokes: Vec<SketchStroke>,
    /// The number of each prefix's next id (`C`, `E`, `L`, `N`, `S`): ids are never reused.
    #[serde(default)]
    #[ts(type = "Record<string, number>")]
    pub next: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub height: Option<f64>,
}

/// A freehand stroke: its points (x, y, pressure), relative to its corner `x`, `y`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SketchStroke {
    /// `S-` and a number up to 2^53 − 1 (9007199254740991), or a name (Q69).
    #[schemars(pattern(
        r"^S-([1-9][0-9]{0,15}|(?=[A-Za-z0-9_-]*[A-Za-z_-])[A-Za-z0-9][A-Za-z0-9_-]{0,63})$"
    ))]
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub points: Vec<[f64; 3]>,
    /// Drawn with a pen that says its pressure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pen: Option<bool>,
}

/// The largest id number: what JavaScript holds exactly, for the whiteboard reads it too.
pub const MAX_ID_NUMBER: u64 = (1 << 53) - 1;
/// The largest limit a budget may set, the same on the whiteboard.
pub const MAX_LIMIT: f64 = 1e12;

/// The number of `id` when it has the prefix: `C-12` is 12 for `C`.
fn number(prefix: &str, id: &str) -> Option<u64> {
    let digits = id.strip_prefix(prefix)?.strip_prefix('-')?;
    if digits.starts_with('0') || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| *n <= MAX_ID_NUMBER)
}

/// Whether `id` is a well-formed id of the prefix (Q69): numbered as the whiteboard makes them
/// (`C-12`), or named as the spec's example writes them by hand (`C-parser`): after the dash,
/// up to 64 ASCII letters, digits, dashes and underscores, a letter or a digit first.
pub fn valid_id(prefix: &str, id: &str) -> bool {
    if number(prefix, id).is_some() {
        return true;
    }
    let Some(rest) = id.strip_prefix(prefix).and_then(|r| r.strip_prefix('-')) else {
        return false;
    };
    rest.len() <= 64
        && rest
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        // Digits alone are a number, and only a number within bounds.
        && !rest.bytes().all(|b| b.is_ascii_digit())
}

fn invalid(message: String) -> Refusal {
    Refusal::new("invalid_file", format!("intent.json: {message}"))
}

fn finite(what: &str, value: f64) -> Outcome<()> {
    if value.is_finite() && value.abs() <= 1e9 {
        Ok(())
    } else {
        Err(invalid(format!("{what} is not a finite coordinate")))
    }
}

impl Intent {
    /// An empty whiteboard, as `timonier init` writes it.
    pub fn empty() -> Self {
        Self {
            version: INTENT_VERSION,
            ..Self::default()
        }
    }

    /// Nothing is drawn: the empty whiteboard `init` puts holds no shape and no stroke.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
            && self.components.is_empty()
            && self.edges.is_empty()
            && self.notes.is_empty()
            && self.budgets.is_empty()
            && self.sketch.strokes.is_empty()
    }

    /// Checks what the file holds: ids well formed and unique, references to what can take
    /// them, texts and numbers within bounds.
    pub fn check(&self) -> Outcome<()> {
        if self.version != INTENT_VERSION {
            return Err(invalid(format!(
                "version {} is not {INTENT_VERSION}",
                self.version
            )));
        }
        let shapes = self.layers.len()
            + self.components.len()
            + self.edges.len()
            + self.notes.len()
            + self.budgets.len();
        if shapes > MAX_SHAPES || self.sketch.strokes.len() > MAX_STROKES {
            return Err(invalid("more elements than a whiteboard holds".into()));
        }
        let mut seen = HashSet::new();
        let mut id = |prefix: &str, id: &str| -> Outcome<()> {
            if !valid_id(prefix, id) || !seen.insert(id.to_owned()) {
                return Err(invalid(format!(
                    "id {id:?} is malformed or repeated (expected {prefix}-n or {prefix}-name)"
                )));
            }
            Ok(())
        };
        for layer in &self.layers {
            id("L", &layer.id)?;
        }
        for component in &self.components {
            id("C", &component.id)?;
        }
        for edge in &self.edges {
            id("E", &edge.id)?;
        }
        for note in &self.notes {
            id("N", &note.id)?;
        }
        for budget in &self.budgets {
            id("N", &budget.id)?;
        }
        for stroke in &self.sketch.strokes {
            id("S", &stroke.id)?;
        }
        let layers: HashSet<&str> = self.layers.iter().map(|l| l.id.as_str()).collect();
        let components: HashSet<&str> = self.components.iter().map(|c| c.id.as_str()).collect();
        let edges: HashSet<&str> = self.edges.iter().map(|e| e.id.as_str()).collect();
        let long = |what: &str, text: &str, max: usize| -> Outcome<()> {
            if text.chars().count() > max {
                return Err(invalid(format!("{what} is longer than {max} characters")));
            }
            Ok(())
        };
        for layer in &self.layers {
            long(&layer.id, &layer.name, MAX_NAME)?;
        }
        for component in &self.components {
            long(&component.id, &component.name, MAX_NAME)?;
            long(&component.id, &component.role, MAX_TEXT)?;
            if let Some(layer) = &component.layer
                && !layers.contains(layer.as_str())
            {
                return Err(invalid(format!(
                    "{} is in layer {layer:?}, which is not there",
                    component.id
                )));
            }
        }
        let end = |id: &str| layers.contains(id) || components.contains(id);
        for edge in &self.edges {
            if edge.src == edge.dst || !end(&edge.src) || !end(&edge.dst) {
                return Err(invalid(format!(
                    "{} does not join two components or layers",
                    edge.id
                )));
            }
        }
        let attachable =
            |id: &Option<String>| id.as_deref().is_none_or(|id| end(id) || edges.contains(id));
        for note in &self.notes {
            long(&note.id, &note.text, MAX_TEXT)?;
            if !attachable(&note.attached_to) {
                return Err(invalid(format!(
                    "{} is attached to what is not there",
                    note.id
                )));
            }
        }
        for budget in &self.budgets {
            if !attachable(&budget.attached_to) {
                return Err(invalid(format!(
                    "{} is attached to what is not there",
                    budget.id
                )));
            }
            let counts = [budget.allocations, budget.calls]
                .into_iter()
                .flatten()
                .map(|n| n as f64);
            for limit in [budget.p50_ms, budget.p95_ms, budget.memory_mb]
                .into_iter()
                .flatten()
                .chain(counts)
            {
                if !limit.is_finite() || limit < 0.0 {
                    return Err(invalid(format!("{} has a limit below zero", budget.id)));
                }
                if limit > MAX_LIMIT {
                    return Err(invalid(format!(
                        "{} has a limit above {MAX_LIMIT}",
                        budget.id
                    )));
                }
            }
        }
        for (id, place) in &self.sketch.shapes {
            // A shape's place: not an edge's (it has none), nor a stroke's (its own `x`, `y`).
            let stroke = self.sketch.strokes.iter().any(|s| &s.id == id);
            if !seen.contains(id) || edges.contains(id.as_str()) || stroke {
                return Err(invalid(format!(
                    "sketch places {id:?}, which is not a shape"
                )));
            }
            finite(id, place.x)?;
            finite(id, place.y)?;
            for size in [place.width, place.height].into_iter().flatten() {
                if !size.is_finite() || size <= 0.0 {
                    return Err(invalid(format!("{id} has a size below zero")));
                }
            }
        }
        for stroke in &self.sketch.strokes {
            finite(&stroke.id, stroke.x)?;
            finite(&stroke.id, stroke.y)?;
            if stroke.points.is_empty() || stroke.points.len() > MAX_POINTS {
                return Err(invalid(format!("{} has no points, or too many", stroke.id)));
            }
            for point in &stroke.points {
                for value in point {
                    finite(&stroke.id, *value)?;
                }
            }
        }
        Ok(())
    }

    /// The same whiteboard in its one order: each list by id number, the named ids after the
    /// numbered ones by name; strokes keep their drawing order, which says what is on top.
    pub fn normalized(mut self) -> Self {
        fn by_number<T>(items: &mut [T], prefix: &str, id: impl Fn(&T) -> &str) {
            items.sort_by(|a, b| {
                let key = |item: &T| {
                    (
                        number(prefix, id(item)).unwrap_or(u64::MAX),
                        id(item).to_owned(),
                    )
                };
                key(a).cmp(&key(b))
            });
        }
        by_number(&mut self.layers, "L", |l| &l.id);
        by_number(&mut self.components, "C", |c| &c.id);
        by_number(&mut self.edges, "E", |e| &e.id);
        by_number(&mut self.notes, "N", |n| &n.id);
        by_number(&mut self.budgets, "N", |b| &b.id);
        self
    }

    /// The file's text: two spaces of indent, a stroke's points on one line, a final newline.
    /// The same whiteboard always gives the same text.
    pub fn to_text(&self) -> String {
        let mut out = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, Layout::default());
        // A struct of plain data always serializes.
        let _ = self.serialize(&mut serializer);
        let mut text = String::from_utf8(out).unwrap_or_default();
        text.push('\n');
        text
    }
}

/// serde_json's pretty layout, except that an array under a `points` key is written on one
/// line: a stroke changes one line of the diff, not hundreds.
#[derive(Default)]
struct Layout {
    indent: usize,
    has_value: bool,
    /// The key being written, and the last one written.
    in_key: bool,
    key: String,
    /// How deep inside a one-line array we are; 0 when not inside one.
    inline: usize,
}

impl Layout {
    fn newline<W: ?Sized + std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(b"\n")?;
        for _ in 0..self.indent {
            writer.write_all(b"  ")?;
        }
        Ok(())
    }
}

impl Formatter for Layout {
    fn begin_array<W: ?Sized + std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        if self.inline > 0 || self.key == "points" {
            self.inline += 1;
        } else {
            self.indent += 1;
        }
        self.key.clear();
        self.has_value = false;
        writer.write_all(b"[")
    }

    fn end_array<W: ?Sized + std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        if self.inline > 0 {
            self.inline -= 1;
        } else {
            self.indent -= 1;
            if self.has_value {
                self.newline(writer)?;
            }
        }
        self.has_value = true;
        writer.write_all(b"]")
    }

    fn begin_array_value<W: ?Sized + std::io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> std::io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        if self.inline == 0 {
            self.newline(writer)?;
        }
        Ok(())
    }

    fn end_array_value<W: ?Sized + std::io::Write>(
        &mut self,
        _writer: &mut W,
    ) -> std::io::Result<()> {
        self.has_value = true;
        Ok(())
    }

    fn begin_object<W: ?Sized + std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        self.indent += 1;
        self.has_value = false;
        writer.write_all(b"{")
    }

    fn end_object<W: ?Sized + std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        self.indent -= 1;
        if self.has_value {
            self.newline(writer)?;
        }
        self.has_value = true;
        writer.write_all(b"}")
    }

    fn begin_object_key<W: ?Sized + std::io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> std::io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        self.in_key = true;
        self.key.clear();
        self.newline(writer)
    }

    fn write_string_fragment<W: ?Sized + std::io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> std::io::Result<()> {
        if self.in_key {
            self.key.push_str(fragment);
        }
        writer.write_all(fragment.as_bytes())
    }

    fn end_object_key<W: ?Sized + std::io::Write>(
        &mut self,
        _writer: &mut W,
    ) -> std::io::Result<()> {
        self.in_key = false;
        Ok(())
    }

    fn begin_object_value<W: ?Sized + std::io::Write>(
        &mut self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        writer.write_all(b": ")
    }

    fn end_object_value<W: ?Sized + std::io::Write>(
        &mut self,
        _writer: &mut W,
    ) -> std::io::Result<()> {
        self.has_value = true;
        Ok(())
    }
}

/// A rectangle of the whiteboard, in its own coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    fn middle(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// How far apart two rectangles are: 0 when they touch or overlap.
    fn gap(&self, other: &Bounds) -> f64 {
        let dx = (other.x - (self.x + self.width))
            .max(self.x - (other.x + other.width))
            .max(0.0);
        let dy = (other.y - (self.y + self.height))
            .max(self.y - (other.y + other.height))
            .max(0.0);
        dx.hypot(dy)
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// The size of a note or a budget on the whiteboard, which the sketch does not keep.
pub const NOTE_SIZE: (f64, f64) = (200.0, 72.0);
/// How far from a component a stroke still counts as drawn near it, in pixels of the board.
pub const NEAR: f64 = 60.0;

/// Where a freehand stroke is drawn, for an agent that does not see the image (Q67): the layer
/// and the component that hold its middle, else the closest component within [`NEAR`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct StrokePlace {
    pub stroke: String,
    pub bounds: Bounds,
    /// The smallest layer that holds its middle.
    pub layer: Option<String>,
    /// The smallest component that holds its middle.
    pub over: Option<String>,
    /// Else, the closest component, when it is within [`NEAR`].
    pub near: Option<String>,
}

impl Intent {
    /// Where a shape is drawn: its place and size, notes at their fixed size; `None` for an
    /// edge, or a shape the sketch does not place.
    pub fn bounds_of(&self, id: &str) -> Option<Bounds> {
        let place = self.sketch.shapes.get(id)?;
        let (width, height) = match (place.width, place.height) {
            (Some(w), Some(h)) => (w, h),
            _ => NOTE_SIZE,
        };
        Some(Bounds {
            x: place.x,
            y: place.y,
            width,
            height,
        })
    }

    /// The rectangle a stroke's points take.
    pub fn stroke_bounds(stroke: &SketchStroke) -> Bounds {
        let (mut right, mut bottom) = (0.0_f64, 0.0_f64);
        for [x, y, _] in &stroke.points {
            right = right.max(*x);
            bottom = bottom.max(*y);
        }
        Bounds {
            x: stroke.x,
            y: stroke.y,
            width: right,
            height: bottom,
        }
    }

    /// Where each freehand stroke is, in drawing order (Q67).
    pub fn stroke_places(&self) -> Vec<StrokePlace> {
        let placed = |ids: Vec<&String>| -> Vec<(String, Bounds)> {
            ids.into_iter()
                .filter_map(|id| Some((id.clone(), self.bounds_of(id)?)))
                .collect()
        };
        let layers = placed(self.layers.iter().map(|l| &l.id).collect());
        let components = placed(self.components.iter().map(|c| &c.id).collect());
        let smallest = |shapes: &[(String, Bounds)], x: f64, y: f64| {
            shapes
                .iter()
                .filter(|(_, b)| b.contains(x, y))
                .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
                .map(|(id, _)| id.clone())
        };
        self.sketch
            .strokes
            .iter()
            .map(|stroke| {
                let bounds = Self::stroke_bounds(stroke);
                let (x, y) = bounds.middle();
                let over = smallest(&components, x, y);
                let near = if over.is_some() {
                    None
                } else {
                    components
                        .iter()
                        .map(|(id, b)| (id, b.gap(&bounds)))
                        .filter(|(_, gap)| *gap <= NEAR)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(id, _)| id.clone())
                };
                StrokePlace {
                    stroke: stroke.id.clone(),
                    bounds,
                    layer: smallest(&layers, x, y),
                    over,
                    near,
                }
            })
            .collect()
    }
}

/// The whiteboard a text holds, checked; the reason when it does not hold one.
pub fn parse(text: &str) -> Outcome<Intent> {
    let intent: Intent = serde_json::from_str(text).map_err(|e| invalid(e.to_string()))?;
    intent.check()?;
    Ok(intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's example, with a budget and a stroke.
    fn sample() -> Intent {
        let text = r#"{
          "version": 1,
          "layers": [{ "id": "L-2", "name": "Infra" }, { "id": "L-1", "name": "Domaine" }],
          "components": [{ "id": "C-1", "name": "Parser", "layer": "L-1",
                            "role": "Transforme les tokens en AST" }],
          "edges": [{ "id": "E-2", "src": "L-1", "dst": "L-2", "kind": "depends_on", "policy": "forbidden" }],
          "notes": [{ "id": "N-1", "text": "Aucune allocation dans la boucle chaude", "attached_to": "C-1" }],
          "budgets": [{ "id": "N-2", "attached_to": "C-1", "p95_ms": 5, "memory_mb": 50 }],
          "sketch": {
            "shapes": { "C-1": { "x": 40, "y": 40, "width": 180, "height": 80 }, "L-1": { "x": 0, "y": 0, "width": 480, "height": 320 },
                        "L-2": { "x": 600, "y": 0, "width": 480, "height": 320 }, "N-1": { "x": 60, "y": 200 }, "N-2": { "x": 300, "y": 200 } },
            "strokes": [{ "id": "S-1", "x": 10, "y": 20, "points": [[0, 0, 0.5], [20, 5, 0.5]] }],
            "next": { "C": 2, "E": 3, "L": 3, "N": 3, "S": 2 }
          }
        }"#;
        parse(text).unwrap()
    }

    /// The test set the whiteboard's tests read too (`tests/intent/`): both sides accept and
    /// refuse the same files (review of Phase 7, I1).
    #[test]
    fn accepts_and_refuses_the_shared_test_set() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intent");
        for (kind, accepted) in [("accepted", true), ("refused", false)] {
            let mut files: Vec<_> = std::fs::read_dir(folder.join(kind))
                .unwrap()
                .flatten()
                .collect();
            files.sort_by_key(std::fs::DirEntry::file_name);
            assert!(!files.is_empty());
            for file in files {
                let text = std::fs::read_to_string(file.path()).unwrap();
                let parsed = parse(&text);
                assert_eq!(
                    parsed.is_ok(),
                    accepted,
                    "{}: {parsed:?}",
                    file.path().display()
                );
            }
        }
    }

    /// The spec's own example (with the layer its edge points to), ids named by hand (Q69).
    #[test]
    fn reads_the_specs_example_and_writes_it_back_the_same() {
        let text = include_str!("../tests/intent/accepted/spec-example.json");
        let intent = parse(text).unwrap().normalized();
        assert_eq!(intent.components[0].id, "C-parser");
        assert_eq!(intent.edges[0].src, "L-domain");
        let written = intent.to_text();
        assert_eq!(parse(&written).unwrap().to_text(), written);
        assert!(
            valid_id("C", "C-parser") && valid_id("L", "L-my_layer-2") && valid_id("C", "C-12")
        );
        assert!(
            !valid_id("C", "C-")
                && !valid_id("C", "C-0")
                && !valid_id("C", "L-1")
                && !valid_id("C", "C-a b")
        );
        // Named ids come after numbered ones, by name.
        let mixed = parse(r#"{"version": 1, "layers": [{"id": "L-zeta", "name": ""}, {"id": "L-2", "name": ""}, {"id": "L-alpha", "name": ""}]}"#)
            .unwrap()
            .normalized();
        let order: Vec<&str> = mixed.layers.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(order, ["L-2", "L-alpha", "L-zeta"]);
    }

    #[test]
    fn reads_a_whiteboard_and_writes_it_back_the_same() {
        let intent = sample().normalized();
        let text = intent.to_text();
        assert_eq!(parse(&text).unwrap(), intent);
        assert_eq!(parse(&text).unwrap().to_text(), text);
        // In id order; a stroke's points on one line.
        assert!(text.find("\"L-1\"").unwrap() < text.find("\"L-2\"").unwrap());
        assert!(text.contains("\"points\": [[0.0,0.0,0.5],[20.0,5.0,0.5]]"));
        assert!(
            text.starts_with("{\n  \"version\": 1,\n  \"layers\": [\n    {\n      \"id\": \"L-1\"")
        );
        assert!(text.ends_with("}\n"));
        // An absent limit is not written; an absent pen neither.
        assert!(!text.contains("p50_ms"));
        assert!(!text.contains("\"pen\""));
    }

    #[test]
    fn a_moved_shape_keeps_its_id_and_changes_only_its_place() {
        let before = sample().normalized();
        let mut after = before.clone();
        after.sketch.shapes.get_mut("C-1").unwrap().x = 140.0;
        let (a, b) = (before.to_text(), after.to_text());
        let changed: Vec<_> = a.lines().zip(b.lines()).filter(|(x, y)| x != y).collect();
        assert_eq!(changed.len(), 1, "{changed:?}");
        assert_eq!(after.components, before.components);
    }

    #[test]
    fn refuses_what_does_not_hold_together() {
        let refused = |change: fn(&mut Intent)| {
            let mut intent = sample();
            change(&mut intent);
            intent.check().unwrap_err().message
        };
        assert!(refused(|i| i.version = 2).contains("version 2"));
        assert!(refused(|i| i.components[0].id = "L-9".into()).contains("malformed"));
        assert!(refused(|i| i.notes[0].id = "N-2".into()).contains("repeated"));
        assert!(refused(|i| i.components[0].layer = Some("L-9".into())).contains("not there"));
        assert!(refused(|i| i.edges[0].dst = "L-1".into()).contains("does not join"));
        assert!(refused(|i| i.edges[0].dst = "N-1".into()).contains("does not join"));
        assert!(refused(|i| i.notes[0].attached_to = Some("S-1".into())).contains("not there"));
        assert!(refused(|i| i.budgets[0].p95_ms = Some(-1.0)).contains("below zero"));
        assert!(
            refused(|i| {
                i.sketch.shapes.insert(
                    "E-2".into(),
                    Place {
                        x: 0.0,
                        y: 0.0,
                        width: None,
                        height: None,
                    },
                );
            })
            .contains("not a shape")
        );
        assert!(refused(|i| i.sketch.strokes[0].points.clear()).contains("no points"));
        assert!(
            refused(|i| {
                i.sketch.shapes.insert(
                    "S-1".into(),
                    Place {
                        x: 0.0,
                        y: 0.0,
                        width: None,
                        height: None,
                    },
                );
            })
            .contains("not a shape")
        );
        assert!(refused(|i| i.layers[0].name = "x".repeat(MAX_NAME + 1)).contains("longer"));
        assert!(
            parse(r#"{"version": 1, "extra": 1}"#)
                .unwrap_err()
                .message
                .contains("unknown field")
        );
    }

    #[test]
    fn says_where_each_stroke_is_drawn() {
        let mut intent = sample();
        let stroke = |id: &str, x: f64, y: f64| SketchStroke {
            id: id.into(),
            x,
            y,
            points: vec![[0.0, 0.0, 0.5], [40.0, 10.0, 0.5]],
            pen: None,
        };
        intent.sketch.strokes = vec![
            // Over the Parser (C-1 at 40,40 180x80), in Domain.
            stroke("S-1", 60.0, 60.0),
            // Below it, within reach, still in Domain.
            stroke("S-2", 60.0, 150.0),
            // In Infra, far from any component.
            stroke("S-3", 800.0, 250.0),
            // Outside everything.
            stroke("S-4", 2000.0, 2000.0),
        ];
        let places = intent.stroke_places();
        let said: Vec<_> = places
            .iter()
            .map(|p| {
                (
                    p.stroke.as_str(),
                    p.layer.as_deref(),
                    p.over.as_deref(),
                    p.near.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            said,
            [
                ("S-1", Some("L-1"), Some("C-1"), None),
                ("S-2", Some("L-1"), None, Some("C-1")),
                ("S-3", Some("L-2"), None, None),
                ("S-4", None, None, None),
            ]
        );
        assert_eq!(
            places[0].bounds,
            Bounds {
                x: 60.0,
                y: 60.0,
                width: 40.0,
                height: 10.0
            }
        );
        assert_eq!(
            intent.bounds_of("N-1").map(|b| (b.width, b.height)),
            Some(NOTE_SIZE)
        );
        assert_eq!(intent.bounds_of("E-2"), None);
    }

    #[test]
    fn an_empty_whiteboard_is_small_and_valid() {
        let text = Intent::empty().to_text();
        assert_eq!(parse(&text).unwrap(), Intent::empty());
        assert!(Intent::empty().is_empty());
        assert!(!sample().is_empty());
        assert_eq!(
            text,
            "{\n  \"version\": 1,\n  \"layers\": [],\n  \"components\": [],\n  \"edges\": [],\n  \"notes\": [],\n  \"budgets\": [],\n  \"sketch\": {\n    \"shapes\": {},\n    \"strokes\": [],\n    \"next\": {}\n  }\n}\n"
        );
    }
}
