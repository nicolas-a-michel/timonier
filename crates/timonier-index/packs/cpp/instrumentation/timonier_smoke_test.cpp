// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `timonier/timonier_otel.hpp`.
#include <cstdlib>

#include "opentelemetry/trace/provider.h"
#include "timonier_otel.hpp"

namespace {

void smoke() {
    auto tracer = opentelemetry::trace::Provider::GetTracerProvider()->GetTracer("timonier-smoke");
    auto span = tracer->StartSpan(
        "smoke", {{"code.function.name", "smoke"},
                  {"code.file.path", "timonier/timonier_smoke_test.cpp"},
                  // Where `smoke` starts, as the conventions ask.
                  {"code.line.number", 11},
                  {"timonier.test.name", "timonier_smoke_test"}});
    span->End();
}

}  // namespace

int main() {
    if (std::getenv("OTEL_EXPORTER_OTLP_ENDPOINT") == nullptr) {
        return 0;  // no run of timonier test
    }
    auto stop = timonier::start_telemetry();
    smoke();
    stop();
    return 0;
}
