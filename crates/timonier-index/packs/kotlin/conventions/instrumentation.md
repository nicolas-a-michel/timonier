# Instrumentation conventions — Kotlin

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`kotlin`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each public function or method (Kotlin's default) that
   other packages call. `private` and `internal` helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `opentelemetry-api` only,
   whose global tracer does nothing without an SDK; the SDK is started by one initialisation
   class, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In Kotlin

- Spans: `val span = tracer.spanBuilder("Cart.total").setAttribute(…).startSpan()` then
  `try { span.makeCurrent().use { … } } finally { span.end() }`, with a `Tracer` from
  `GlobalOpenTelemetry.getTracer("<package>")`.
- Export: `io.opentelemetry:opentelemetry-sdk`, `opentelemetry-exporter-otlp` and
  `opentelemetry-sdk-extension-autoconfigure` 1.67.0, with `opentelemetry-api` 1.67.0 (on
  2026-10-22), which `timonier init` adds to the Gradle build: the API in `implementation`,
  since the code that ships uses it (inert without an SDK), the others in `testImplementation`.
  The object `timonier.TimonierOtel` (`src/test/kotlin/timonier/TimonierOtel.kt`), written by
  `init`, starts it: call `TimonierOtel.start()` where the tests set up (a JUnit extension, or a
  `@BeforeAll`) and call what it returns when they end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`,
  `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which
  Timonier drops the spans), which `timonier test` sets, and stays off with
  `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as Gradle's
  JUnit XML names it (`total()`, and the class's full name), on the span or on the resource.
  Nothing sets it for you: a span without it sits under its run, not under its test.
- `src/test/kotlin/timonier/TimonierSmokeTest.kt` is Timonier's smoke test, written for JUnit 5,
  which `timonier doctor` runs.
- Tests write the JUnit XML Timonier reads: Gradle writes it into `build/test-results/test/`,
  for each project.
