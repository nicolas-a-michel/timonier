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
- Export: `tracing-opentelemetry` with `opentelemetry`, `opentelemetry_sdk` and
  `opentelemetry-otlp`, upgraded together (0.34.0 and 0.33.1 on 2026-10-20). The exporter reads
  `OTEL_EXPORTER_OTLP_ENDPOINT`, which `timonier test` sets.
- Tests run with `cargo nextest run --profile timonier`, which writes the JUnit XML Timonier
  reads (`.timonier/tests.json`).
