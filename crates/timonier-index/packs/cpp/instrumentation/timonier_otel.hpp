// OpenTelemetry for this project, written by `timonier init` (Timonier's C++ conventions, in
// `.timonier/conventions/cpp/`). The SDK starts when a run of `timonier test` asks
// (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
#pragma once

#include <cstdlib>
#include <functional>
#include <memory>
#include <string>

#include "opentelemetry/exporters/otlp/otlp_http_exporter_factory.h"
#include "opentelemetry/exporters/otlp/otlp_http_exporter_options.h"
#include "opentelemetry/sdk/resource/resource.h"
#include "opentelemetry/sdk/trace/batch_span_processor_factory.h"
#include "opentelemetry/sdk/trace/batch_span_processor_options.h"
#include "opentelemetry/sdk/trace/provider.h"
#include "opentelemetry/sdk/trace/tracer_provider.h"
#include "opentelemetry/sdk/trace/tracer_provider_factory.h"

namespace timonier {

// Starts the export when a run asks for it; returns what flushes and stops it.
inline std::function<void()> start_telemetry() {
    const char* endpoint = std::getenv("OTEL_EXPORTER_OTLP_ENDPOINT");
    if (endpoint == nullptr || *endpoint == '\0') {
        return [] {};
    }
    const char* disabled = std::getenv("OTEL_SDK_DISABLED");
    if (disabled != nullptr && std::string(disabled) == "true") {
        return [] {};
    }
    namespace otlp = opentelemetry::exporter::otlp;
    namespace trace_sdk = opentelemetry::sdk::trace;
    // The options read the endpoint and the headers from the environment.
    otlp::OtlpHttpExporterOptions options;
    auto processor = trace_sdk::BatchSpanProcessorFactory::Create(
        otlp::OtlpHttpExporterFactory::Create(options), trace_sdk::BatchSpanProcessorOptions{});
    std::shared_ptr<trace_sdk::TracerProvider> provider = trace_sdk::TracerProviderFactory::Create(
        std::move(processor), opentelemetry::sdk::resource::Resource::Create({}));
    std::shared_ptr<opentelemetry::trace::TracerProvider> global = provider;
    trace_sdk::Provider::SetTracerProvider(global);
    return [provider] {
        provider->ForceFlush();
        provider->Shutdown();
    };
}

}  // namespace timonier
