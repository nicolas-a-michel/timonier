# Instrumentation conventions — PHP

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`php`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each `public` method of a class that other namespaces
   call. `private` and `protected` helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on `open-telemetry/api` only,
   whose global tracer does nothing without an SDK; the SDK is started by one initialisation
   class, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In PHP

- Spans: `$span = Globals::tracerProvider()->getTracer('<package>')->spanBuilder('Cart::total')
  ->setAttribute(…)->startSpan();` then `$span->end();` in a `finally`, from
  `open-telemetry/api`.
- Export: `open-telemetry/sdk` 1.15 and `open-telemetry/exporter-otlp` 1.4, with
  `open-telemetry/api` 1.10 and Guzzle as their HTTP client (on 2026-10-22), which
  `timonier init` adds to `composer.json` (the API in `require`, the others in
  `require-dev`); run `composer update` afterwards. `Timonier\TimonierOtel`
  (`tests/Timonier/TimonierOtel.php`), written by `init`, starts it: call
  `TimonierOtel::start()` where the tests set up (PHPUnit's bootstrap file) and
  `TimonierOtel::stop()` when they end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`,
  `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which
  Timonier drops the spans), which `timonier test` sets, and stays off with
  `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as PHPUnit's
  JUnit XML names it (`testTotal`, and the class's full name), on the span or on the
  resource. Nothing sets it for you: a span without it sits under its run, not under its test.
- `tests/Timonier/TimonierSmokeTest.php` is Timonier's smoke test, which `timonier doctor`
  runs.
- Tests write the JUnit XML Timonier reads with the command of `.timonier/tests.json`
  (`php vendor/bin/phpunit --log-junit …`).
