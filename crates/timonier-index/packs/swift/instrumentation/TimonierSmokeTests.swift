// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.
import Foundation
import OpenTelemetryApi
import XCTest

final class TimonierSmokeTests: XCTestCase {
    func smoke() {
        let span = OpenTelemetry.instance.tracerProvider.get(instrumentationName: "timonier-smoke", instrumentationVersion: nil)
            .spanBuilder(spanName: "smoke")
            .setAttribute(key: "code.function.name", value: "smoke")
            .setAttribute(key: "code.file.path", value: "Tests/TimonierTests/TimonierSmokeTests.swift")
            // Where `smoke` starts, as the conventions ask.
            .setAttribute(key: "code.line.number", value: 9)
            .setAttribute(key: "timonier.test.name", value: "testSmokeTraceReachesTimonier")
            .startSpan()
        span.end()
    }

    func testSmokeTraceReachesTimonier() throws {
        try XCTSkipIf(ProcessInfo.processInfo.environment["OTEL_EXPORTER_OTLP_ENDPOINT"] == nil, "no run of timonier test")
        let stop = TimonierOtel.start()
        smoke()
        stop()
    }
}
