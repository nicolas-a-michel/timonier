// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's Scala
// conventions, in `.timonier/conventions/scala/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
package timonier

import io.opentelemetry.sdk.OpenTelemetrySdk
import io.opentelemetry.sdk.autoconfigure.AutoConfiguredOpenTelemetrySdk

import java.util.concurrent.TimeUnit

import scala.jdk.CollectionConverters.*

object TimonierOtel:
  private var sdk: Option[OpenTelemetrySdk] = None

  /** Starts the export once, when a run asks for it; returns what flushes it, and waits: sbt
    * runs the tests in its own JVM and closes their classes as soon as they end.
    */
  def start(): () => Unit = synchronized {
    val endpoint = Option(System.getenv("OTEL_EXPORTER_OTLP_ENDPOINT")).filter(_.nonEmpty)
    val disabled = Option(System.getenv("OTEL_SDK_DISABLED")).exists(_.equalsIgnoreCase("true"))
    if endpoint.isEmpty || disabled then () => ()
    else
      val started = sdk.getOrElse {
        val built = AutoConfiguredOpenTelemetrySdk
          .builder()
          // Defaults only: the run's variables come first.
          .addPropertiesSupplier(() =>
            Map(
              "otel.exporter.otlp.protocol" -> "http/protobuf",
              "otel.metrics.exporter" -> "none",
              "otel.logs.exporter" -> "none"
            ).asJava
          )
          .setResultAsGlobal()
          .build()
          .getOpenTelemetrySdk
        sdk = Some(built)
        built
      }
      () => { started.getSdkTracerProvider.forceFlush().join(10, TimeUnit.SECONDS); () }
  }
