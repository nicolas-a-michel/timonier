# Instrumentation conventions — C

Timonier fills the project's performance views from OpenTelemetry traces recorded while the
tests run. Instrument the code you write as follows. These rules come from Timonier's language
pack (`c`); `timonier init` copies them here, and `timonier doctor` keeps them current.

## Rules

1. **One span per component boundary**: each function of a library's public header that other
   components call. `static` helpers get none.
2. **Every such span carries** `code.function.name`, `code.file.path` and `code.line.number`:
   they attach the span to the code graph.
3. **No span in a hot loop** (over items, rows, tokens, frames): count with a counter or
   measure with a histogram instead.
4. **The instrumentation can be turned off**: without the export started, a span costs a call
   and nothing else; the export starts in one place, when a configuration variable asks for it.
5. **Never put** file contents, secrets or whole payloads in attributes: sizes and kinds only.

## In C

- OpenTelemetry has no C SDK. `timonier init` writes the folder `timonier/`: a C API,
  `timonier_otel.h`, implemented in C++ (`timonier_otel.cpp`) on opentelemetry-cpp 1.29.0 and
  its OTLP/HTTP exporter (on 2026-10-22), and the CMake that builds it into the library
  `timonier_otel` when `find_package(opentelemetry-cpp CONFIG)` finds the SDK; the build needs a
  C++ compiler. `init` adds `opentelemetry-cpp[otlp-http]` to `vcpkg.json`; with Conan, add
  `opentelemetry-cpp` to the conanfile; without either, install it.
- Spans: `timonier_span* span = timonier_span_start("cart_total", "cart_total", __FILE__,
  __LINE__);` then `timonier_span_end(span);` before returning; `timonier_span_set` adds a
  string attribute. Link the code and the tests to `timonier_otel`, call
  `timonier_start_telemetry()` where the tests set up and `timonier_stop_telemetry()` when they
  end. It reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_RESOURCE_ATTRIBUTES` and
  `OTEL_EXPORTER_OTLP_HEADERS` (the run's token, without which Timonier drops the spans), which
  `timonier test` sets, and stays off with `OTEL_SDK_DISABLED=true`.
- Name the test of a span: `timonier.test.name`, as the JUnit XML of CTest names it (the test's
  name), with `timonier_span_set`. Nothing sets it for you: a span without it sits under its
  run, not under its test.
- `timonier/timonier_smoke_test.c` is Timonier's smoke test, which `timonier doctor` runs.
- Tests are CTest's (`add_test`), which writes the JUnit XML Timonier reads; the build for the
  runs goes into `.timonier/runs/cmake`.
