/**
 * PLAN.md 4.7: on Timonier, the opening view fits in a few boxes named by role; the map opens
 * by click and by keyboard, and stays WCAG AA in every VSCode theme. The view is Timonier's own,
 * written by the `view_snapshot` example (test/views/timonier).
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, theme = "dark-modern", locale = "en") {
  await page.goto(`/demo.html?map=timonier&theme=${theme}&locale=${locale}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

/** The titles of the boxes on screen. */
async function roles(page: Page): Promise<string[]> {
  return page.locator(".react-flow__node .tim-map-role__title").allInnerTexts();
}

test.describe.configure({ mode: "parallel" });

test("Timonier opens on a few boxes, one per role", async ({ page }) => {
  await open(page);
  const titles = await roles(page);
  expect(titles).toEqual(["Entry point", "Interface", "Data model", "Persistence", "Utility", "External libraries"]);
  // Each box says who it holds.
  await expect(page.locator(".react-flow__node", { hasText: "Entry point" })).toContainText("timonier-daemon");
  // Its name for screen readers: the role, its members, what Enter does.
  const label = await page.locator(".react-flow__node", { hasText: "Persistence" }).getAttribute("aria-label");
  expect(label).toBe("Persistence, 1 member: timonier-store. Enter opens it.");
});

test("speaks French", async ({ page }) => {
  await open(page, "light-2026", "fr");
  expect(await roles(page)).toEqual([
    "Point d'entrée",
    "Interface",
    "Modèle de données",
    "Persistance",
    "Utilitaire",
    "Bibliothèques externes",
  ]);
});

test("a click opens a role on its members, and the tools go back", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Entry point" }).click();
  await expect(page.locator(".react-flow__node .tim-box__name")).toHaveText(["timonier-cli", "timonier-daemon"]);
  // A box opens on what it holds, as the daemon expands it.
  await page.locator('.react-flow__node[data-id="pkg:cargo/timonier-cli"]').click();
  await expect(page.locator(".react-flow__node .tim-map-frame--box")).toHaveCount(1);
  await page.getByRole("button", { name: "Back to the roles" }).click();
  await expect(page.locator(".react-flow__node .tim-map-role__title")).toHaveCount(6);
});

test("Enter opens the focused role", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Interface" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".react-flow__node .tim-box__name")).toHaveText(["@timonier/webview", "timonier"]);
});

test("F2 or a double click on a package asks for its name and role (ADR 0075)", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Persistence" }).click();
  const store = page.locator('.react-flow__node[data-id="pkg:cargo/timonier-store"]');
  await store.focus();
  await page.keyboard.press("F2");
  await expect(page.locator("body")).toHaveAttribute(
    "data-rename",
    JSON.stringify({ target: "pkg:cargo/timonier-store", name: "timonier-store", role: "persistence" }),
  );
  await page.evaluate(() => delete document.body.dataset.rename);
  await store.dblclick();
  await expect(page.locator("body")).toHaveAttribute("data-rename", /timonier-store/);
  // A role group is no box of the graph: F2 there does nothing.
  await page.evaluate(() => delete document.body.dataset.rename);
  await page.locator(".react-flow__node", { hasText: "Utility" }).focus();
  await page.keyboard.press("F2");
  await expect(page.locator("body")).not.toHaveAttribute("data-rename", /./);
});

for (const theme of themes) {
  test(`${theme}: WCAG AA, lines at 3:1`, async ({ page }) => {
    await open(page, theme);
    await page.locator(".react-flow__node", { hasText: "Entry point" }).click();
    await page.locator(".react-flow__node", { hasText: "Persistence" }).focus();
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    // In a map node, axe cannot decide the text's contrast: React Flow's layers between a node and
    // its pane take no pointer events (« bgOverlap »), and the edges, images for screen readers,
    // lie in the stack under the text (« imgNode »). That text, and only it, is measured here.
    const elsewhere = findings.undecided.filter(
      (u) => !u.inMapNode || (u.reason !== "bgOverlap" && u.reason !== "imgNode"),
    );
    expect(elsewhere).toEqual([]);
    const text = await textSamples(page, ".react-flow__node :is(span, li, .tim-box__name)");
    expect(text.length).toBeGreaterThan(10);
    expect(weak(text, 4.5)).toEqual([]);
    const found = await samples(page);
    expect(found.filter((s) => s.what.startsWith("stroke")).length).toBeGreaterThan(3);
    // The pins where edges leave and enter boxes.
    expect(found.filter((s) => s.what.startsWith("fill")).length).toBeGreaterThan(3);
    expect(weak(found)).toEqual([]);
    // Pointed at, each item in turn: its outline stands out, its wires take its colour; all
    // still at 3:1, and its text at 4.5:1.
    const items = page.locator(".react-flow__node");
    const count = await items.count();
    expect(count).toBeGreaterThan(6);
    for (let i = 0; i < count; i++) {
      // An open frame takes the pointer on its title bar only: its body lets it through.
      const item = items.nth(i);
      const frame = (await item.getAttribute("class"))?.includes("tim-map-node--frame");
      await (frame ? item.locator(".tim-map-frame__header") : item).hover();
      await settled(page);
      expect(weak(await samples(page))).toEqual([]);
      expect(weak(await textSamples(page, ".react-flow__node :is(span, li, .tim-box__name)"), 4.5)).toEqual([]);
    }
    await expect(page.locator(".tim-edge--active").first()).toBeAttached();
  });
}

/** The zoom of the map, from React Flow's viewport. */
async function zoom(page: Page): Promise<number> {
  const transform = await page.locator(".react-flow__viewport").evaluate((e) => getComputedStyle(e).transform);
  // matrix(a, b, c, d, e, f): `a` is the scale.
  return Number(/matrix\(([^,]+)/.exec(transform)?.[1] ?? "1");
}

test("+ and - zoom, as the zoom buttons do (PLAN.md 4.9, review m23)", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Interface" }).focus();
  const start = await zoom(page);
  await page.keyboard.press("+");
  await expect.poll(() => zoom(page)).toBeGreaterThan(start);
  const zoomed = await zoom(page);
  await page.keyboard.press("-");
  await expect.poll(() => zoom(page)).toBeLessThan(zoomed);
  const out = await zoom(page);
  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect.poll(() => zoom(page)).toBeGreaterThan(out);
  const back = await zoom(page);
  await page.getByRole("button", { name: "Zoom out" }).click();
  await expect.poll(() => zoom(page)).toBeLessThan(back);
});

test("Escape puts the details away (WCAG 1.4.13, review m15)", async ({ page }) => {
  await open(page);
  await page.locator(".react-flow__node", { hasText: "Persistence" }).focus();
  const details = page.locator(".tim-map-details");
  await expect(details).toContainText("Persistence");
  await page.keyboard.press("Escape");
  await expect(details).toHaveCount(0);
  // The next item focused shows its details again.
  await page.keyboard.press("ArrowUp");
  await expect(details).toHaveCount(1);
});

/** The id of the map node that has the focus. */
async function focused(page: Page): Promise<string | undefined> {
  return page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.id);
}

async function opened(page: Page): Promise<{ file: string; span: unknown } | undefined> {
  const value = await page.evaluate(() => document.body.dataset.opened);
  return value ? (JSON.parse(value) as { file: string; span: unknown }) : undefined;
}

test.describe("navigation (PLAN.md 4.9)", () => {
  test("the arrows move to the nearest box in their direction, Home to the first", async ({ page }) => {
    await open(page);
    await page.locator(".react-flow__node", { hasText: "Entry point" }).focus();
    expect(await focused(page)).toBe("@role/entry_point");
    await page.keyboard.press("ArrowRight");
    const right = await focused(page);
    expect(right).toMatch(/^@role\//);
    expect(right).not.toBe("@role/entry_point");
    const x = (id: string) =>
      page.locator(`.react-flow__node[data-id="${id}"]`).evaluate((e) => e.getBoundingClientRect().x);
    expect(await x(right ?? "")).toBeGreaterThan(await x("@role/entry_point"));
    await page.keyboard.press("ArrowLeft");
    expect(await focused(page)).toBe("@role/entry_point");
    await page.keyboard.press("Home");
    expect(await focused(page)).toBe("@role/entry_point");
  });

  test("Shift+Enter, a double click or the details open the code of a file", async ({ page }) => {
    await open(page);
    await page.locator(".react-flow__node", { hasText: "Persistence" }).click();
    // By its id: until the layout is done, the closed card still lists it.
    await page.locator('.react-flow__node[data-id="pkg:cargo/timonier-store"]').click();
    const file = page.locator('.react-flow__node[data-id^="file:crates/timonier-store/"]').first();
    await expect(file).toBeVisible();
    const id = await file.getAttribute("data-id");
    const path = id?.replace(/^file:/, "");
    await file.focus();
    await page.keyboard.press("Shift+Enter");
    expect((await opened(page))?.file).toBe(path);
    await page.evaluate(() => delete document.body.dataset.opened);
    await file.dblclick();
    expect((await opened(page))?.file).toBe(path);
    await page.evaluate(() => delete document.body.dataset.opened);
    await file.hover();
    await page.getByRole("button", { name: "Go to the code" }).click();
    expect((await opened(page))?.file).toBe(path);
  });

  test("a box says what Enter does with it", async ({ page }) => {
    await open(page);
    await page.locator(".react-flow__node", { hasText: "Persistence" }).click();
    // By its id: until the layout is done, the closed card still lists it.
    await page.locator('.react-flow__node[data-id="pkg:cargo/timonier-store"]').click();
    const label = await page.locator('.react-flow__node[data-id^="file:"]').first().getAttribute("aria-label");
    expect(label).toMatch(/Enter (opens it|opens its code)/);
  });
});

test.describe("level of detail (PLAN.md 4.10)", () => {
  test("a large map folds its deeper boxes, and a click shows what one holds", async ({ page }) => {
    // The first layout of 3,146 nodes takes seconds on a loaded machine: more than the 5 s an
    // action gets (review n° 17, m17-3). What is tested here is the folding, not its speed.
    test.slow();
    await page.goto("/demo.html?map=synthetic&size=20x12x12&open=all&theme=dark-modern");
    await page.waitForSelector(".react-flow__node", { timeout: 30_000 });
    // 3,146 nodes in the view: the files are folded into their components.
    await expect(page.locator('.react-flow__node[data-id^="file:"]')).toHaveCount(0);
    const component = page.locator('.react-flow__node[data-id="component:p0/c0"]');
    await expect(component).toContainText("12 folded");
    expect(await component.getAttribute("aria-label")).toContain("Folded to keep the map fluid");
    await component.focus();
    await page.keyboard.press("Enter");
    await expect(page.locator('.react-flow__node[data-id^="file:p0/c0/"]')).toHaveCount(12);
    await expect(page.locator('.react-flow__node[data-id="component:p0/c1"]')).toContainText("12 folded");
  });
});
