---
name: otel-instrumentation
description: OpenTelemetry instrumentation conventions imposed by Timonier, which Timonier also applies to its own code — one span per component boundary, mandatory code.* attributes, timonier.* attributes, no spans in hot loops (histograms and counters instead), instrumentation that can be disabled at compile time or by configuration, and how test runs inject the OTLP endpoint. Use when instrumenting any code (Timonier's own crates or a language pack's conventions), when touching timonier-otlp or `timonier test`, or when reviewing instrumentation gaps.
---

# OpenTelemetry instrumentation

Performance graphs fill themselves during tests through explicit OpenTelemetry instrumentation
("level 3"), from the MVP on. The instrumentation uses the language's standard SDK, with no
proprietary library, so it stays valid if Timonier disappears. Timonier instruments itself with
the same rules.

## The rules (spec, « Conventions imposées à l'IA »)

1. **One span per component boundary**: each public function of a module.
2. **Mandatory attributes** on these spans: `code.function.name`, `code.file.path`,
   `code.line.number`. They are what attaches a span to the graph.
3. **No span in a hot loop.** Use histograms and counters instead.
4. **Instrumentation can be disabled** at compile time or by configuration. In Rust:
   `tracing` + `tracing-opentelemetry` behind a feature.
5. **Internal calls** also carry `timonier.payload.bytes` and `timonier.payload.type` (used by
   the data-flow view in v2).

## How a test run connects (spec)

- `timonier test` sets `OTEL_EXPORTER_OTLP_ENDPOINT` to the local receiver, which listens on
  gRPC 4317 and HTTP 4318.
- It also sets `OTEL_RESOURCE_ATTRIBUTES=timonier.run_id=…,timonier.snapshot=…`.
- Each test becomes a root span.
- After each run the daemon checks for:
  - public functions never seen in the traces;
  - spans missing the `code.*` attributes;
  - the overhead, estimated against the level-1 measurement.
- These gaps are reported and readable by the agent over MCP (`get_instrumentation_gaps`).

## The receiver (ADR 0047, 0048)

- `timonier-otlp` listens on 127.0.0.1, 4317 and 4318 or ports of the system's choice; each port
  serves gRPC and HTTP (protobuf or JSON), so the injected endpoint suits any exporter. The
  daemon starts it at the first `run/start`.
- It keeps only spans and points whose resource or own attributes carry `timonier.run_id` of
  an open run (or one ended less than 60 s ago).
- A span names its test with `timonier.test.name` and `timonier.test.classname` (span or
  resource attribute), matched against the JUnit cases; each JUnit case is a synthesized root
  span.
- A span finds its node by: `rust:` + `code.function.name` as a graph ID; then the function of
  `code.file.path` with the same last name; then the symbol holding `code.line.number`; then
  the file.

## Applying it in Timonier's Rust code

- Library crates depend only on `tracing`. A span costs almost nothing when no subscriber is
  installed.
- The OpenTelemetry layer and exporter live behind the Cargo feature `otel`, in `timonier-cli`
  only, on by default (ADR 0044). `--no-default-features` builds a binary with no OpenTelemetry
  dependency. At run time the exporter exists only when `OTEL_EXPORTER_OTLP_ENDPOINT` is set;
  `OTEL_SDK_DISABLED=true` turns it off.
  - Verified versions: `tracing-opentelemetry` 0.34.0 goes with `opentelemetry` /
    `opentelemetry_sdk` / `opentelemetry-otlp` 0.33.1.
  - The OTel Rust traces API is still Beta: always upgrade these crates together.
- Public boundary functions use `#[tracing::instrument(skip_all, fields(...))]`, with explicit,
  small fields. Never put file contents, secrets or whole payloads in fields: sizes only.
- Verified on 2026-10-21 (`tracing-opentelemetry` 0.34.0, `src/layer.rs`): it exports
  `code.file.path`, `code.line.number` and `code.module.name` (current names), and **never**
  `code.function.name`. The receiver derives it as `<code.module.name>::<span name>` for such
  spans and marks it derived (ADR 0044): do not rename the span of a boundary function
  (`#[instrument(name = …)]`), or the derived name is wrong.
- Hot paths get counters and histograms, not spans. Examples: the per-node and per-edge loops
  in indexing, SQLite row loops, the particle loop in the webview. Metric names use the
  `timonier.` prefix.
- Timonier-specific attributes use the `timonier.*` namespace. Everything else uses the
  OpenTelemetry semantic-convention names.

## Review checklist

- [ ] Every public function of a component boundary has a span. Internal helpers do not.
- [ ] Every span has the `code.*` attributes, set automatically or explicitly.
- [ ] No span is created inside a loop over nodes, edges, rows, tokens or frames.
- [ ] A build without the feature compiles and runs with no exporter.
- [ ] No secret, path to a user secret or file content appears in attributes.
- [ ] A test proves that a span reaches the receiver and attaches to the right graph node
      (from Phase 8 on).
