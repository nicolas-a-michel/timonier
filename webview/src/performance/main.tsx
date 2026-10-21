import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { connectHost } from "../host";
import { resolveLocale } from "../i18n";
import { isPerformanceMessage, readyMessage, type PerfRun, type PerfRuns, type RegressionReport } from "../protocol";
import { Performance } from "./Performance";
import "../tokens.css";
import "../design/design.css";
import "./performance.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Timonier webview: the #root element is missing.");
}

const locale = resolveLocale(document.documentElement.lang || navigator.language);
const host = connectHost();
const root = createRoot(container);
// The host sends the runs when the page opens and when a run ends, then the run asked for
// (ADR 0051). Without a host (the development page), nothing to show.
let runs: PerfRuns | null = host ? { runs: [] } : null;
let run: PerfRun | null | undefined;
let chosen: string | null = null;
let regressions: RegressionReport | null | undefined;

const choose = (next: string) => {
  chosen = next;
  run = undefined;
  render();
  host?.postMessage({ type: "chooseRun", run: next });
};

function render() {
  root.render(
    <StrictMode>
      <Performance
        locale={locale}
        runs={runs}
        run={run}
        chosen={chosen}
        onChoose={choose}
        now={Date.now()}
        regressions={regressions}
      />
    </StrictMode>,
  );
}

window.addEventListener("message", (event: MessageEvent<unknown>) => {
  const message = event.data;
  if (!isPerformanceMessage(message)) return;
  if (message.type === "perfRuns") {
    runs = message.runs;
    // The newest run but `doctor`'s smoke tests, until the developer picks another one
    // (relecture n° 3, I-2).
    const newest = (message.runs.runs.find((r) => !r.smoke) ?? message.runs.runs[0])?.run ?? null;
    if (newest && (chosen === null || !message.runs.runs.some((r) => r.run === chosen))) {
      choose(newest);
      return;
    }
    render();
  } else if (message.type === "perfRegressions") {
    regressions = message.report;
    render();
  } else if (message.type === "perfRun") {
    run = message.run;
    render();
  } else if (message.type === "daemon") {
    if (message.state !== "connected") runs = null;
    render();
  }
});

render();
host?.postMessage(readyMessage());
