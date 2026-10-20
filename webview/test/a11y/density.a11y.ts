/**
 * The compact map (Q32, setting `timonier.map.density`): smaller boxes and tighter gaps, at
 * WCAG AA in every captured theme, with no text cut by its box.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, theme: string, density: "compact" | "comfortable") {
  await page.goto(`/demo.html?map=timonier&theme=${theme}&locale=fr&review=1&density=${density}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
  for (const role of ["entry_point", "persistence"]) {
    await page.locator(`.react-flow__node[data-id="@role/${role}"]`).click();
    await expect(page.locator(`.react-flow__node[data-id="@role/${role}"] .tim-map-frame`)).toBeVisible();
  }
  await settled(page);
}

test.describe.configure({ mode: "parallel" });

test("the compact boxes are smaller, and hold their text whole", async ({ page }) => {
  await open(page, "dark-modern", "compact");
  const zoom = Number(
    /matrix\(([^,]+)/.exec(
      await page.locator(".react-flow__viewport").evaluate((e) => getComputedStyle(e).transform),
    )?.[1] ?? "1",
  );
  const box = await page.locator('.react-flow__node[data-id="pkg:cargo/timonier-cli"]').boundingBox();
  expect(Math.round((box?.width ?? 0) / zoom)).toBe(184);
  // No line of a box or of a role card goes past its bottom.
  const cut = await page.evaluate(() => {
    const found: string[] = [];
    for (const container of document.querySelectorAll(".tim-box, .tim-map-role")) {
      const bottom = container.getBoundingClientRect().bottom;
      for (const line of container.querySelectorAll(".tim-box__meta, .tim-box__role, li")) {
        if (line.getBoundingClientRect().bottom > bottom + 0.5) found.push(line.textContent ?? "");
      }
    }
    return found;
  });
  expect(cut).toEqual([]);
});

for (const theme of themes) {
  test(`${theme}: the compact map at WCAG AA`, async ({ page }) => {
    await open(page, theme, "compact");
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    const text = await textSamples(page, ".react-flow__node :is(span, li, .tim-box__name)");
    expect(text.length).toBeGreaterThan(10);
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}
