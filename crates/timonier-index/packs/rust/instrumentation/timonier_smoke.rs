//! Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
//! `timonier test`, and its span must come back attached to the code graph. It goes through this
//! crate's OpenTelemetry module, `src/timonier_otel.rs`.
#![cfg(feature = "otel")]

#[path = "../src/timonier_otel.rs"]
mod timonier_otel;

#[test]
fn timonier_smoke() {
    let otel = timonier_otel::init();
    smoke();
    drop(otel);
}

#[tracing::instrument]
fn smoke() {
    tracing::info!("the smoke test of the instrumentation");
}
