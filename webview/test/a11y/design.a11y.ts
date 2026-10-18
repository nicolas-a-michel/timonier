/**
 * PLAN.md 4.6: WCAG AA in every built-in VSCode theme, measured on the design system's
 * demonstration page with the variables VSCode really gives a webview (test/themes).
 * - axe-core checks the text (4.5:1) and the rest of WCAG A and AA it can see;
 * - lines, icons and the focus ring (WCAG 1.4.11, 3:1) are measured here: axe does not.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, themes, weak } from "./measure";

async function open(page: Page, theme: string, locale = "en") {
  await page.goto(`/demo.html?theme=${theme}&locale=${locale}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
}

test.describe.configure({ mode: "parallel" });

test("every theme VSCode ships is captured", () => {
  expect(themes).toEqual([
    "dark-2026",
    "dark-modern",
    "high-contrast",
    "high-contrast-light",
    "light-2026",
    "light-modern",
  ]);
});

for (const theme of themes) {
  test.describe(theme, () => {
    for (const locale of ["en", "fr"]) {
      test(`axe-core finds nothing against WCAG A and AA (${locale})`, async ({ page }) => {
        await open(page, theme, locale);
        const findings = await axeFindings(page);
        expect(findings.violations).toEqual([]);
        // A contrast axe could not decide is not a pass.
        expect(findings.undecided).toEqual([]);
      });
    }

    test("lines, icons and the focus ring reach 3:1", async ({ page }) => {
      await open(page, theme);
      // The focus ring, on the first box reached by the keyboard.
      await page.locator(".tim-box").first().focus();
      await page.keyboard.press("Shift+Tab");
      await page.keyboard.press("Tab");
      const found = await samples(page);
      expect(found.some((s) => s.what === "focus ring")).toBe(true);
      expect(found.filter((s) => s.what.startsWith("stroke")).length).toBeGreaterThanOrEqual(3);
      expect(weak(found)).toEqual([]);
    });
  });
}

test("reduced motion turns the transitions off", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await open(page, themes[0] ?? "light-modern");
  const durations = await page.evaluate(() =>
    [".tim-box", ".tim-edge"].map((selector) => {
      const element = document.querySelector(selector);
      return element ? getComputedStyle(element).transitionDuration : "missing";
    }),
  );
  for (const duration of durations) {
    expect(duration.split(",").every((d) => parseFloat(d) === 0)).toBe(true);
  }
});
