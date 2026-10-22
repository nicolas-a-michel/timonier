// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// package's OpenTelemetry module, `timonier.otel.mjs`, and runs with Node's own test runner.
import { after, test } from "node:test";

import { trace } from "@opentelemetry/api";

import { startTelemetry } from "./timonier.otel.mjs";

const stop = startTelemetry();
after(() => stop());

function smoke() {
  trace.getTracer("timonier-smoke").startActiveSpan(
    "smoke",
    {
      attributes: {
        "code.function.name": "smoke",
        "code.file.path": "timonier.smoke.test.mjs",
        // Where `smoke` starts, as the conventions ask.
        "code.line.number": 13,
        "timonier.test.name": "timonier smoke",
      },
    },
    (span) => span.end(),
  );
}

test("timonier smoke", { skip: !process.env.OTEL_EXPORTER_OTLP_ENDPOINT }, () => {
  smoke();
});
