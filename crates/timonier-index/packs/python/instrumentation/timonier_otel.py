"""OpenTelemetry for this project, written by `timonier init` (Timonier's Python conventions, in
`.timonier/conventions/python/`). The SDK starts when a run of `timonier test` asks
(`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and `OTEL_EXPORTER_OTLP_HEADERS`,
and stays off with `OTEL_SDK_DISABLED=true`."""

import os


def start_telemetry():
    """Starts the export when a run asks for it; returns what flushes and stops it."""
    if not os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT"):
        return lambda: None
    if os.environ.get("OTEL_SDK_DISABLED", "").lower() == "true":
        return lambda: None
    from opentelemetry import trace
    from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor

    provider = TracerProvider(resource=Resource.create())
    provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
    trace.set_tracer_provider(provider)
    return provider.shutdown
