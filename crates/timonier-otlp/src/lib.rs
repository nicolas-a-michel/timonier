//! Timonier's local OpenTelemetry receiver (OTLP over gRPC 4317 and HTTP 4318).
//!
//! See the `otel-instrumentation` skill and ADR 0048.

pub mod decode;
mod receiver;

pub use receiver::{Receiver, Sink, TOKEN_HEADER};

/// OTLP's default HTTP port (spec « Performance »).
pub const HTTP_PORT: u16 = 4318;
/// OTLP's default gRPC port.
pub const GRPC_PORT: u16 = 4317;

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
