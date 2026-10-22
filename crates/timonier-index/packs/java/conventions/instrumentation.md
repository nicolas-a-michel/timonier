# Instrumentation conventions — Java

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`java`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each public method of a class that other packages
   call. Private and package-private helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `opentelemetry-api` only,
   whose global tracer does nothing without an SDK; the SDK is started by one initialisation
   class, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In Java

- Spans: `Span span = tracer.spanBuilder("Cart.total").setAttribute(…).startSpan();` then
  `try (Scope scope = span.makeCurrent()) { … } finally { span.end(); }`, with a `Tracer` from
  `GlobalOpenTelemetry.getTracer("<package>")`.
- Export: `io.opentelemetry:opentelemetry-sdk`, `opentelemetry-exporter-otlp` and
  `opentelemetry-sdk-extension-autoconfigure` 1.67.0, with `opentelemetry-api` 1.67.0 (on
  2026-10-22), which `timonier init` adds: the API among the project's dependencies, since the
  code that ships uses it (inert without an SDK), the others for the tests only.
  The class `timonier.TimonierOtel` (`src/test/java/timonier/TimonierOtel.java`), written by
  `init`, starts it: call `TimonierOtel.start()` where the tests set up (a JUnit extension, or a
  `@BeforeAll`) and run what it returns when they end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`,
  `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which
  Timonier drops the spans), which `timonier test` sets, and stays off with
  `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the JUnit
  XML names it (the method, and the class's full name), on the span or on the resource.
  Nothing sets it for you: a span without it sits under its run, not under its test.
- `src/test/java/timonier/TimonierSmokeTest.java` is Timonier's smoke test, written for JUnit 5,
  which `timonier doctor` runs.
- Tests write the JUnit XML Timonier reads: Maven's Surefire into `target/surefire-reports/`,
  Gradle into `build/test-results/test/`, for each module.
