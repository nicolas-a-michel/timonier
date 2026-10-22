// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's Kotlin
// conventions, in `.timonier/conventions/kotlin/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
package timonier

import io.opentelemetry.sdk.OpenTelemetrySdk
import io.opentelemetry.sdk.autoconfigure.AutoConfiguredOpenTelemetrySdk

object TimonierOtel {
    private var sdk: OpenTelemetrySdk? = null

    /** Starts the export once, when a run asks for it; returns what flushes it. */
    @Synchronized
    fun start(): () -> Unit {
        val endpoint = System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT")
        if (endpoint.isNullOrEmpty()) return {}
        if (System.getenv("OTEL_SDK_DISABLED").equals("true", ignoreCase = true)) return {}
        val started = sdk ?: AutoConfiguredOpenTelemetrySdk.builder()
            // Defaults only: the run's variables come first.
            .addPropertiesSupplier {
                mapOf(
                    "otel.exporter.otlp.protocol" to "http/protobuf",
                    "otel.metrics.exporter" to "none",
                    "otel.logs.exporter" to "none",
                )
            }
            .setResultAsGlobal()
            .build()
            .openTelemetrySdk
            .also { sdk = it }
        return { started.sdkTracerProvider.forceFlush() }
    }
}
