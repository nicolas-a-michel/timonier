# Instrumentation conventions — Go

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`go`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each exported function or method of a package (a name
   that starts with a capital letter) that other packages call. Unexported helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `go.opentelemetry.io/otel`
   only, whose global tracer does nothing without an SDK; the SDK is started by one
   initialisation package, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In Go

- Spans: `ctx, span := otel.Tracer("<import path>").Start(ctx, "Name", trace.WithAttributes(…))`
  then `defer span.End()`; pass `ctx` on, so that the spans below have this one as parent.
- Export: `go.opentelemetry.io/otel`, `go.opentelemetry.io/otel/sdk` and
  `go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp` v1.47.0 (on 2026-10-22),
  which `timonier init` adds to `go.mod`; run `go mod tidy` afterwards, which fills `go.sum`.
  The package `timonier` (`timonier/otel.go`), written by `init`, starts it: call
  `stop := timonier.StartTelemetry(ctx)` where the tests set up (a `TestMain`, say) and
  `stop(ctx)` when they end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES`
  and `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which Timonier drops the spans),
  which `timonier test` sets, and stays off with `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the JUnit
  XML names it (`TestTotal`, and the package's import path), on the span or on the resource.
  Nothing sets it for you: a span without it sits under its run, not under its test.
- `timonier/timonier_smoke_test.go` is Timonier's smoke test, which `timonier doctor` runs.
- Tests write the JUnit XML Timonier reads with the command of `.timonier/tests.json`
  (`gotestsum --junitfile …`).
