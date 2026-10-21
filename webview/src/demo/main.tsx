/**
 * The design system's demonstration page (ADR 0020), in development only: `npm run dev -w webview`,
 * then /demo.html. `?theme=<name>` applies the variables VSCode gives a webview in that theme
 * (test/themes, captured by editors/vscode/scripts/capture-themes.mjs); `?locale=fr`, French;
 * `?map=timonier`, the map of a real repository (test/views); `&review=1`, with a review diff
 * (ADR 0024); `&density=compact`, the compact map (Q32); `&intent=1`, compared with a
 * whiteboard (ADR 0042); `?performance=1`, the Performances panel (ADR 0051).
 * The contrast tests (`npm run test:a11y`) read this page in every theme.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { applyDensity } from "../density";
import { resolveLocale } from "../i18n";
import type { ViewSnapshot } from "../protocol";
import { Demo } from "./Demo";
import { MapDemo } from "./MapDemo";
import { synthetic } from "./synthetic";
import "../tokens.css";
import "../design/design.css";
import "../map/map.css";
import "../app.css";
import "./demo.css";

interface Theme {
  bodyClass: string;
  variables: Record<string, string>;
}

const themes = import.meta.glob<Theme>("../../test/themes/*.json", { import: "default" });
/** Views of real repositories (the `view_snapshot` example): `?map=<name>`. */
const views = import.meta.glob<{ snapshot: ViewSnapshot }>("../../test/views/*/open.json", { import: "default" });

/** Applies the theme as VSCode does: variables on <html>, its kind as a class of <body>. */
async function applyTheme(name: string | null): Promise<string[]> {
  const names = Object.keys(themes).map((file) => file.replace(/^.*\//, "").replace(/\.json$/, ""));
  const load = name ? themes[`../../test/themes/${name}.json`] : undefined;
  if (load) {
    const theme = await load();
    for (const [variable, value] of Object.entries(theme.variables)) {
      document.documentElement.style.setProperty(variable, value);
    }
    document.body.className = theme.bodyClass;
  }
  return names.sort();
}

const params = new URLSearchParams(location.search);
const theme = params.get("theme");
const locale = resolveLocale(params.get("locale") ?? "en");
const container = document.getElementById("root");
if (!container) throw new Error("demo: the #root element is missing.");

const map = params.get("map");
/** `?density=compact`: the compact map (Q32), applied as the host's message is. */
const density = applyDensity(params.get("density") === "compact" ? "compact" : "comfortable");
/** `?map=synthetic&size=20x12x12&open=all`: a synthetic repository, everything open. */
const size = (params.get("size") ?? "20x12x12").split("x").map(Number);
const openAll = params.get("open") === "all";
const ready = () => (document.body.dataset.ready = "true");

void applyTheme(theme).then(async (names) => {
  const view =
    map === "synthetic"
      ? () => Promise.resolve({ snapshot: synthetic(size[0] ?? 20, size[1] ?? 12, size[2] ?? 12) })
      : map
        ? views[`../../test/views/${map}/open.json`]
        : undefined;
  // `?performance=1`: the Performances panel on a sample run (ADR 0051); `&empty=1`, before any
  // run; `&tab=tree|evolution`, that tab first. `?map=…&performance=map`: the map's layer.
  if (params.get("performance") === "1") {
    await import("../performance/performance.css");
    const { SAMPLE_RUN, SAMPLE_RUNS, SAMPLE_NOW, SAMPLE_REGRESSIONS } = await import("../performance/sample");
    const { Performance } = await import("../performance/Performance");
    const empty = params.get("empty") === "1";
    const tab = params.get("tab");
    createRoot(container).render(
      <StrictMode>
        <Performance
          locale={locale}
          runs={empty ? { runs: [] } : SAMPLE_RUNS}
          run={empty ? undefined : SAMPLE_RUN}
          chosen={SAMPLE_RUN.summary.run}
          onChoose={() => undefined}
          now={SAMPLE_NOW}
          initialTab={tab === "tree" || tab === "evolution" ? tab : undefined}
          regressions={empty ? undefined : SAMPLE_REGRESSIONS}
        />
      </StrictMode>,
    );
    ready();
    return;
  }
  // `?tracking=plan|todo|sessions`: a panel of the tracking, in the side bar (ADR 0031);
  // `&empty=1`, before anything is tracked; `&problem=1`, with a file that does not read;
  // `&unset=1`, a project not set up yet (Q52).
  const panel = params.get("tracking");
  if (panel) {
    await import("../tracking/tracking.css");
    const { SAMPLE_TRACKING, EMPTY_TRACKING } = await import("../tracking/sample");
    const { TrackingPanel } = await import("../tracking/TrackingPanel");
    const { panelOf } = await import("../tracking/panel");
    const { preset } = await import("../tracking/WorkflowSettings");
    // `tracking=workflow`: the development process's settings (ADR 0056), Solo discipliné.
    const disciplined = preset("solo_disciplined");
    const tracking = params.get("empty") === "1" ? { ...EMPTY_TRACKING } : SAMPLE_TRACKING;
    const problems =
      params.get("problem") === "1" ? [{ file: "plan.json", message: "expected value at line 1 column 3" }] : [];
    // `all` (the default): the view of the secondary side bar (ADR 0031); `&width=` its width.
    const width = Number(params.get("width") ?? "300");
    createRoot(container).render(
      <StrictMode>
        <div className="demo-sidebar" style={{ width: `${width}px` }}>
          <TrackingPanel
            locale={locale}
            panel={panelOf(panel)}
            tracking={{ ...tracking, problems, ...(params.get("unset") === "1" ? { configured: false } : {}) }}
            daemon={{ state: "connected", detail: null }}
            workflow={{
              view: {
                workflow: disciplined,
                instructions:
                  "- Start each task with `task_start`: it opens its ticket and its branch; work and commit on that branch only.\n- Commit messages follow Conventional Commits and end with the task's ticket (`Refs #12`).\n- `session_end` opens the task's pull request; GitHub merges it once the checks pass.",
                problems: [],
              },
              problem: null,
              onSet: () => undefined,
            }}
            agents={
              params.get("unset") === "1"
                ? [
                    { key: "claude", label: "Claude Code", found: true, why: "~/.claude exists" },
                    { key: "copilot", label: "GitHub Copilot (VSCode)", found: true, why: "a .vscode folder" },
                    { key: "cursor", label: "Cursor", found: false, why: null },
                    { key: "codex", label: "Codex", found: false, why: null },
                  ]
                : undefined
            }
            actions={{
              onReveal: () => undefined,
              onOpenMap: () => undefined,
              onMarkReviewed: () => undefined,
              onConfigure: () => undefined,
            }}
          />
        </div>
      </StrictMode>,
    );
    ready();
    return;
  }
  const content = view ? (
    <MapDemo
      locale={locale}
      open={(await view()).snapshot}
      openAll={openAll}
      review={params.get("review") === "1"}
      only={params.get("only") === "1"}
      debt={params.get("debt") === "1"}
      intent={params.get("intent") === "1"}
      performance={params.get("performance") === "map"}
      budget={params.get("budget") === "1"}
      density={density}
      onRendered={ready}
    />
  ) : (
    <Demo locale={locale} theme={theme} themes={names} onRendered={ready} />
  );
  createRoot(container).render(<StrictMode>{content}</StrictMode>);
});
