/**
 * The « Certainty » layer (PLAN.md 11.5, ADR 0074): a toggle of the toolbar, each box's share of
 * links confirmed in words, the map's own share in the legend; WCAG AA in every captured theme
 * (review of Phase 11, I8).
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, { theme = "dark-modern", locale = "en", certainty = false } = {}) {
  await page.goto(`/demo.html?map=timonier&theme=${theme}&locale=${locale}${certainty ? "&certainty=1" : ""}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

test.describe.configure({ mode: "parallel" });

test("the toolbar turns the layer on and off, and the boxes and the map say how sure", async ({ page }) => {
  await open(page);
  await expect(page.locator(".tim-sure-words")).toHaveCount(0);
  const toggle = page.getByRole("button", { name: "Certainty" });
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  const words = page.locator(".tim-sure-words");
  expect(await words.count()).toBeGreaterThan(3);
  await expect(words.first()).toHaveText(/^\d+% sure$/);
  // Its counts on hover, the map's share in the legend.
  await expect(words.first()).toHaveAttribute("title", /of \d+ links? confirmed by a manifest/);
  await expect(page.locator(".tim-legend")).toContainText(/of the links drawn are confirmed/);
  await toggle.click();
  await expect(page.locator(".tim-sure-words")).toHaveCount(0);
});

test("speaks French", async ({ page }) => {
  await open(page, { locale: "fr", certainty: true });
  await expect(page.locator(".tim-sure-words").first()).toHaveText(/^\d+\s%\ssûrs?$/);
});

for (const theme of themes) {
  test(`${theme}: the certainty layer at WCAG AA`, async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 900 });
    await open(page, { theme, certainty: true });
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    const text = await textSamples(
      page,
      ".react-flow__node :is(span, li, .tim-box__name):not([aria-hidden]), .tim-legend span:not([aria-hidden])",
    );
    expect(text.length).toBeGreaterThan(10);
    expect(weak(text, 4.5)).toEqual([]);
    // The three line looks, solid and dashed, at 3:1.
    expect(weak(await samples(page))).toEqual([]);
  });
}
