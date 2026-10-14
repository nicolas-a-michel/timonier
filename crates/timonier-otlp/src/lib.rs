//! Timonier's local OpenTelemetry receiver (OTLP over gRPC 4317 and HTTP 4318).
//!
//! See the `otel-instrumentation` skill.

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
