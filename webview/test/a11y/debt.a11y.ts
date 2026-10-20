/**
 * The layer of the review debt (PLAN.md 6.5, ADR 0030, Q44): a toggle of the toolbar, the
 * boxes in three levels with their words, the untracked ones hatched and named, the legend; WCAG
 * AA in every captured theme.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, { theme = "dark-modern", locale = "en", debt = false } = {}) {
  await page.goto(`/demo.html?map=timonier&review=1&theme=${theme}&locale=${locale}${debt ? "&debt=1" : ""}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

const box = (page: Page, id: string) => page.locator(`.react-flow__node[data-id="${id}"]`);

test.describe.configure({ mode: "parallel" });

test("the toolbar turns the layer on and off, and nothing shows without it", async ({ page }) => {
  await open(page);
  await expect(page.locator(".tim-debt")).toHaveCount(0);
  const toggle = page.getByRole("button", { name: "Review debt" });
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  // A role adds up its members' files; its name says it too.
  const utility = box(page, "@role/utility");
  await expect(utility.locator(".tim-debt")).toHaveText("12 to read again");
  await expect(utility).toHaveAttribute("aria-label", /12 files to read again\./);
  await expect(utility.locator(".tim-map-role")).toHaveAttribute("data-debt", "2");
  // Untracked: a hatched mark and the word.
  const entry = box(page, "@role/entry_point");
  await expect(entry.locator(".tim-debt__untracked")).toHaveText("untracked");
  await expect(entry).toHaveAttribute("aria-label", /2 files to read again, untracked\./);
  // The legend says what the levels and the hatching are.
  await expect(page.locator(".tim-legend")).toContainText("Review debt");
  await expect(page.locator(".tim-legend__swatch")).toHaveCount(4);
  await toggle.click();
  await expect(page.locator(".tim-debt")).toHaveCount(0);
});

test("a box says its files in words, at its level", async ({ page }) => {
  await open(page, { locale: "fr", debt: true });
  await box(page, "@role/utility").click();
  const index = box(page, "pkg:cargo/timonier-index");
  await expect(index.locator(".tim-box__meta .tim-debt")).toHaveText("9 fichiers à relire");
  await expect(index.locator(".tim-box")).toHaveAttribute("data-debt", "2");
  const mcp = box(page, "pkg:cargo/timonier-mcp");
  await expect(mcp.locator(".tim-box")).toHaveAttribute("data-debt", "3");
  // In the layer, the debt alone: no review flag beside it.
  await expect(index.locator(".tim-box__flag")).toHaveCount(0);
});

for (const theme of themes) {
  test(`${theme}: the debt layer at WCAG AA`, async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 1000 });
    await open(page, { theme, debt: true });
    for (const role of ["entry_point", "utility"]) await box(page, `@role/${role}`).click();
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    // The decorative marks (hatching, swatches) have no text: they are measured as marks, at 3:1.
    const text = await textSamples(
      page,
      ".react-flow__node :is(span, li, .tim-box__name):not([aria-hidden]), .tim-legend span:not([aria-hidden])",
    );
    expect(text.length).toBeGreaterThan(10);
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}
