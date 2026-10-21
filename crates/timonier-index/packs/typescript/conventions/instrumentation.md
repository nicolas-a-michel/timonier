# Instrumentation conventions — TypeScript

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`typescript`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each exported function of a module that other modules
   call. Internal helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `@opentelemetry/api` only,
   which does nothing without an SDK; the SDK is started by one initialisation module, when a
   configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In TypeScript

- Spans: `trace.getTracer(<package name>).startActiveSpan(...)` from `@opentelemetry/api`, the
  span ended in a `finally`.
- Export: `@opentelemetry/sdk-node` 0.223.0 with `@opentelemetry/api` 1.9.1 (on 2026-10-21),
  which `timonier init` adds: the API as a dependency, since the code that ships imports it
  (inert without an SDK), the SDK as a development dependency; sdk-node brings the OTLP
  exporters.
  The module `timonier.otel.ts`, written by `init`, starts it: call `startTelemetry()` where the
  tests set up (a vitest `setupFiles` entry, say) and await what it returns when they end. It
  reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS`
  (the run's token, without which Timonier drops the spans), which `timonier test` sets, and
  stays off with `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the JUnit
  XML names it, on the span or on the resource. Nothing sets it for you in TypeScript: a span
  without it sits under its run, not under its test.
- `timonier.smoke.test.ts` is Timonier's smoke test, which `timonier doctor` runs.
- Tests write the JUnit XML Timonier reads with the command of `.timonier/tests.json`.
