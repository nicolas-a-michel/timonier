//! OpenTelemetry for this crate, written by `timonier init` (Timonier's Rust conventions, in
//! `.timonier/conventions/rust/`). The spans of `tracing` go out over OTLP when
//! `OTEL_EXPORTER_OTLP_ENDPOINT` is set, as `timonier test` sets it, unless
//! `OTEL_SDK_DISABLED=true` (the Rust SDK does not read it: this module does).
//!
//! Behind the feature `otel`: declare it with `#[cfg(feature = "otel")] mod timonier_otel;`,
//! then keep `let _otel = timonier_otel::init();` alive from the start of `main`, or of a test.

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The export: dropping it sends the spans still queued.
pub struct Otel {
    provider: SdkTracerProvider,
    _runtime: tokio::runtime::Runtime,
}

impl Drop for Otel {
    fn drop(&mut self) {
        let _ = self.provider.shutdown();
    }
}

/// Starts the export when a run asks for it; `None` otherwise, or if it cannot start.
pub fn init() -> Option<Otel> {
    let asked = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok_and(|e| !e.trim().is_empty());
    let disabled =
        std::env::var("OTEL_SDK_DISABLED").is_ok_and(|v| v.trim().eq_ignore_ascii_case("true"));
    if !asked || disabled {
        return None;
    }
    // The gRPC exporter needs a Tokio runtime of its own.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .ok()?;
    let provider = {
        // The exporter's client spawns on this runtime.
        let _entered = runtime.enter();
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .ok()?;
        // The test cargo-nextest runs in this process names the spans' test for Timonier.
        let mut resource = opentelemetry_sdk::Resource::builder();
        for (variable, key) in [
            ("NEXTEST_TEST_NAME", "timonier.test.name"),
            ("NEXTEST_BINARY_ID", "timonier.test.classname"),
        ] {
            if let Ok(value) = std::env::var(variable) {
                resource = resource.with_attribute(KeyValue::new(key, value));
            }
        }
        SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource.build())
            .build()
    };
    let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer(env!("CARGO_PKG_NAME")));
    let _ = tracing_subscriber::registry().with(layer).try_init();
    Some(Otel {
        provider,
        _runtime: runtime,
    })
}
