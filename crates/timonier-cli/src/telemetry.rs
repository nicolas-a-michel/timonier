//! Timonier's own logs and traces (ADR 0044): logs on standard error, filtered by `RUST_LOG`;
//! with the `otel` feature, the spans of Timonier's crates exported over OTLP when
//! `OTEL_EXPORTER_OTLP_ENDPOINT` is set (as `timonier test` sets it) and `OTEL_SDK_DISABLED`
//! is not `true`: the Rust SDK does not read the latter, Timonier does.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Ends the export when dropped: the spans still queued are sent first.
pub struct Telemetry {
    #[cfg(feature = "otel")]
    export: Option<otel::Export>,
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        #[cfg(feature = "otel")]
        if let Some(export) = self.export.take() {
            export.shutdown();
        }
    }
}

/// Whether the environment asks for the export, as OpenTelemetry's variables say.
#[cfg(any(feature = "otel", test))]
pub fn exporting(endpoint: Option<&str>, disabled: Option<&str>) -> bool {
    endpoint.is_some_and(|e| !e.trim().is_empty())
        && !disabled.is_some_and(|d| d.trim().eq_ignore_ascii_case("true"))
}

/// The test cargo-nextest runs in this process, or in the one that launched it, as resource
/// attributes: they name the test of the spans for Timonier (ADR 0047), as the Rust pack's
/// module does.
#[cfg(any(feature = "otel", test))]
pub fn test_attributes(var: impl Fn(&str) -> Option<String>) -> Vec<(&'static str, String)> {
    [
        ("NEXTEST_TEST_NAME", "timonier.test.name"),
        ("NEXTEST_BINARY_ID", "timonier.test.classname"),
    ]
    .into_iter()
    .filter_map(|(variable, key)| var(variable).map(|value| (key, value)))
    .collect()
}

/// Installs the logs, at `level` unless `RUST_LOG` says otherwise, and the export when asked.
/// `ansi`: colours, on a terminal.
pub fn install(level: &str, ansi: bool) -> Telemetry {
    let logs = tracing_subscriber::fmt::layer()
        .with_writer(|| Stderr)
        .with_ansi(ansi)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)));
    let registry = tracing_subscriber::registry().with(logs);
    #[cfg(feature = "otel")]
    {
        let wanted = exporting(
            std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().as_deref(),
            std::env::var("OTEL_SDK_DISABLED").ok().as_deref(),
        );
        let export = wanted.then(otel::Export::start).flatten();
        let layer = export.as_ref().map(otel::Export::layer);
        let _ = registry.with(layer).try_init();
        Telemetry { export }
    }
    #[cfg(not(feature = "otel"))]
    {
        let _ = registry.try_init();
        Telemetry {}
    }
}

/// Standard error, for the log, that never fails. The process that launched the daemon may read
/// it, then go: the extension's window closed. The daemon outlives it (ADR 0019), its lines lost
/// since; a failed write would otherwise make `tracing-subscriber` panic in `eprintln!`.
struct Stderr;

impl std::io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        Ok(())
    }
}

#[cfg(feature = "otel")]
mod otel {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::Layer;
    use tracing_subscriber::filter::Targets;

    /// The exporter, and the runtime its gRPC client needs.
    pub struct Export {
        provider: SdkTracerProvider,
        runtime: tokio::runtime::Runtime,
    }

    impl Export {
        /// `None` when the exporter cannot be made: Timonier runs without traces then.
        pub fn start() -> Option<Self> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("timonier-otel")
                .enable_all()
                .build()
                .ok()?;
            let provider = runtime.block_on(async {
                let exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .build()
                    .ok()?;
                Some(
                    SdkTracerProvider::builder()
                        .with_batch_exporter(exporter)
                        .with_resource(
                            opentelemetry_sdk::Resource::builder()
                                .with_service_name("timonier")
                                .with_attributes(
                                    super::test_attributes(|v| std::env::var(v).ok())
                                        .into_iter()
                                        .map(|(key, value)| {
                                            opentelemetry::KeyValue::new(key, value)
                                        }),
                                )
                                .build(),
                        )
                        .build(),
                )
            })?;
            Some(Self { provider, runtime })
        }

        /// The spans of Timonier's own crates, at INFO and above; nothing of the libraries.
        pub fn layer<S>(&self) -> impl Layer<S>
        where
            S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
        {
            let targets = Targets::new().with_targets(
                [
                    "timonier_core",
                    "timonier_store",
                    "timonier_index",
                    "timonier_otlp",
                    "timonier_daemon",
                    "timonier_mcp",
                    "timonier_cli",
                ]
                .map(|target| (target, tracing::Level::INFO)),
            );
            tracing_opentelemetry::layer()
                .with_tracer(self.provider.tracer("timonier"))
                .with_filter(targets)
        }

        pub fn shutdown(self) {
            let _ = self.provider.shutdown();
            drop(self.runtime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_follows_opentelemetry_s_variables() {
        assert!(exporting(Some("http://127.0.0.1:4318"), None));
        assert!(exporting(Some("http://127.0.0.1:4318"), Some("false")));
        assert!(!exporting(Some("http://127.0.0.1:4318"), Some("TRUE")));
        assert!(!exporting(None, None));
        assert!(!exporting(Some(" "), None));
    }

    #[test]
    fn the_test_nextest_runs_names_the_spans() {
        let var = |name: &str| match name {
            "NEXTEST_TEST_NAME" => Some("a_test".to_owned()),
            "NEXTEST_BINARY_ID" => Some("timonier-cli::mcp".to_owned()),
            _ => None,
        };
        assert_eq!(
            test_attributes(var),
            [
                ("timonier.test.name", "a_test".to_owned()),
                ("timonier.test.classname", "timonier-cli::mcp".to_owned())
            ]
        );
        assert!(test_attributes(|_| None).is_empty());
    }
}
