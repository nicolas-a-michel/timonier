/** The development process's settings (ADR 0056). */
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { trackingMessages } from "./messages";
import { preset, WorkflowSettings } from "./WorkflowSettings";

describe("the development process's settings", () => {
  it("are the presets of the daemon", () => {
    const fast = preset("solo_fast");
    expect(fast.branches.per_task).toBe(false);
    expect(fast.ci.provider).toBe("none");
    const disciplined = preset("solo_disciplined");
    expect([disciplined.merge.auto, disciplined.merge.human_approval]).toEqual([true, false]);
    expect(preset("team").merge.human_approval).toBe(true);
  });

  it("show the preset chosen, each setting, what the agents do, and a refusal", () => {
    const html = renderToStaticMarkup(
      <WorkflowSettings
        t={trackingMessages.fr}
        view={{
          workflow: preset("solo_disciplined"),
          instructions: "- Start each task with `task_start`.",
          problems: [],
        }}
        problem="auto-merge and a required human approval exclude each other"
      />,
    );
    expect(html).toMatch(/<input type="radio" name="preset" checked=""\/><span><strong>Solo discipliné<\/strong>/);
    expect(html).toContain("Une branche par tâche");
    expect(html).toMatch(/<option value="github_actions" selected="">GitHub Actions<\/option>/);
    expect(html).toContain("Non enregistré : auto-merge and a required human approval exclude each other.");
    expect(html).toContain("Start each task with `task_start`.");
  });

  it("say a custom process, and teach while the daemon is not there", () => {
    const custom = { ...preset("solo_disciplined"), preset: "custom" as const };
    const html = renderToStaticMarkup(
      <WorkflowSettings
        t={trackingMessages.en}
        view={{ workflow: custom, instructions: "", problems: [] }}
        problem={null}
      />,
    );
    expect(html).toContain("Custom: one setting differs from the presets.");
    expect(
      renderToStaticMarkup(<WorkflowSettings t={trackingMessages.en} view={undefined} problem={null} />),
    ).toContain("The process appears once the daemon answers.");
  });
});
