//! A test process with real OpenTelemetry instrumentation, for `timonier test`'s tests (PLAN.md
//! 8.4): it exports over OTLP/gRPC to where `OTEL_EXPORTER_OTLP_ENDPOINT` says, with the
//! resource `OTEL_RESOURCE_ATTRIBUTES` gives, the spans of the fixture's test `parses`.

use opentelemetry::KeyValue;
use opentelemetry::trace::{Span, TraceContextExt, Tracer, TracerProvider};

fn main() {
    // The Rust SDK does not read OTEL_SDK_DISABLED (0.33.1): the conventions' initialisation
    // does, as here (ADR 0049).
    if std::env::var("OTEL_SDK_DISABLED").is_ok_and(|v| v.eq_ignore_ascii_case("true")) {
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let provider = runtime.block_on(async {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .expect("an exporter");
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(opentelemetry_sdk::Resource::builder().build())
            .build()
    });
    let tracer = provider.tracer("fixture");
    let test = KeyValue::new("timonier.test.name", "parses");
    // Its function is an ID of the graph.
    // Spans without their code: what the gaps report (ADR 0049).
    tracer.in_span("bare", |_| {});
    tracer
        .span_builder("file only")
        .with_attributes([KeyValue::new("code.file.path", "src/lib.rs")])
        .start(&tracer)
        .end();
    tracer.in_span("load", |context| {
        context
            .span()
            .set_attributes([test.clone(), KeyValue::new("code.function.name", "demo::f")]);
        // `f` slowed on purpose, for the regression tests (ADR 0053).
        if let Some(ms) = std::env::var("TIMONIER_FIXTURE_SLOW_MS")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        // A method: its module does not say its type, its name does in the file.
        tracer
            .span_builder("draw")
            .with_attributes([
                test.clone(),
                KeyValue::new("code.function.name", "demo::Thing::g"),
                KeyValue::new("code.file.path", "src/lib.rs"),
            ])
            .start_with_context(&tracer, &context)
            .end();
        // A function the graph does not know: its line, in its file.
        tracer
            .span_builder("inline")
            .with_attributes([
                test.clone(),
                KeyValue::new("code.function.name", "demo::inlined"),
                KeyValue::new("code.file.path", "src/lib.rs"),
                KeyValue::new("code.line.number", 2_i64),
            ])
            .start_with_context(&tracer, &context)
            .end();
    });
    provider.shutdown().expect("the spans sent");
}
