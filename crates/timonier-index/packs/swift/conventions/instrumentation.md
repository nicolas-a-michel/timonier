# Instrumentation conventions — Swift

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`swift`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each `public` (or `open`) function or method of a
   module that other modules call. Internal and private helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `OpenTelemetryApi` only,
   whose default tracer does nothing without an SDK; the SDK is started by one initialisation
   type, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In Swift

- Spans: `let span = OpenTelemetry.instance.tracerProvider.get(instrumentationName: "<module>",
  instrumentationVersion: nil).spanBuilder(spanName: "Cart.total").setAttribute(…).startSpan()`
  then `defer { span.end() }`.
- Export: `OpenTelemetryApi` and `OpenTelemetrySdk` of opentelemetry-swift-core 2.6.0 and
  `OpenTelemetryProtocolExporterHTTP` of opentelemetry-swift 2.6.0 (on 2026-10-22), which
  `timonier init` adds to `Package.swift` with a test target `TimonierTests` (lines marked
  `// timonier`). `TimonierOtel.start()`, in that target, starts the export and returns what
  flushes it. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES` and
  `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which Timonier drops the spans), which
  `timonier test` sets, and stays off with `OTEL_SDK_DISABLED=true`. To measure your own
  targets, add `OpenTelemetryApi` to them, and move the starter where their tests set up.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the xUnit
  XML names it (`testTotal`, and `ShopTests.CartTests`), on the span or on the resource.
  Nothing sets it for you: a span without it sits under its run, not under its test.
- `Tests/TimonierTests/TimonierSmokeTests.swift` is Timonier's smoke test, which
  `timonier doctor` runs.
- Tests write the xUnit XML Timonier reads with the command of `.timonier/tests.json`
  (`swift test --parallel --xunit-output …`): one file for XCTest, one for Swift Testing.
