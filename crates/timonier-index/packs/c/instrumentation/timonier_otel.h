/* OpenTelemetry for this project, written by `timonier init` (Timonier's C conventions, in
 * `.timonier/conventions/c/`). OpenTelemetry has no C SDK: this C API is implemented in C++ on
 * opentelemetry-cpp (`timonier_otel.cpp`). The export starts when a run of `timonier test` asks
 * (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
 * `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`. */
#ifndef TIMONIER_OTEL_H
#define TIMONIER_OTEL_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct timonier_span timonier_span;

/* Starts the export when a run asks for it; does nothing otherwise. */
void timonier_start_telemetry(void);
/* Flushes and stops the export. */
void timonier_stop_telemetry(void);

/* Starts a span with the `code.*` attributes the conventions ask for; ends it with
 * `timonier_span_end`. Without an export, both cost a call and nothing else. */
timonier_span* timonier_span_start(const char* name, const char* function, const char* file, int line);
/* Sets a string attribute, `timonier.test.name` say, on a started span. */
void timonier_span_set(timonier_span* span, const char* key, const char* value);
void timonier_span_end(timonier_span* span);

#ifdef __cplusplus
}
#endif

#endif
