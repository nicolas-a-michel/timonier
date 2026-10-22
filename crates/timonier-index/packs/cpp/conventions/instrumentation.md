# Instrumentation conventions — C++

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`cpp`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each function or method of a library's public header
   that other components call. Internal helpers (in an anonymous namespace, `static`, private)
   get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: the code depends on opentelemetry-cpp's API
   only, header-only and inert without an SDK; the SDK is started by one initialisation
   header, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In C++

- Spans: `auto span = tracer->StartSpan("Cart::total", {{"code.function.name", "Cart::total"},
  …}); auto scope = tracer->WithActiveSpan(span);` and `span->End()` before returning, with a
  tracer from `opentelemetry::trace::Provider::GetTracerProvider()->GetTracer("<library>")`.
- Export: opentelemetry-cpp 1.29.0 (on 2026-10-22) with its OTLP/HTTP exporter, which
  `timonier init` adds to `vcpkg.json` (`opentelemetry-cpp[otlp-http]`); with Conan, add
  `opentelemetry-cpp` to the conanfile; without either, install it so that
  `find_package(opentelemetry-cpp CONFIG)` finds it. The folder `timonier/`, written by `init`
  and added to the build by a block of the root `CMakeLists.txt`, gives the target
  `timonier_otel` and `timonier::start_telemetry()` (`timonier/timonier_otel.hpp`): link the
  tests to it, call it where they set up and call what it returns when they end. It reads
  `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS`
  (the run's token, without which Timonier drops the spans), which `timonier test` sets, and
  stays off with `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name` (and `timonier.test.classname`), as the JUnit
  XML of CTest names it (the test's name), on the span or on the resource. Nothing sets it for
  you: a span without it sits under its run, not under its test.
- `timonier/timonier_smoke_test.cpp` is Timonier's smoke test, which `timonier doctor` runs.
- Tests are CTest's (`add_test`), which writes the JUnit XML Timonier reads; the build for the
  runs goes into `.timonier/runs/cmake`.
