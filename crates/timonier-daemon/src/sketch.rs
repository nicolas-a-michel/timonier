//! The whiteboard drawn as an image, for the agent's vision (Q6, Q67, ADR 0040): the freehand
//! strokes, the typed shapes, and their ids, so that what the agent sees and what
//! `intent.json` says go together. Black on white, whatever the IDE's theme: an image to be
//! read, not shown. Nothing comes back onto the whiteboard.

use timonier_core::intent::{Bounds, Intent, Policy};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, StrokeDash,
    Transform,
};

/// The longest side of the image, in pixels.
pub const MAX_SIDE: f64 = 1600.0;
/// The room around the drawing, in pixels of the board.
const MARGIN: f64 = 32.0;

const INK: (u8, u8, u8) = (20, 20, 20);
const OUTLINE: (u8, u8, u8) = (60, 60, 60);
const LAYER: (u8, u8, u8) = (110, 110, 110);
const FORBIDDEN: (u8, u8, u8) = (200, 30, 30);
const NOTE: (u8, u8, u8) = (255, 246, 200);
const LABEL: (u8, u8, u8) = (90, 90, 90);

fn paint((r, g, b): (u8, u8, u8)) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 255);
    paint.anti_alias = true;
    paint
}

fn line(width: f32, dash: Option<(f32, f32)>) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        dash: dash.and_then(|(on, off)| StrokeDash::new(vec![on, off], 0.0)),
        ..Stroke::default()
    }
}

/// Everything the board draws, a stroke's points included: the image frames it.
fn extent(intent: &Intent) -> Option<Bounds> {
    let mut rects: Vec<Bounds> = intent
        .sketch
        .shapes
        .keys()
        .filter_map(|id| intent.bounds_of(id))
        .collect();
    rects.extend(intent.sketch.strokes.iter().map(Intent::stroke_bounds));
    let left = rects.iter().map(|b| b.x).reduce(f64::min)?;
    let top = rects.iter().map(|b| b.y).reduce(f64::min)?;
    let right = rects.iter().map(|b| b.x + b.width).reduce(f64::max)?;
    let bottom = rects.iter().map(|b| b.y + b.height).reduce(f64::max)?;
    Some(Bounds {
        x: left - MARGIN,
        y: top - MARGIN,
        width: right - left + 2.0 * MARGIN,
        height: bottom - top + 2.0 * MARGIN,
    })
}

/// Where the line from the middle of `rect` toward `(tx, ty)` leaves it.
fn border(rect: &Bounds, tx: f64, ty: f64) -> (f64, f64) {
    let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let (dx, dy) = (tx - cx, ty - cy);
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let sx = if dx == 0.0 {
        f64::INFINITY
    } else {
        rect.width / 2.0 / dx.abs()
    };
    let sy = if dy == 0.0 {
        f64::INFINITY
    } else {
        rect.height / 2.0 / dy.abs()
    };
    let scale = sx.min(sy);
    (cx + dx * scale, cy + dy * scale)
}

/// The sketch as a PNG; `None` when nothing is drawn.
pub fn render(intent: &Intent) -> Option<Vec<u8>> {
    let frame = extent(intent)?;
    let scale = (MAX_SIDE / frame.width.max(frame.height)).min(1.0);
    let width = (frame.width * scale).ceil().max(1.0) as u32;
    let height = (frame.height * scale).ceil().max(1.0) as u32;
    let mut pixmap = Pixmap::new(width, height)?;
    pixmap.fill(Color::WHITE);
    // From the board's coordinates to the image's.
    let at = |x: f64, y: f64| {
        (
            ((x - frame.x) * scale) as f32,
            ((y - frame.y) * scale) as f32,
        )
    };
    let s = scale as f32;
    let rect_of = |b: &Bounds| {
        let (l, t) = at(b.x, b.y);
        let (r, bottom) = at(b.x + b.width, b.y + b.height);
        Rect::from_ltrb(l, t, r, bottom)
    };
    let outline = |pixmap: &mut Pixmap, b: &Bounds, colour, stroke: &Stroke| {
        if let Some(rect) = rect_of(b) {
            let path = PathBuilder::from_rect(rect);
            pixmap.stroke_path(&path, &paint(colour), stroke, Transform::identity(), None);
        }
    };

    // Layers first, under everything.
    for layer in &intent.layers {
        if let Some(b) = intent.bounds_of(&layer.id) {
            outline(
                &mut pixmap,
                &b,
                LAYER,
                &line(2.0 * s, Some((8.0 * s, 6.0 * s))),
            );
            label(&mut pixmap, &layer.id, at(b.x + 6.0, b.y + 6.0), s);
        }
    }
    for component in &intent.components {
        if let Some(b) = intent.bounds_of(&component.id) {
            if let Some(rect) = rect_of(&b) {
                pixmap.fill_rect(rect, &paint((255, 255, 255)), Transform::identity(), None);
            }
            outline(&mut pixmap, &b, OUTLINE, &line(2.0 * s, None));
            label(&mut pixmap, &component.id, at(b.x + 6.0, b.y + 6.0), s);
        }
    }
    for edge in &intent.edges {
        let (Some(from), Some(to)) = (intent.bounds_of(&edge.src), intent.bounds_of(&edge.dst))
        else {
            continue;
        };
        let (fx, fy) = (from.x + from.width / 2.0, from.y + from.height / 2.0);
        let (tx, ty) = (to.x + to.width / 2.0, to.y + to.height / 2.0);
        let start = border(&from, tx, ty);
        let end = border(&to, fx, fy);
        let (colour, stroke) = match edge.policy {
            Policy::Allowed => (INK, line(2.0 * s, None)),
            Policy::Forbidden => (FORBIDDEN, line(2.5 * s, Some((10.0 * s, 6.0 * s)))),
            Policy::Required => (INK, line(4.5 * s, None)),
        };
        let mut path = PathBuilder::new();
        let (x0, y0) = at(start.0, start.1);
        let (x1, y1) = at(end.0, end.1);
        path.move_to(x0, y0);
        path.line_to(x1, y1);
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint(colour), &stroke, Transform::identity(), None);
        }
        // The head: a closed triangle at the end.
        let angle = (y1 - y0).atan2(x1 - x0);
        let size = 14.0 * s;
        let mut head = PathBuilder::new();
        head.move_to(x1, y1);
        for side in [-1.0_f32, 1.0] {
            let a = angle + side * std::f32::consts::PI / 7.0;
            head.line_to(x1 - size * a.cos(), y1 - size * a.sin());
        }
        head.close();
        if let Some(head) = head.finish() {
            pixmap.fill_path(
                &head,
                &paint(colour),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        label(
            &mut pixmap,
            &edge.id,
            ((x0 + x1) / 2.0 + 4.0 * s, (y0 + y1) / 2.0 + 4.0 * s),
            s,
        );
    }
    let notes = intent
        .notes
        .iter()
        .map(|n| &n.id)
        .chain(intent.budgets.iter().map(|b| &b.id));
    for id in notes {
        if let Some(b) = intent.bounds_of(id) {
            if let Some(rect) = rect_of(&b) {
                pixmap.fill_rect(rect, &paint(NOTE), Transform::identity(), None);
            }
            outline(&mut pixmap, &b, LAYER, &line(1.5 * s, None));
            label(&mut pixmap, id, at(b.x + 6.0, b.y + 6.0), s);
        }
    }
    // The freehand strokes last, on top, as they are on the whiteboard.
    let ink = line((3.0 * s).max(1.0), None);
    for stroke in &intent.sketch.strokes {
        let mut path = PathBuilder::new();
        for (i, [x, y, _]) in stroke.points.iter().enumerate() {
            let (px, py) = at(stroke.x + x, stroke.y + y);
            if i == 0 {
                path.move_to(px, py);
                // A single point still leaves a dot.
                path.line_to(px + 0.01, py);
            } else {
                path.line_to(px, py);
            }
        }
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint(INK), &ink, Transform::identity(), None);
        }
    }
    pixmap.encode_png().ok()
}

/// A 5×7 dot font for ids: the capitals, the digits, the dash and the underscore. Named ids
/// (`C-parser`, Q69) are written in capitals; a character outside the font is a hollow square,
/// never skipped (review n° 2 of Phase 7, I-1). Each row is 5 bits, the left one the highest.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

/// Writes an id at `(x, y)`, its top-left corner, in dots of two pixels at zoom 1.
fn label(pixmap: &mut Pixmap, text: &str, (x, y): (f32, f32), scale: f32) {
    let dot = (2.0 * scale).max(1.0);
    let colour = paint(LABEL);
    let mut left = x;
    for c in text.chars() {
        let rows = glyph(c);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..5 {
                if bits & (0x10 >> column) != 0
                    && let Some(rect) =
                        Rect::from_xywh(left + column as f32 * dot, y + row as f32 * dot, dot, dot)
                {
                    pixmap.fill_rect(rect, &colour, Transform::identity(), None);
                }
            }
        }
        left += 6.0 * dot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Intent {
        timonier_core::intent::parse(
            r#"{
              "version": 1,
              "layers": [{ "id": "L-1", "name": "Domain" }],
              "components": [{ "id": "C-1", "name": "Parser", "layer": "L-1", "role": "" },
                             { "id": "C-2", "name": "Store", "layer": null, "role": "" }],
              "edges": [{ "id": "E-1", "src": "C-1", "dst": "C-2", "kind": "depends_on", "policy": "forbidden" }],
              "notes": [{ "id": "N-1", "text": "Pure", "attached_to": "C-1" }],
              "sketch": {
                "shapes": { "L-1": { "x": 0, "y": 0, "width": 400, "height": 300 },
                            "C-1": { "x": 40, "y": 40, "width": 180, "height": 80 },
                            "C-2": { "x": 600, "y": 40, "width": 180, "height": 80 },
                            "N-1": { "x": 40, "y": 200 } },
                "strokes": [{ "id": "S-1", "x": 100, "y": 400, "points": [[0, 0, 0.5], [200, 0, 0.5]] }]
              }
            }"#,
        )
        .unwrap()
    }

    fn rgb(pixmap: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pixmap.pixel(x, y).unwrap().demultiply();
        (p.red(), p.green(), p.blue())
    }

    #[test]
    fn draws_the_shapes_the_arrows_and_the_strokes_black_on_white() {
        let png = render(&sample()).unwrap();
        let image = Pixmap::decode_png(&png).unwrap();
        // The drawing spans x -32..812 and y -32..432 on the board.
        assert_eq!((image.width(), image.height()), (844, 464));
        let at = |x: f64, y: f64| ((x + 32.0) as u32, (y + 32.0) as u32);
        let (x, y) = at(200.0, 400.0);
        assert_eq!(rgb(&image, x, y), INK, "the stroke");
        let (x, y) = at(500.0, 250.0);
        assert_eq!(rgb(&image, x, y), (255, 255, 255), "nothing there");
        let (x, y) = at(40.0, 80.0);
        assert_eq!(rgb(&image, x, y), OUTLINE, "the box's left side");
        // The forbidden arrow, red, on its way from C-1 to C-2 (dashes start at its tail).
        let (x, y) = at(222.0, 80.0);
        assert_eq!(rgb(&image, x, y), FORBIDDEN);
        let (x, y) = at(100.0, 230.0);
        assert_eq!(rgb(&image, x, y), NOTE, "the note");
    }

    #[test]
    fn the_same_whiteboard_gives_the_same_image_and_nothing_gives_none() {
        assert_eq!(render(&sample()), render(&sample()));
        assert_eq!(render(&Intent::empty()), None);
    }

    #[test]
    fn a_large_whiteboard_fits_the_largest_side() {
        let mut intent = sample();
        intent.sketch.strokes[0].points.push([8000.0, 3000.0, 0.5]);
        let image = Pixmap::decode_png(&render(&intent).unwrap()).unwrap();
        assert_eq!(image.width(), MAX_SIDE as u32);
        assert!(image.height() < image.width());
    }

    #[test]
    fn ids_are_written_on_the_image() {
        let mut pixmap = Pixmap::new(40, 20).unwrap();
        pixmap.fill(Color::WHITE);
        label(&mut pixmap, "C-1", (1.0, 1.0), 1.0);
        let dark = (0..40)
            .flat_map(|x| (0..20).map(move |y| (x, y)))
            .filter(|&(x, y)| rgb(&pixmap, x, y) == LABEL)
            .count();
        // C, the dash and 1: 13 + 5 + 10 dots of 2×2 pixels.
        assert_eq!(dark, (13 + 5 + 10) * 4);
    }

    /// Review n° 2 of Phase 7, I-1: a named id (Q69) is written whole, each character in its
    /// cell, and two named ids are not written alike.
    #[test]
    fn named_ids_are_written_whole() {
        let written = |text: &str| {
            let mut pixmap = Pixmap::new(200, 20).unwrap();
            pixmap.fill(Color::WHITE);
            label(&mut pixmap, text, (1.0, 1.0), 1.0);
            pixmap
        };
        let text = "C-db_2Ä";
        let image = written(text);
        for cell in 0..text.chars().count() as u32 {
            let left = 1 + cell * 12;
            let dark = (left..left + 10)
                .flat_map(|x| (1..15).map(move |y| (x, y)))
                .filter(|&(x, y)| rgb(&image, x, y) == LABEL)
                .count();
            assert!(dark > 0, "cell {cell} of {text} is empty");
        }
        assert_ne!(written("L-domain").data(), written("L-infra").data());
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('?'), glyph('é'), "the hollow square");
    }
}
