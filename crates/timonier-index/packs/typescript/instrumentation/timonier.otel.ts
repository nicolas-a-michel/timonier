// OpenTelemetry for this package, written by `timonier init` (Timonier's TypeScript
// conventions, in `.timonier/conventions/typescript/`). The SDK starts when a run of
// `timonier test` asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES`, and
// stays off with `OTEL_SDK_DISABLED=true`.
import { NodeSDK } from "@opentelemetry/sdk-node";

/** Starts the export when a run asks for it; returns what flushes and stops it. */
export function startTelemetry(): () => Promise<void> {
  if (!process.env.OTEL_EXPORTER_OTLP_ENDPOINT) return () => Promise.resolve();
  // Timonier takes traces and metrics, not logs.
  process.env.OTEL_LOGS_EXPORTER ??= "none";
  const sdk = new NodeSDK();
  sdk.start();
  return () => sdk.shutdown();
}
