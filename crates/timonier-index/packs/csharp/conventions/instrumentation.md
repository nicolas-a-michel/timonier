# Instrumentation conventions — C#

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`csharp`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each `public` method of a type that other projects or
   namespaces call. `private` and `internal` helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code uses .NET's `ActivitySource`, which
   does nothing while nothing listens; the SDK is started by one initialisation class, when a
   configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In C#

- Spans: .NET's own `System.Diagnostics.ActivitySource` and `Activity`, which OpenTelemetry
  listens to: `using var activity = Source.StartActivity("Cart.Total");` then
  `activity?.SetTag("code.function.name", …)`; the `using` ends it.
- Export: `OpenTelemetry` and `OpenTelemetry.Exporter.OpenTelemetryProtocol` 1.20.1 (on
  2026-10-22), which `timonier init` adds to the test project with the JUnit logger
  `JunitXml.TestLogger` 8.0.0. `Timonier.TimonierOtel` (`Timonier/TimonierOtel.cs` in the
  test project), written by `init`, starts it over OTLP/HTTP for every ActivitySource: call
  `TimonierOtel.Start()` where the tests set up (a fixture) and `TimonierOtel.Stop()` when they
  end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES` and
  `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which Timonier drops the spans), which
  `timonier test` sets, and stays off with `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the JUnit
  XML names it (the test method, and its class), on the span or on the resource. Nothing sets
  it for you: a span without it sits under its run, not under its test.
- `Timonier/TimonierSmokeTest.cs` is Timonier's smoke test, written for xUnit, which
  `timonier doctor` runs.
- Tests write the JUnit XML Timonier reads with the command of `.timonier/tests.json`
  (`dotnet test --logger junit…`), one report per test assembly.
