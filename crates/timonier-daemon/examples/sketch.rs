//! Draws a whiteboard as the agent sees it (Q67, ADR 0040), and says where each stroke is.
//!
//! `cargo run -p timonier-daemon --example sketch [intent.json] [out.png]`: by default the
//! project's `.timonier/intent.json`, into `sketch.png`.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .map_or_else(|| PathBuf::from(".timonier/intent.json"), PathBuf::from);
    let output = args
        .next()
        .map_or_else(|| PathBuf::from("sketch.png"), PathBuf::from);
    let intent = timonier_core::intent::parse(&std::fs::read_to_string(&input)?)
        .map_err(|refusal| refusal.message)?;
    match timonier_daemon::sketch::render(&intent) {
        Some(png) => {
            std::fs::write(&output, &png)?;
            println!("{} ({} bytes)", output.display(), png.len());
        }
        None => println!("nothing is drawn"),
    }
    for place in intent.stroke_places() {
        println!(
            "{}: layer {}, over {}, near {}",
            place.stroke,
            place.layer.as_deref().unwrap_or("-"),
            place.over.as_deref().unwrap_or("-"),
            place.near.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}
