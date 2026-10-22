// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.
package timonier

import io.opentelemetry.api.GlobalOpenTelemetry
import org.junit.jupiter.api.Assumptions
import org.junit.jupiter.api.Test

class TimonierSmokeTest {
    private fun smoke() {
        GlobalOpenTelemetry.getTracer("timonier-smoke")
            .spanBuilder("smoke")
            .setAttribute("code.function.name", "smoke")
            .setAttribute("code.file.path", "src/test/kotlin/timonier/TimonierSmokeTest.kt")
            // Where `smoke` starts, as the conventions ask.
            .setAttribute("code.line.number", 11L)
            .setAttribute("timonier.test.name", "smokeTraceReachesTimonier()")
            .startSpan()
            .end()
    }

    @Test
    fun smokeTraceReachesTimonier() {
        Assumptions.assumeTrue(System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT") != null, "no run of timonier test")
        val flush = TimonierOtel.start()
        smoke()
        flush()
    }
}
