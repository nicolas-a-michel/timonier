/**
 * PLAN.md 4.10: the map on a synthetic repository of several thousand nodes, everything open.
 * `npm run bench:map -w webview` (Chromium, headless): figures for PLAN.md, not a pass or fail
 * on CI, where machines differ.
 */
import { test, type Page } from "@playwright/test";

/** Frames drawn per second while `act` runs for `ms` milliseconds. */
async function fps(page: Page, ms: number, act: () => Promise<void>): Promise<number> {
  await page.evaluate(() => {
    const w = window as unknown as { frames: number; raf: number };
    w.frames = 0;
    const tick = () => {
      w.frames++;
      w.raf = requestAnimationFrame(tick);
    };
    w.raf = requestAnimationFrame(tick);
  });
  const start = Date.now();
  while (Date.now() - start < ms) await act();
  const elapsed = Date.now() - start;
  const frames = await page.evaluate(() => {
    const w = window as unknown as { frames: number; raf: number };
    cancelAnimationFrame(w.raf);
    return w.frames;
  });
  return (frames * 1000) / elapsed;
}

for (const size of ["20x12x12", "40x12x12"]) {
  test(`map of ${size}`, async ({ page }) => {
    test.setTimeout(300_000);
    const start = Date.now();
    await page.goto(`/demo.html?map=synthetic&size=${size}&open=all&theme=dark-modern`);
    await page.waitForSelector(".react-flow__node", { timeout: 240_000 });
    const firstDraw = Date.now() - start;
    const items = await page.evaluate(() => document.querySelectorAll(".react-flow__node").length);
    const edges = await page.evaluate(() => document.querySelectorAll(".react-flow__edge").length);
    // Panning drags the background: a point of the pane that no box covers, since a box
    // dragged moves itself (ADR 0021).
    const [cx, cy] = await page.evaluate(() => {
      const pane = document.querySelector(".react-flow__pane")?.getBoundingClientRect();
      if (!pane) return [0, 0];
      for (let fy = 0.5; fy < 0.95; fy += 0.05) {
        for (let fx = 0.5; fx < 0.95; fx += 0.05) {
          const x = pane.x + pane.width * fx;
          const y = pane.y + pane.height * fy;
          if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) return [x, y];
        }
      }
      return [pane.x + 4, pane.y + pane.height - 4];
    });
    let step = 0;
    const pan = await fps(page, 3000, async () => {
      step++;
      await page.mouse.move(cx, cy);
      await page.mouse.down();
      await page.mouse.move(cx + (step % 2 ? 80 : -80), cy + (step % 2 ? 40 : -40), { steps: 4 });
      await page.mouse.up();
    });
    const zoom = await fps(page, 3000, async () => {
      step++;
      await page.mouse.move(cx, cy);
      await page.mouse.wheel(0, step % 2 ? 120 : -120);
    });
    const nodes = Number(size.split("x").reduce((a, b) => String(Number(a) * Number(b)), "1"));
    console.log(
      `${size}: ${nodes} files; ${items} nodes and ${edges} edges in the DOM; first draw ${firstDraw} ms; ` +
        `pan ${pan.toFixed(1)} fps; zoom ${zoom.toFixed(1)} fps`,
    );
  });
}
