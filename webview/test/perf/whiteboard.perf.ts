/**
 * PLAN.md 7.1, ADR 0036 and 0037: the whiteboard with thousands of freehand strokes, in its
 * production build. `npm run bench:whiteboard -w webview` (Chromium, headless): figures for
 * PLAN.md, not a pass or fail on CI, where machines differ.
 */
import { expect, test } from "@playwright/test";

import { open } from "../production/page";

/** A board of `count` strokes of 40 points, in rows, as `intent.json` holds it. */
function board(count: number) {
  const strokes = Array.from({ length: count }, (_, i) => ({
    id: `S-${i + 1}`,
    x: (i % 60) * 130,
    y: Math.floor(i / 60) * 40,
    points: Array.from({ length: 40 }, (_, j) => [j * 3, Math.sin(j / 4) * 10 + 10, 0.5]),
  }));
  return {
    version: 1,
    layers: [],
    components: [],
    edges: [],
    notes: [],
    budgets: [],
    sketch: { shapes: {}, strokes, next: { S: count + 1 } },
  };
}

for (const count of [1000, 3000]) {
  test(`${count} strokes: loading, a new stroke, an undo`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1300, height: 900 });
    const loading = Date.now();
    await open(page, { preset: board(count) });
    const count1 = page.locator(".tim-whiteboard__count");
    await expect(count1).toHaveText(`${count} strokes, 0 shapes`);
    const load = Date.now() - loading;
    await page.keyboard.press("p");
    await page.mouse.move(300, 400);
    await page.mouse.down();
    for (let i = 1; i <= 12; i++) await page.mouse.move(300 + i * 15, 400 + Math.sin(i / 2) * 30);
    const drawing = Date.now();
    await page.mouse.up();
    await expect(count1).toHaveText(`${count + 1} strokes, 0 shapes`);
    const draw = Date.now() - drawing;
    const undoing = Date.now();
    await page.keyboard.press("Control+z");
    await expect(count1).toHaveText(`${count} strokes, 0 shapes`);
    const undo = Date.now() - undoing;
    console.log(JSON.stringify({ strokes: count, loadMs: load, strokeMs: draw, undoMs: undo }));
  });
}
