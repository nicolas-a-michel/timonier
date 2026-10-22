// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's Java
// conventions, in `.timonier/conventions/java/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
package timonier;

import io.opentelemetry.sdk.OpenTelemetrySdk;
import io.opentelemetry.sdk.autoconfigure.AutoConfiguredOpenTelemetrySdk;
import java.util.Map;

public final class TimonierOtel {
    private static OpenTelemetrySdk sdk;

    private TimonierOtel() {}

    /** Starts the export once, when a run asks for it; returns what flushes and stops it. */
    public static synchronized Runnable start() {
        String endpoint = System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT");
        if (endpoint == null || endpoint.isEmpty()) {
            return () -> {};
        }
        if ("true".equalsIgnoreCase(System.getenv("OTEL_SDK_DISABLED"))) {
            return () -> {};
        }
        if (sdk == null) {
            sdk = AutoConfiguredOpenTelemetrySdk.builder()
                    // Defaults only: the run's variables come first.
                    .addPropertiesSupplier(() -> Map.of(
                            "otel.exporter.otlp.protocol", "http/protobuf",
                            "otel.metrics.exporter", "none",
                            "otel.logs.exporter", "none"))
                    .setResultAsGlobal()
                    .build()
                    .getOpenTelemetrySdk();
        }
        OpenTelemetrySdk started = sdk;
        return () -> started.getSdkTracerProvider().forceFlush();
    }
}
