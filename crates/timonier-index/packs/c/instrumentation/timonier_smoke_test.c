/* Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
 * `timonier test`, and its span must come back attached to the code graph. It goes through this
 * project's OpenTelemetry API, `timonier/timonier_otel.h`. */
#include <stdlib.h>

#include "timonier_otel.h"

static void smoke(void) {
    /* Where `smoke` starts, as the conventions ask. */
    timonier_span* span = timonier_span_start("smoke", "smoke", "timonier/timonier_smoke_test.c", 8);
    timonier_span_set(span, "timonier.test.name", "timonier_smoke_test");
    timonier_span_end(span);
}

int main(void) {
    if (getenv("OTEL_EXPORTER_OTLP_ENDPOINT") == NULL) {
        return 0; /* no run of timonier test */
    }
    timonier_start_telemetry();
    smoke();
    timonier_stop_telemetry();
    return 0;
}
