// The C API of `timonier_otel.h` on opentelemetry-cpp, written by `timonier init`.
#include "timonier_otel.h"

#include <cstdlib>
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
#include "opentelemetry/trace/provider.h"

namespace trace_api = opentelemetry::trace;
namespace trace_sdk = opentelemetry::sdk::trace;

struct timonier_span {
    opentelemetry::nostd::shared_ptr<trace_api::Span> span;
};

namespace {
std::shared_ptr<trace_sdk::TracerProvider> provider;
}

extern "C" void timonier_start_telemetry(void) {
    const char* endpoint = std::getenv("OTEL_EXPORTER_OTLP_ENDPOINT");
    if (endpoint == nullptr || *endpoint == '\0' || provider) {
        return;
    }
    const char* disabled = std::getenv("OTEL_SDK_DISABLED");
    if (disabled != nullptr && std::string(disabled) == "true") {
        return;
    }
    namespace otlp = opentelemetry::exporter::otlp;
    // The options read the endpoint and the headers from the environment.
    otlp::OtlpHttpExporterOptions options;
    auto processor = trace_sdk::BatchSpanProcessorFactory::Create(
        otlp::OtlpHttpExporterFactory::Create(options), trace_sdk::BatchSpanProcessorOptions{});
    provider = trace_sdk::TracerProviderFactory::Create(
        std::move(processor), opentelemetry::sdk::resource::Resource::Create({}));
    std::shared_ptr<trace_api::TracerProvider> global = provider;
    trace_sdk::Provider::SetTracerProvider(global);
}

extern "C" void timonier_stop_telemetry(void) {
    if (provider) {
        provider->ForceFlush();
        provider->Shutdown();
    }
}

extern "C" timonier_span* timonier_span_start(const char* name, const char* function, const char* file, int line) {
    auto tracer = trace_api::Provider::GetTracerProvider()->GetTracer("timonier");
    auto span = tracer->StartSpan(name, {{"code.function.name", function},
                                         {"code.file.path", file},
                                         {"code.line.number", line}});
    return new timonier_span{span};
}

extern "C" void timonier_span_set(timonier_span* span, const char* key, const char* value) {
    if (span != nullptr) {
        span->span->SetAttribute(key, value);
    }
}

extern "C" void timonier_span_end(timonier_span* span) {
    if (span != nullptr) {
        span->span->End();
        delete span;
    }
}
