/**
 * The whiteboard as the extension ships it (PLAN.md 7.1, ADR 0036): the production build, in
 * the page the extension writes, under its CSP. Freehand drawing, the eraser, undo and the
 * keyboard work there, not only on the development server; and it is at WCAG AA in every
 * captured theme.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, themes, weak } from "../a11y/measure";
import { open } from "./page";

interface Theme {
  bodyClass: string;
  variables: Record<string, string>;
}

const strokes = (page: Page) => page.locator(".react-flow__node-stroke");

/** Draws a stroke with the mouse, from (x, y), on the board. */
async function scribble(page: Page, x: number, y: number) {
  await page.mouse.move(x, y);
  await page.mouse.down();
  for (let i = 1; i <= 12; i++) await page.mouse.move(x + i * 15, y + Math.sin(i / 2) * 30);
  await page.mouse.up();
}

test.describe.configure({ mode: "parallel" });

test("runs under the webview's CSP, and says it is ready", async ({ page }) => {
  const { errors } = await open(page);
  await expect(page.getByRole("toolbar", { name: "Whiteboard tools" })).toBeVisible();
  await expect(page.getByText("An empty whiteboard")).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { violations: string[] }).violations)).toEqual([]);
  expect(errors).toEqual([]);
});

test("draws freehand with the pointer, then undoes and redoes from the keyboard", async ({ page }) => {
  await open(page);
  await scribble(page, 300, 400);
  await expect(strokes(page)).toHaveCount(1);
  await expect(strokes(page).locator("path")).toHaveAttribute("d", /^M[\d.]+,[\d.]+ Q.*Z$/);
  await expect(page.locator(".tim-whiteboard__count")).toHaveText("1 stroke, 0 shapes");
  await scribble(page, 300, 600);
  await expect(strokes(page)).toHaveCount(2);

  await page.keyboard.press("Control+z");
  await expect(strokes(page)).toHaveCount(1);
  await page.keyboard.press("Control+z");
  await expect(strokes(page)).toHaveCount(0);
  await page.keyboard.press("Control+Shift+z");
  await expect(strokes(page)).toHaveCount(1);
  await page.keyboard.press("Control+y");
  await expect(strokes(page)).toHaveCount(2);

  // The page keeps the board: a reload of the panel finds it as it was left.
  await page.reload();
  await page.waitForFunction(() =>
    ((window as unknown as { sent: { type: string }[] }).sent ?? []).some((m) => m.type === "ready"),
  );
  await expect(strokes(page)).toHaveCount(2);
});

test("the eraser removes the strokes it goes over, in one step", async ({ page }) => {
  await open(page);
  await scribble(page, 300, 300);
  await scribble(page, 300, 500);
  await scribble(page, 300, 700);
  await expect(strokes(page)).toHaveCount(3);
  const box = await strokes(page).nth(0).boundingBox();
  const second = await strokes(page).nth(1).boundingBox();
  if (!box || !second) throw new Error("no strokes");

  await page.keyboard.press("e");
  await expect(page.getByRole("button", { name: "Erase (E)" })).toHaveAttribute("aria-pressed", "true");
  // Down through the first two strokes, from above the first to below the second.
  const x = 300 + 90;
  await page.mouse.move(x, box.y - 20);
  await page.mouse.down();
  for (let y = box.y - 20; y <= second.y + second.height + 20; y += 6) await page.mouse.move(x, y);
  await page.mouse.up();
  await expect(strokes(page)).toHaveCount(1);
  await page.keyboard.press("Control+z");
  await expect(strokes(page)).toHaveCount(3);
});

test("from the keyboard alone: pick a tool, reach a stroke, move it, delete it, and undo", async ({ page }) => {
  await open(page);
  await scribble(page, 300, 400);
  await expect(strokes(page)).toHaveCount(1);
  const stroke = strokes(page).first();

  await page.keyboard.press("v");
  await expect(page.getByRole("button", { name: "Select (V)" })).toHaveAttribute("aria-pressed", "true");
  await stroke.focus();
  await expect(stroke).toHaveAccessibleName("Stroke 1");
  await page.keyboard.press("Enter");
  await expect(stroke).toHaveClass(/selected/);
  const before = await stroke.boundingBox();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await expect.poll(async () => (await stroke.boundingBox())?.x).toBeGreaterThan((before?.x ?? 0) + 5);
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  await expect.poll(async () => (await stroke.boundingBox())?.x).toBeCloseTo(before?.x ?? 0, 0);

  await stroke.focus();
  await page.keyboard.press("Delete");
  await expect(strokes(page)).toHaveCount(0);
  await page.keyboard.press("Control+z");
  await expect(strokes(page)).toHaveCount(1);
});

test("the toolbar is one tab stop, then the arrow keys", async ({ page }) => {
  await open(page);
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Select (V)" })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("button", { name: "Draw (P)" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: "Draw (P)" })).toHaveAttribute("aria-pressed", "true");
});

test("sends the board as intent.json after each gesture, and draws again the one it is given (Q66)", async ({
  page,
}) => {
  await open(page);
  await put(page, "b", 400, 400, "Parser");
  const last = () =>
    page.evaluate(
      () =>
        (window as unknown as { sent: { type: string; intent?: { components: { name: string }[] } }[] }).sent
          .filter((m) => m.type === "intent")
          .at(-1)?.intent,
    );
  await expect.poll(async () => (await last())?.components.map((c) => c.name)).toEqual(["Parser"]);
  // The file changed outside the page: the whiteboard draws it, from a new history.
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "intent",
        intent: {
          version: 1,
          layers: [{ id: "L-1", name: "From git" }],
          components: [],
          edges: [],
          notes: [],
          budgets: [],
          sketch: { shapes: { "L-1": { x: 0, y: 0, width: 400, height: 300 } }, strokes: [], next: { L: 2 } },
        },
        problem: null,
      },
      "*",
    ),
  );
  await expect(page.locator(".react-flow__node-zone")).toContainText("From git");
  await expect(page.locator(".react-flow__node-box")).toHaveCount(0);
  await page.keyboard.press("Control+z");
  await expect(page.locator(".react-flow__node-zone")).toHaveCount(1);
  // Saved or not, the page says it.
  await page.evaluate(() =>
    window.postMessage({ type: "intentSaved", saved: false, detail: "the daemon is not running" }, "*"),
  );
  await expect(page.getByText("Not saved: the daemon is not running")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.evaluate(() => window.postMessage({ type: "intentSaved", saved: true, detail: null }, "*"));
  await expect(page.getByText("Saved in .timonier/intent.json")).toBeVisible();
});

test("a file that does not read is not overwritten until the developer starts over", async ({ page }) => {
  await open(page);
  await page.evaluate(() =>
    window.postMessage({ type: "intent", intent: null, problem: "intent.json: expected value at line 1" }, "*"),
  );
  await expect(page.getByRole("heading", { name: "The whiteboard does not read" })).toBeVisible();
  await expect(page.getByText("intent.json: expected value at line 1")).toBeVisible();
  const sent = () =>
    page.evaluate(
      () => (window as unknown as { sent: { type: string }[] }).sent.filter((m) => m.type === "intent").length,
    );
  const before = await sent();
  await page.getByRole("button", { name: "Start over with an empty whiteboard" }).click();
  await expect(page.getByRole("toolbar", { name: "Whiteboard tools" })).toBeVisible();
  expect(await sent()).toBe(before);
  await put(page, "b", 400, 400, "New");
  await expect.poll(sent).toBeGreaterThan(before);
});

test("speaks French when the IDE does", async ({ page }) => {
  await open(page, { locale: "fr" });
  await expect(page.getByRole("toolbar", { name: "Outils du tableau blanc" })).toBeVisible();
  await expect(page.getByText("Un tableau blanc vide")).toBeVisible();
});

const node = (page: Page, kind: string) => page.locator(`.react-flow__node-${kind}`);
const field = (page: Page, name: string) => page.getByRole("textbox", { name, exact: true });

/** Puts a shape with its tool, where the pointer clicks, and names it. */
async function put(page: Page, key: string, x: number, y: number, name?: string) {
  await page.keyboard.press(key);
  await page.mouse.click(x, y);
  if (name !== undefined) {
    await expect(field(page, "Name")).toBeFocused();
    await page.keyboard.type(name);
    await page.keyboard.press("Enter");
  }
}

test("typed shapes with the pointer: a zone, boxes inside it, an arrow with its policy (PLAN.md 7.2)", async ({
  page,
}) => {
  await open(page);
  await put(page, "z", 400, 400, "Domain");
  await expect(node(page, "zone")).toContainText("Domain");
  await put(page, "b", 300, 400, "Parser");
  await put(page, "b", 800, 400, "Store");
  await expect(node(page, "box")).toHaveCount(2);
  // A box drawn inside a zone is in its layer (Q62); the other is not.
  await expect(node(page, "box").first()).toHaveAccessibleName(/^Parser, intended component, in Domain\./);
  await expect(node(page, "box").nth(1)).toHaveAccessibleName(/^Store, intended component\./);
  await expect(node(page, "zone")).toHaveAccessibleName(/^Domain, layer, 1 component\./);

  await page.keyboard.press("a");
  await expect(page.locator(".tim-whiteboard__count")).toHaveText("Click the box or zone the arrow leaves from.");
  await page.mouse.click(300, 400);
  await expect(page.locator(".tim-whiteboard__count")).toHaveText(/^From Parser: click/);
  await page.mouse.click(800, 400);
  const arrow = page.locator(".react-flow__edge-arrow");
  await expect(arrow).toHaveCount(1);
  await expect(arrow).toHaveAccessibleName("Allowed dependency from Parser to Store.");
  await page.getByRole("radio", { name: "Forbidden" }).check();
  await expect(arrow).toHaveAccessibleName("Forbidden dependency from Parser to Store.");
  await expect(page.locator(".tim-wb-arrow__mark")).toHaveText("⊘ forbidden");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+z");
  await expect(arrow).toHaveAccessibleName("Allowed dependency from Parser to Store.");

  // From the arrow to a box, then a note put down on the board: it attaches to the box (Q65).
  await arrow.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("radio", { name: "Allowed" })).toBeChecked();
  await node(page, "box").first().click();
  await page.keyboard.press("n");
  await page.mouse.click(300, 760);
  await page.keyboard.type("Pure functions only");
  await page.keyboard.press("Control+Enter");
  await expect(node(page, "note")).toHaveAccessibleName("Note on Parser: Pure functions only");
  // Two steps: the note, then its words.
  await page.keyboard.press("Control+z");
  await expect(node(page, "note")).toHaveAccessibleName("Note on Parser: Empty note");
  await page.keyboard.press("Control+z");
  await expect(node(page, "note")).toHaveCount(0);

  // The zone carries its box when it is moved: the same distance.
  const top = async (locator: ReturnType<typeof node>) => (await locator.boundingBox())?.y ?? Number.NaN;
  const [boxBefore, zoneBefore] = [await top(node(page, "box").first()), await top(node(page, "zone"))];
  await page.keyboard.press("v");
  await page.mouse.move(250, 260);
  await page.mouse.down();
  await page.mouse.move(250, 320, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await top(node(page, "zone"))) - zoneBefore).toBeGreaterThan(30);
  const zoneMoved = (await top(node(page, "zone"))) - zoneBefore;
  await expect.poll(async () => (await top(node(page, "box").first())) - boxBefore).toBeCloseTo(zoneMoved, 0);
  await expect(node(page, "box").nth(1)).toHaveAccessibleName(/^Store, intended component\./);

  // A box removed takes its arrows with it, in one step.
  await node(page, "box").first().click();
  await page.keyboard.press("Delete");
  await expect(node(page, "box")).toHaveCount(1);
  await expect(arrow).toHaveCount(0);
  await page.keyboard.press("Control+z");
  await expect(node(page, "box")).toHaveCount(2);
  await expect(arrow).toHaveCount(1);
});

test("typed shapes from the keyboard alone: a box, a note on it, a budget, an arrow", async ({ page }) => {
  await open(page);
  const tools = page.getByRole("toolbar", { name: "Whiteboard tools" });
  await page.keyboard.press("Tab");
  await expect(tools.getByRole("button", { name: "Select (V)" })).toBeFocused();
  for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowRight");
  await expect(tools.getByRole("button", { name: "Box (B)" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(field(page, "Name")).toBeFocused();
  await page.keyboard.type("Parser");
  await page.keyboard.press("Tab");
  await page.keyboard.type("Turns tokens into a tree");
  await page.keyboard.press("Tab");
  await expect(node(page, "box")).toHaveAccessibleName("Parser, intended component. Turns tokens into a tree");

  // With the box selected, a note and a budget attach to it (Q65).
  await tools.getByRole("button", { name: "Box (B)" }).focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await expect(tools.getByRole("button", { name: "Note (N)" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(field(page, "Text")).toBeFocused();
  await page.keyboard.type("No allocation in the hot loop");
  await page.keyboard.press("Control+Enter");
  await expect(node(page, "note")).toHaveAccessibleName("Note on Parser: No allocation in the hot loop");
  await expect(page.getByRole("combobox", { name: "Attached to" })).toHaveValue("C-1");
  await expect(page.locator(".react-flow__edge-attach")).toHaveCount(1);

  await node(page, "box").focus();
  await page.keyboard.press("Enter");
  await tools.getByRole("button", { name: "Budget (M)" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("spinbutton", { name: "Latency p95 (ms)" })).toBeFocused();
  await page.keyboard.type("5");
  await page.keyboard.press("Enter");
  await page.getByRole("spinbutton", { name: "Peak memory (MB)" }).fill("50");
  await page.keyboard.press("Enter");
  await expect(node(page, "budget")).toHaveAccessibleName("Budget of Parser: p95 < 5 ms, memory < 50 MB");

  // A second box, then an arrow to it from the first one's properties.
  await tools.getByRole("button", { name: "Box (B)" }).focus();
  await page.keyboard.press("Enter");
  await page.keyboard.type("Store");
  await page.keyboard.press("Enter");
  await node(page, "box").first().focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("combobox", { name: "New arrow to" })).toHaveValue("C-2");
  await page.getByRole("button", { name: "Add the arrow" }).click();
  await expect(page.locator(".react-flow__edge-arrow")).toHaveAccessibleName(
    "Allowed dependency from Parser to Store.",
  );
  await page.getByRole("radio", { name: "Required" }).focus();
  await page.keyboard.press("Space");
  await expect(page.locator(".tim-wb-arrow__mark")).toHaveText("! required");
  await expect(page.locator(".tim-whiteboard__count")).toHaveText("0 strokes, 5 shapes");

  // Everything the board holds comes back after a reload of the panel.
  await page.reload();
  await page.waitForFunction(() =>
    ((window as unknown as { sent: { type: string }[] }).sent ?? []).some((m) => m.type === "ready"),
  );
  await expect(page.locator(".tim-whiteboard__count")).toHaveText("0 strokes, 5 shapes");
  await expect(node(page, "budget")).toHaveAccessibleName("Budget of Parser: p95 < 5 ms, memory < 50 MB");
});

for (const theme of themes) {
  test(`${theme}: the whiteboard at WCAG AA`, async ({ page }) => {
    await open(page);
    const captured = JSON.parse(
      readFileSync(path.join(import.meta.dirname, "..", "themes", `${theme}.json`), "utf8"),
    ) as Theme;
    // As VSCode does: variables on <html>, the kind of theme as a class of <body>.
    await page.evaluate((t) => {
      for (const [name, value] of Object.entries(t.variables)) document.documentElement.style.setProperty(name, value);
      document.body.className = t.bodyClass;
    }, captured);
    await scribble(page, 300, 650);
    await put(page, "z", 450, 400, "Domain");
    await put(page, "b", 400, 400, "Parser");
    await put(page, "b", 900, 400, "Store");
    await page.keyboard.press("a");
    await page.mouse.click(400, 400);
    await page.mouse.click(900, 400);
    await page.getByRole("radio", { name: "Forbidden" }).check();
    await node(page, "box").first().click();
    await page.keyboard.press("n");
    await page.mouse.click(400, 600);
    await page.keyboard.type("A note");
    await page.keyboard.press("Control+Enter");
    await page.keyboard.press("v");
    await node(page, "box").first().focus();
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    const found = await samples(page);
    expect(found.some((s) => s.what.startsWith("fill"))).toBe(true);
    expect(weak(found)).toEqual([]);
  });
}
