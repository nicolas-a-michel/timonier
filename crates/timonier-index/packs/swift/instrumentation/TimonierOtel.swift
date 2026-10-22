// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's Swift
// conventions, in `.timonier/conventions/swift/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
import Foundation
import OpenTelemetryApi
import OpenTelemetryProtocolExporterHttp
import OpenTelemetrySdk

public enum TimonierOtel {
    /// Starts the export when a run asks for it; returns what flushes and stops it.
    public static func start() -> () -> Void {
        let environment = ProcessInfo.processInfo.environment
        guard let endpoint = environment["OTEL_EXPORTER_OTLP_ENDPOINT"], !endpoint.isEmpty,
              environment["OTEL_SDK_DISABLED"]?.lowercased() != "true",
              let url = URL(string: endpoint.hasSuffix("/") ? endpoint + "v1/traces" : endpoint + "/v1/traces")
        else {
            return {}
        }
        // The exporter reads the headers from the environment; the resource reads its attributes.
        let exporter = OtlpHttpTraceExporter(endpoint: url)
        let provider = TracerProviderBuilder()
            .add(spanProcessor: BatchSpanProcessor(spanExporter: exporter))
            .with(resource: EnvVarResource.get())
            .build()
        OpenTelemetry.registerTracerProvider(tracerProvider: provider)
        return {
            provider.forceFlush()
            provider.shutdown()
        }
    }
}
