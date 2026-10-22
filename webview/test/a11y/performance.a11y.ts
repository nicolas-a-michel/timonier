/**
 * The Performances panel and the map's layer (PLAN.md 8.7, ADR 0051): read by keyboard, the
 * curves said in words, and at WCAG AA in every captured theme.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, { theme = "dark-modern", locale = "en", extra = "" } = {}) {
  await page.setViewportSize({ width: 1200, height: 900 });
  await page.goto(`/demo.html?performance=1&theme=${theme}&locale=${locale}${extra}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".tim-perf, .tim-empty");
}

test.describe.configure({ mode: "parallel" });

test("the tabs move with the arrows, the tests and the tree's levels with the keyboard", async ({ page }) => {
  await open(page);
  await page.getByRole("tab", { name: "Timeline" }).focus();
  await page.keyboard.press("ArrowRight");
  const tree = page.getByRole("tab", { name: "Call tree" });
  await expect(tree).toBeFocused();
  await expect(tree).toHaveAttribute("aria-selected", "true");
  const grid = page.getByRole("treegrid", { name: "Call tree of the run" });
  await expect(grid).toBeVisible();
  const parses = grid.getByRole("button", { name: "parses_a_module" });
  await parses.focus();
  await page.keyboard.press("Enter");
  await expect(parses).toHaveAttribute("aria-expanded", "true");
  await expect(grid.getByRole("button", { name: "parse", exact: true })).toBeVisible();
  // The tests of the timeline: each named with its state and its time.
  await page.getByRole("tab", { name: "Call tree" }).focus();
  await page.keyboard.press("ArrowLeft");
  // Its state in a visible word after its name (relecture n° 1, I2), in the name too (WCAG 2.5.3).
  const failed = page.getByRole("button", { name: /^renders_the_view failed ?420 ms/ });
  await failed.focus();
  await page.keyboard.press("Enter");
  await expect(failed).toHaveAttribute("aria-current", "true");
  await expect(page.getByRole("img", { name: "Spans of renders_the_view" })).toBeVisible();
});

test("the curves are said in a table, for screen readers", async ({ page }) => {
  await open(page, { extra: "&tab=evolution" });
  const chart = page.getByRole("img", { name: "Duration and CPU time of the runs" });
  await expect(chart).toBeVisible();
  const table = page.getByRole("table", { name: "Duration and CPU time of the runs" });
  // A heading and a point per commit: two of the six runs share 51d0e9c (relecture n° 1, I1).
  await expect(table.getByRole("row")).toHaveCount(6);
  await expect(table.getByRole("row").last()).toContainText("9f4c2ab");
});

/** PLAN.md 8.9, ADR 0053: the regressions of the latest batch, their state in shape and words. */
test("the evolution lists the latest batch's regressions, probable and confirmed", async ({ page }) => {
  await open(page, { extra: "&tab=evolution" });
  const list = page.getByRole("region", { name: "Regressions of the last batch, against the main branch" });
  await expect(list).toContainText("Confirmed");
  await expect(list).toContainText("parse: +36.8 % (p = 0.0039)");
  await expect(list).toContainText("Probable");
  await expect(list).toContainText("test renders_the_view: +16.7 %");
});

test("before any run, the panel offers to run the tests, by keyboard too", async ({ page }) => {
  await open(page, { extra: "&empty=1" });
  await expect(page.getByText("No run yet")).toBeVisible();
  const run = page.getByRole("button", { name: "Run the tests" });
  await expect(run).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(run).toBeFocused();
});

for (const theme of themes) {
  test(`${theme}: the panel before any run at WCAG AA`, async ({ page }) => {
    await open(page, { theme, extra: "&empty=1" });
    await expect(page.getByRole("button", { name: "Run the tests" })).toBeVisible();
    expect((await axeFindings(page)).violations).toEqual([]);
  });
}

for (const theme of themes) {
  for (const tab of ["timeline", "tree", "evolution"]) {
    test(`${theme}: the ${tab} at WCAG AA`, async ({ page }) => {
      await open(page, { theme, extra: `&tab=${tab}` });
      if (tab === "tree") {
        for (const level of await page.locator(".tim-calltree__toggle").all()) await level.click();
      }
      await page.locator(".tim-perf__tab[aria-selected='true']").focus();
      await settled(page);
      const findings = await axeFindings(page);
      expect(findings.violations).toEqual([]);
      const text = await textSamples(page, ".tim-perf :is(p, span, dt, dd, th, td, button, h1, h2):not([aria-hidden])");
      expect(text.length).toBeGreaterThan(5);
      expect(weak(text, 4.5)).toEqual([]);
      expect(weak(await samples(page))).toEqual([]);
    });
  }
  test(`${theme}: the map's Performances layer at WCAG AA`, async ({ page }) => {
    await page.setViewportSize({ width: 1500, height: 950 });
    await page.goto(`/demo.html?map=timonier&performance=map&theme=${theme}`);
    await page.waitForSelector("body[data-ready='true']", { state: "attached" });
    await page.waitForSelector(".tim-perf-words");
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    const text = await textSamples(page, ".tim-perf-words span, .tim-review--perf :is(p, strong, button)");
    expect(text.length).toBeGreaterThan(2);
    expect(weak(text, 4.5)).toEqual([]);
  });
}

/** PLAN.md 8.8, ADR 0052: a budget the last runs exceed marks its box, always, in shape and words. */
test("an exceeded budget marks its box on the map, said in its name", async ({ page }) => {
  await page.setViewportSize({ width: 1500, height: 950 });
  await page.goto("/demo.html?map=timonier&budget=1&theme=dark-modern");
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  const mark = page.locator(".tim-budget-mark");
  await expect(mark).toHaveCount(1);
  await expect(mark).toContainText("Budget exceeded");
  await expect(mark).toHaveAttribute("title", "Budget N-1 exceeded on Daemon: p95 7.4 ms > 5 ms");
  // The role card that holds the daemon says it, for screen readers.
  await expect(page.locator(".react-flow__node", { has: mark })).toHaveAttribute(
    "aria-label",
    /Budget N-1 exceeded on Daemon: p95 7\.4 ms > 5 ms\./,
  );
  await expect(page.locator(".tim-legend")).toContainText("A budget of the whiteboard the last runs exceed");
});

for (const theme of themes) {
  test(`${theme}: an exceeded budget at WCAG AA`, async ({ page }) => {
    await page.setViewportSize({ width: 1500, height: 950 });
    await page.goto(`/demo.html?map=timonier&budget=1&theme=${theme}`);
    await page.waitForSelector("body[data-ready='true']", { state: "attached" });
    await page.waitForSelector(".tim-budget-mark");
    await settled(page);
    expect((await axeFindings(page)).violations).toEqual([]);
    const text = await textSamples(page, ".tim-budget-mark span");
    expect(text.length).toBeGreaterThan(0);
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}
