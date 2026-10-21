/**
 * The README's screenshots (docs/images/), in English and VSCode's Dark Modern theme: the
 * development page for the map, the tracking and the Performances panel, the production build
 * for the whiteboard. On demand: `npm run screenshots -w webview`.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { test, type Page } from "@playwright/test";

import { open as openWhiteboard } from "../production/page";

const OUT = path.join(import.meta.dirname, "..", "..", "..", "docs", "images");
const THEME = "dark-modern";

async function shoot(page: Page, name: string) {
  // The map lays out, the curves draw: what moves settles first.
  await page.waitForTimeout(1_200);
  await page.screenshot({ path: path.join(OUT, `${name}.png`) });
}

async function demo(page: Page, query: string, ready: string, size = { width: 1400, height: 820 }) {
  await page.setViewportSize(size);
  await page.goto(`/demo.html?${query}&theme=${THEME}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(ready);
}

test.describe.configure({ mode: "parallel" });

for (const [name, query] of [
  ["map", "map=timonier"],
  ["review", "map=timonier&review=1"],
  ["debt", "map=timonier&review=1&debt=1"],
  ["intent", "map=timonier&intent=1"],
  ["performance-layer", "map=timonier&performance=map"],
  ["budget", "map=timonier&budget=1"],
] as const) {
  test(`the map: ${name}`, async ({ page }) => {
    await demo(page, query, ".react-flow__node");
    await shoot(page, name);
  });
}

test("the tracking: plan, todo and sessions", async ({ page }) => {
  await demo(page, "tracking=all&width=1000", ".tim-tracking", { width: 1020, height: 520 });
  await shoot(page, "tracking");
});

test("the bridge", async ({ page }) => {
  await demo(page, "tracking=bridge&width=380", ".tim-bridge", { width: 400, height: 440 });
  await shoot(page, "bridge");
});

for (const tab of ["timeline", "tree", "evolution"]) {
  test(`the Performances panel: ${tab}`, async ({ page }) => {
    await demo(page, `performance=1&tab=${tab}`, ".tim-perf", { width: 1300, height: 480 });
    if (tab === "tree") {
      for (const level of await page.locator(".tim-calltree__toggle").all()) await level.click();
    }
    await shoot(page, `performance-${tab}`);
  });
}

/** A whiteboard for Timonier: layers, components, a forbidden arrow, a note, a budget, ink. */
const BOARD = {
  version: 1,
  layers: [
    { id: "L-1", name: "Core" },
    { id: "L-2", name: "Front ends" },
  ],
  components: [
    { id: "C-1", name: "Daemon", layer: "L-1", role: "Keeps the graph, serves the views" },
    { id: "C-2", name: "Store", layer: "L-1", role: "SQLite history of the graph" },
    { id: "C-3", name: "Extension", layer: "L-2", role: "VSCode panels" },
    { id: "C-4", name: "Web", layer: "L-2", role: "A browser front end" },
  ],
  edges: [
    { id: "E-1", src: "C-3", dst: "C-1", kind: "depends_on", policy: "required" },
    { id: "E-2", src: "C-3", dst: "C-2", kind: "depends_on", policy: "forbidden" },
    { id: "E-3", src: "C-1", dst: "C-2", kind: "depends_on", policy: "allowed" },
  ],
  notes: [{ id: "N-1", text: "Only the daemon writes the database", attached_to: "C-2" }],
  budgets: [{ id: "N-2", attached_to: "C-1", p95_ms: 5, memory_mb: 50 }],
  sketch: {
    shapes: {
      "L-1": { x: 40, y: 40, width: 620, height: 260 },
      "L-2": { x: 40, y: 360, width: 620, height: 220 },
      "C-1": { x: 90, y: 110, width: 200, height: 90 },
      "C-2": { x: 400, y: 110, width: 200, height: 90 },
      "C-3": { x: 90, y: 440, width: 200, height: 90 },
      "C-4": { x: 400, y: 440, width: 200, height: 90 },
      "N-1": { x: 720, y: 120 },
      "N-2": { x: 720, y: 240 },
    },
    strokes: [
      {
        id: "S-1",
        x: 380,
        y: 420,
        points: [
          [0, 0, 0.5],
          [60, -12, 0.5],
          [140, -10, 0.5],
          [230, 8, 0.5],
          [250, 70, 0.5],
          [230, 140, 0.5],
          [120, 150, 0.5],
          [10, 140, 0.5],
          [-8, 70, 0.5],
          [0, 0, 0.5],
        ],
        pen: false,
      },
    ],
    next: { C: 5, E: 4, L: 3, N: 3, S: 2 },
  },
};

test("the whiteboard", async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 820 });
  await openWhiteboard(page, { preset: BOARD });
  // The theme's variables, as VSCode gives them to a webview.
  const theme = JSON.parse(readFileSync(path.join(import.meta.dirname, "..", "themes", `${THEME}.json`), "utf8")) as {
    bodyClass: string;
    variables: Record<string, string>;
  };
  await page.evaluate((t) => {
    for (const [name, value] of Object.entries(t.variables)) document.documentElement.style.setProperty(name, value);
    document.body.className = t.bodyClass;
  }, theme);
  await page.getByRole("button", { name: /fit/i }).first().click();
  await shoot(page, "whiteboard");
});
