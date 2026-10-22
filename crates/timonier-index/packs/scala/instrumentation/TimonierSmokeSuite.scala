// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.
package timonier

import io.opentelemetry.api.GlobalOpenTelemetry

class TimonierSmokeSuite extends munit.FunSuite:
  private def smoke(): Unit =
    GlobalOpenTelemetry
      .getTracer("timonier-smoke")
      .spanBuilder("smoke")
      .setAttribute("code.function.name", "smoke")
      .setAttribute("code.file.path", "src/test/scala/timonier/TimonierSmokeSuite.scala")
      // Where `smoke` starts, as the conventions ask.
      .setAttribute("code.line.number", 9L)
      .setAttribute("timonier.test.name", "smoke trace reaches Timonier")
      .startSpan()
      .end()

  test("smoke trace reaches Timonier") {
    assume(System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT") != null, "no run of timonier test")
    val flush = TimonierOtel.start()
    smoke()
    flush()
  }
