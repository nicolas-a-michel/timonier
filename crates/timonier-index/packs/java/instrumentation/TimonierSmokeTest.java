// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.
package timonier;

import io.opentelemetry.api.GlobalOpenTelemetry;
import io.opentelemetry.api.trace.Span;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

class TimonierSmokeTest {
    static void smoke() {
        Span span = GlobalOpenTelemetry.getTracer("timonier-smoke")
                .spanBuilder("smoke")
                .setAttribute("code.function.name", "smoke")
                .setAttribute("code.file.path", "src/test/java/timonier/TimonierSmokeTest.java")
                // Where `smoke` starts, as the conventions ask.
                .setAttribute("code.line.number", 12L)
                .setAttribute("timonier.test.name", "smokeTraceReachesTimonier")
                .startSpan();
        span.end();
    }

    @Test
    void smokeTraceReachesTimonier() {
        Assumptions.assumeTrue(
                System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT") != null, "no run of timonier test");
        Runnable flush = TimonierOtel.start();
        smoke();
        flush.run();
    }
}
