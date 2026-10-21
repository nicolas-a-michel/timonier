# Instrumentation conventions — Rust

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`rust`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each public function of a module that other modules
   call. Internal helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph. Set them explicitly when the layer does not.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead. Metric names start with the crate's name.
4. **The instrumentation can be turned off**: library crates depend on `tracing` only; the
   OpenTelemetry layer and its exporter live behind a Cargo feature, in the binary.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In Rust

- Spans: `#[tracing::instrument(skip_all, fields(...))]` on the boundary functions, with
  small, explicit fields.
- Export: `tracing-opentelemetry` 0.34.0 with `opentelemetry`, `opentelemetry_sdk` and
  `opentelemetry-otlp` 0.33.1, upgraded together (on 2026-10-21). `timonier init` adds them to
  the root package as optional dependencies behind the feature `otel`, with the module
  `src/timonier_otel.rs`: declare it with `#[cfg(feature = "otel")] mod timonier_otel;` and keep
  `let _otel = timonier_otel::init();` alive at the start of `main` (or of a test). It exports
  when `OTEL_EXPORTER_OTLP_ENDPOINT` is set, as `timonier test` sets it, and reads
  `OTEL_SDK_DISABLED` itself: the Rust SDK does not. Under cargo-nextest, it names each span's
  test from `NEXTEST_TEST_NAME` and `NEXTEST_BINARY_ID`.
- `tracing` gives `code.file.path` and `code.line.number`; Timonier derives `code.function.name`
  from the module and the span's name: do not rename the span of a boundary function.
- `tests/timonier_smoke.rs` is Timonier's smoke test, which `timonier doctor` runs.
- Tests run with `cargo nextest run --profile timonier`, which writes the JUnit XML Timonier
  reads (`.timonier/tests.json`).
