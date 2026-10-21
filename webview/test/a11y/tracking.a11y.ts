/**
 * The panels of the tracking (PLAN.md 6.6, ADR 0031): the plan, the todo and the journal in the
 * side bar, read by keyboard, and at WCAG AA in every captured theme.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, panel: string, { theme = "dark-modern", locale = "en", extra = "", width = 300 } = {}) {
  await page.setViewportSize({ width: width + 20, height: 1000 });
  await page.goto(`/demo.html?tracking=${panel}&theme=${theme}&locale=${locale}&width=${width}${extra}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".tim-tracking, .tim-empty");
}

test.describe.configure({ mode: "parallel" });

test("a milestone and a session fold and unfold from the keyboard", async ({ page }) => {
  await open(page, "plan");
  const done = page.locator("details.tim-milestone").first();
  await expect(done).not.toHaveAttribute("open", "");
  await page.keyboard.press("Tab");
  await expect(done.locator("summary")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(done).toHaveAttribute("open", "");
  await expect(page.getByRole("progressbar", { name: /Phase 6 .*: 2 of 5 tasks done/ })).toBeVisible();

  await open(page, "sessions");
  const second = page.locator("details.tim-session").nth(1);
  await expect(second).not.toHaveAttribute("open", "");
  await second.locator("summary").focus();
  await page.keyboard.press("Space");
  await expect(second).toHaveAttribute("open", "");
  await expect(second).toContainText("timonier-daemon → timonier-store");
});

for (const theme of themes) {
  for (const panel of ["plan", "todo", "sessions", "all", "bridge"]) {
    test(`${theme}: the ${panel} panel at WCAG AA`, async ({ page }) => {
      await open(page, panel, { theme, extra: "&problem=1" });
      // Everything unfolded, a summary focused: its ring is measured too.
      await page.evaluate(() => document.querySelectorAll("details").forEach((d) => (d.open = true)));
      if ((await page.locator("summary").count()) > 0) await page.locator("summary").first().focus();
      await settled(page);
      const findings = await axeFindings(page);
      expect(findings.violations).toEqual([]);
      const text = await textSamples(page, ".tim-tracking :is(p, span, li, h3, strong):not([aria-hidden])");
      expect(text.length).toBeGreaterThan(5);
      expect(weak(text, 4.5)).toEqual([]);
      expect(weak(await samples(page))).toEqual([]);
    });
  }
}

test("the columns sit side by side, glued, in a wide side bar, and one under the other in a narrow one", async ({
  page,
}) => {
  const boxes = async () => Promise.all([0, 1].map((i) => page.locator(".tim-tracking__column").nth(i).boundingBox()));
  await open(page, "all", { width: 700 });
  const [left, right] = await boxes();
  expect(left && right).toBeTruthy();
  if (!left || !right) return;
  expect(Math.abs(left.y - right.y)).toBeLessThan(1);
  expect(Math.abs(left.x + left.width - right.x)).toBeLessThan(1);
  await open(page, "all", { width: 300 });
  const [top, bottom] = await boxes();
  if (!top || !bottom) throw new Error("no columns");
  expect(bottom.y).toBeGreaterThanOrEqual(top.y + top.height);
});

test("the bridge's boxes and buttons are reached by keyboard, named, and say what they do", async ({ page }) => {
  await open(page, "bridge");
  await expect(page.getByRole("progressbar", { name: /Phase 6 .*: 2 of 5 tasks done/ })).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Open Map" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Mark as reviewed" })).toBeFocused();
  await page.keyboard.press("Tab");
  const daemon = page.getByRole("button", { name: /timonier-daemon/ });
  await expect(daemon).toBeFocused();
  await expect(daemon).toHaveAccessibleName("timonier-daemon 9 files to read again · untracked");
  await expect(daemon).toHaveAttribute("title", "Shows it on the map");
});

for (const theme of themes) {
  test(`${theme}: the offer to set the project up at WCAG AA, first under the keyboard`, async ({ page }) => {
    await open(page, "bridge", { theme, extra: "&unset=1" });
    await expect(page.getByRole("note")).toContainText("This project is not set up for your agents");
    // The agents to set up first, those found ticked (T-118), then the button.
    await page.keyboard.press("Tab");
    const claude = page.getByRole("checkbox", { name: "Claude Code" });
    await expect(claude).toBeFocused();
    await expect(claude).toBeChecked();
    await expect(page.getByRole("checkbox", { name: /Cursor/ })).not.toBeChecked();
    for (let i = 0; i < 4; i++) await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Set up" })).toBeFocused();
    await settled(page);
    expect((await axeFindings(page)).violations).toEqual([]);
    const text = await textSamples(page, ".tim-bridge__setup :is(p, button, legend, label span)");
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}

test("each panel teaches what fills it, in French too", async ({ page }) => {
  await open(page, "todo", { locale: "fr", extra: "&empty=1" });
  await expect(page.locator(".tim-empty")).toContainText("Aucune session ouverte");
  const findings = await axeFindings(page);
  expect(findings.violations).toEqual([]);
});

/** ADR 0056: the development process's settings, in every theme, by the keyboard. */
for (const theme of themes) {
  test(`${theme}: the development process's settings at WCAG AA, by the keyboard`, async ({ page }) => {
    await open(page, "workflow", { theme, width: 640 });
    await expect(page.getByRole("radio", { name: /Solo, disciplined/ })).toBeChecked();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("radio", { name: /Solo, disciplined/ })).toBeFocused();
    await expect(page.getByRole("checkbox", { name: "A branch per task" })).toBeChecked();
    await expect(page.getByRole("combobox", { name: "CI" })).toHaveValue("github_actions");
    await settled(page);
    expect((await axeFindings(page)).violations).toEqual([]);
    const text = await textSamples(page, ".tim-workflow :is(h1, h2, legend, label span, li)");
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}
