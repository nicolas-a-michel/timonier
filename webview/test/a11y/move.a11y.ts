/**
 * Boxes moved by hand (ADR 0021, Q20): computed by default, movable with the mouse or the
 * keyboard, kept where they were left, and put back by « Reset the layout ».
 */
import { expect, test, type Locator, type Page } from "@playwright/test";

import { settled } from "./measure";

async function open(page: Page) {
  await page.goto("/demo.html?map=timonier&theme=dark-modern&locale=en");
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

/** Where a node is, in its container, as React Flow places it. */
async function at(page: Page, id: string): Promise<[number, number]> {
  const style = (await page.locator(`.react-flow__node[data-id="${id}"]`).getAttribute("style")) ?? "";
  const found = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(style);
  if (!found) throw new Error(`no position for ${id}: ${style}`);
  return [Number(found[1]), Number(found[2])];
}

/** Where the item is on screen, once the layout no longer moves it. */
async function steady(page: Page, item: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  await settled(page);
  let last = "";
  await expect
    .poll(async () => {
      const box = JSON.stringify(await item.boundingBox());
      const same = box === last;
      last = box;
      return same;
    })
    .toBe(true);
  const box = await item.boundingBox();
  if (!box) throw new Error("not on screen");
  return box;
}

/** The map node that takes the pointer at this point of the screen; `null` for the map itself. */
async function nodeAt(page: Page, point: { x: number; y: number }): Promise<string | null> {
  return page.evaluate(
    ({ x, y }) => document.elementFromPoint(x, y)?.closest(".react-flow__node")?.getAttribute("data-id") ?? null,
    point,
  );
}

async function pins(page: Page): Promise<Record<string, { parent: string; x: number; y: number }>> {
  const value = await page.evaluate(() => document.body.dataset.pins);
  return value ? (JSON.parse(value) as Record<string, { parent: string; x: number; y: number }>) : {};
}

test.describe.configure({ mode: "parallel" });

test("a box dragged stays where it was left, and the others make room", async ({ page }) => {
  await open(page);
  const id = "@role/persistence";
  const [x, y] = await at(page, id);
  const box = await page.locator(`.react-flow__node[data-id="${id}"]`).boundingBox();
  if (!box) throw new Error("no box");
  await page.mouse.move(box.x + 40, box.y + 20);
  await page.mouse.down();
  await page.mouse.move(box.x + 140, box.y + 220, { steps: 8 });
  await page.mouse.up();
  // Not a click: the role did not open.
  await expect(page.locator(".react-flow__node .tim-map-role__title")).toHaveCount(6);
  await expect.poll(async () => (await pins(page))[id]?.parent).toBe("@root");
  const [nx, ny] = await at(page, id);
  expect(nx).toBeGreaterThan(x + 50);
  expect(ny).toBeGreaterThan(y + 50);
  // Another role opens: the moved box keeps its place.
  await page.locator(".react-flow__node", { hasText: "Interface" }).click();
  await expect(page.locator(".react-flow__node .tim-box__name").first()).toBeVisible();
  expect(await at(page, id)).toEqual([nx, ny]);
});

test("Alt and an arrow move the focused box one cell, and the reset puts it back", async ({ page }) => {
  await open(page);
  const id = "@role/interface";
  const start = await at(page, id);
  // A pin holds whole pixels.
  const [x, y] = start.map(Math.round) as [number, number];
  await page.locator(`.react-flow__node[data-id="${id}"]`).focus();
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => at(page, id)).toEqual([x + 16, y]);
  await page.keyboard.press("Alt+ArrowDown");
  await expect.poll(() => at(page, id)).toEqual([x + 16, y + 16]);
  // An arrow alone still goes to another box, and moves nothing.
  await page.keyboard.press("ArrowUp");
  expect(await at(page, id)).toEqual([x + 16, y + 16]);
  await page.getByRole("button", { name: "Reset the layout" }).click();
  await expect.poll(() => pins(page)).toEqual({});
  await expect.poll(() => at(page, id)).toEqual(start);
});

test("the details panel moves the box pointed at, without dragging (WCAG 2.5.7)", async ({ page }) => {
  await open(page);
  const id = "@role/data_model";
  const [x, y] = (await at(page, id)).map(Math.round) as [number, number];
  await page.locator(`.react-flow__node[data-id="${id}"]`).hover();
  await expect(page.locator(".tim-map-details")).toContainText("Data model");
  await page.getByRole("button", { name: "Move right" }).click();
  await expect.poll(() => at(page, id)).toEqual([x + 16, y]);
  await page.getByRole("button", { name: "Move down" }).click();
  await expect.poll(() => at(page, id)).toEqual([x + 16, y + 16]);
});

test("dragging the body of an open frame pans the map, and leaves the frame in place", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Entry point" }).click();
  const frame = page.locator('.react-flow__node[data-id="@role/entry_point"]');
  await expect(frame.locator(".tim-map-frame")).toBeVisible();
  // Once the frame has its size and its place (review n° 16, m16-7).
  const box = await steady(page, frame);
  const viewport = () => page.locator(".react-flow__viewport").getAttribute("style");
  const before = await viewport();
  const placedBefore = await at(page, "@role/entry_point");
  // Its body, below its boxes: what the frame holds is elsewhere, and the body lets the pointer
  // through to the map.
  const point = { x: box.x + box.width - 6, y: box.y + box.height - 6 };
  expect(await nodeAt(page, point)).toBeNull();
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  await page.mouse.move(point.x + 120, point.y + 60, { steps: 6 });
  await page.mouse.up();
  await expect.poll(viewport).not.toBe(before);
  expect(await at(page, "@role/entry_point")).toEqual(placedBefore);
  expect(await pins(page)).toEqual({});
});
