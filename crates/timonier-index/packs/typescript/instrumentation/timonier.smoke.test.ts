// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// package's OpenTelemetry module, `timonier.otel.ts`.
import { trace } from "@opentelemetry/api";
import { afterAll, test } from "vitest";

import { startTelemetry } from "./timonier.otel";

const stop = startTelemetry();
afterAll(() => stop());

function smoke(): void {
  trace.getTracer("timonier-smoke").startActiveSpan(
    "smoke",
    {
      attributes: {
        "code.function.name": "smoke",
        "code.file.path": "timonier.smoke.test.ts",
        // Where `smoke` starts, as the conventions ask.
        "code.line.number": 12,
        "timonier.test.name": "timonier smoke",
      },
    },
    (span) => span.end(),
  );
}

test.skipIf(!process.env.OTEL_EXPORTER_OTLP_ENDPOINT)("timonier smoke", () => {
  smoke();
});
