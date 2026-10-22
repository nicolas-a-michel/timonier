/** The Performances panel's rendering (PLAN.md 8.7, ADR 0051). */
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { performanceMessages } from "./messages";
import { Performance } from "./Performance";
import { SAMPLE_NOW, SAMPLE_REGRESSIONS, SAMPLE_RUN, SAMPLE_RUNS } from "./sample";

describe("the Performances panel", () => {
  it("says a failed test and a span in error in shape and word, not in colour alone", () => {
    const html = renderToStaticMarkup(
      <Performance
        locale="en"
        runs={SAMPLE_RUNS}
        run={SAMPLE_RUN}
        chosen={SAMPLE_RUN.summary.run}
        onChoose={() => undefined}
        now={SAMPLE_NOW}
      />,
    );
    // renders_the_view failed: a badge with its icon and the word.
    expect(html).toMatch(
      /renders_the_view <span class="tim-badge tim-badge--violation"[^>]*><svg[\s\S]*?<span>failed<\/span>/,
    );
    expect(performanceMessages.en.barTitle("load", "300 ms", "load", true)).toBe("load · 300 ms · load · error");
    expect(performanceMessages.fr.spanError).toBe("erreur");
  });

  it("says why nothing was compared in the page's language (relecture n° 1, m1)", () => {
    const html = renderToStaticMarkup(
      <Performance
        locale="fr"
        runs={SAMPLE_RUNS}
        run={SAMPLE_RUN}
        chosen={SAMPLE_RUN.summary.run}
        onChoose={() => undefined}
        now={SAMPLE_NOW}
        initialTab="evolution"
        regressions={{
          ...SAMPLE_REGRESSIONS,
          regressions: [],
          note: "no reference yet",
          noteKind: "no_reference",
          main: "main",
        }}
      />,
    );
    expect(html).toContain("Pas encore de référence : lancez d&#x27;abord les mêmes tests sur `main`.");
    expect(html).not.toContain("no reference yet");
  });

  it("opens on the newest run but doctor's smoke tests, which the list names (relecture n° 4, m-1)", () => {
    const smoke = { ...SAMPLE_RUN.summary, run: "R-13", smoke: true, command: ["sh", "-c", "smoke"] };
    const html = renderToStaticMarkup(
      <Performance
        locale="en"
        runs={{ runs: [smoke, ...SAMPLE_RUNS.runs] }}
        run={null}
        chosen={null}
        onChoose={() => undefined}
        now={SAMPLE_NOW}
      />,
    );
    expect(html).toMatch(/<option value="R-13">[^<]*smoke test of doctor<\/option>/);
    expect(html).toMatch(/<option value="R-12" selected="">/);
  });
});

describe("the Performances panel before any run", () => {
  it("offers to run the tests, and says the commands show first", () => {
    const empty = { runs: [] };
    const html = renderToStaticMarkup(
      <Performance
        locale="en"
        runs={empty}
        run={null}
        chosen={null}
        onChoose={() => undefined}
        onRunTests={() => undefined}
        now={SAMPLE_NOW}
      />,
    );
    expect(html).toContain("No run yet");
    expect(html).toContain(">Run the tests</button>");
    expect(html).toContain("You see the commands before they run.");
    const fr = renderToStaticMarkup(
      <Performance
        locale="fr"
        runs={empty}
        run={null}
        chosen={null}
        onChoose={() => undefined}
        onRunTests={() => undefined}
        now={SAMPLE_NOW}
      />,
    );
    expect(fr).toContain(">Lancer les tests</button>");
    expect(performanceMessages.fr.runTests).toBe("Lancer les tests");
  });
});
