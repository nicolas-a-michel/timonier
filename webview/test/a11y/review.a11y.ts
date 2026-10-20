/**
 * The review diff on the map (PLAN.md 5.3, ADR 0024): badges with a shape and words, never a
 * colour alone; the summary and its lists by keyboard; the filter « only the changes »; WCAG AA
 * in every captured theme.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, { theme = "dark-modern", locale = "en", review = true, only = false } = {}) {
  const extra = `${review ? "&review=1" : ""}${only ? "&only=1" : ""}`;
  await page.goto(`/demo.html?map=timonier&theme=${theme}&locale=${locale}${extra}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

/** The roles whose members changed, open: their boxes show their badges. */
async function openChangedRoles(page: Page) {
  for (const role of ["entry_point", "data_model", "persistence", "utility"]) {
    await page.locator(`.react-flow__node[data-id="@role/${role}"]`).click();
    await expect(page.locator(`.react-flow__node[data-id="@role/${role}"] .tim-map-frame`)).toBeVisible();
  }
}

const box = (page: Page, id: string) => page.locator(`.react-flow__node[data-id="${id}"]`);

test.describe.configure({ mode: "parallel" });

test("each changed box says how in words, and its name says it all", async ({ page }) => {
  await open(page);
  // Closed, a role counts its changed members, and its name says which.
  const entry = box(page, "@role/entry_point");
  await expect(entry.locator(".tim-map-changes")).toHaveText("1 modified");
  await expect(entry).toHaveAttribute("aria-label", /Changed since the last review: timonier-daemon\./);
  await openChangedRoles(page);
  await expect(box(page, "pkg:cargo/timonier-daemon").locator(".tim-box__flag")).toHaveText("Modified");
  await expect(box(page, "pkg:cargo/timonier-mcp").locator(".tim-box__flag")).toHaveText("New");
  // A cycle first, the fan-in in the details and the name.
  const core = box(page, "pkg:cargo/timonier-core");
  await expect(core.locator(".tim-box__flag")).toHaveText("Cycle");
  await expect(core).toHaveAttribute("aria-label", /7 boxes depend on it, against 4 at the review/);
  await expect(box(page, "pkg:cargo/timonier-cli").locator(".tim-box__flag")).toHaveCount(0);
  // The lines: new, or in the cycle, with a mark halfway and in their names.
  await expect(page.locator(".tim-edge-flag", { hasText: "Cycle" }).first()).toBeVisible();
  await expect(page.locator(".tim-edge-flag", { hasText: "New" })).toHaveCount(1);
  await expect(page.locator(".react-flow__edge[aria-label*='New since the last review']")).not.toHaveCount(0);
});

test("the banner above the map says what changed, and the legend what the marks mean (five-second test n° 1)", async ({
  page,
}) => {
  await open(page, { locale: "fr" });
  const banner = page.getByRole("region", { name: "Depuis la dernière revue" });
  const map = page.locator(".react-flow");
  const [top, board] = [await banner.boundingBox(), await map.boundingBox()];
  // Above the map, across it: read before the boxes.
  expect(top && board && top.y + top.height <= board.y + 1).toBe(true);
  expect(top && board && top.width >= board.width - 1).toBe(true);
  await expect(banner).toContainText("Depuis votre revue du");
  const legend = page.locator(".tim-legend");
  await expect(legend).toContainText("Changements depuis la revue");
  for (const word of ["Nouveau", "Modifié", "Dans un cycle ajouté"]) await expect(legend).toContainText(word);
  // No mark of a change looks like an action: a dot and a star, filled.
  await expect(page.locator(".tim-map-changes").first()).toContainText("modifié");
});

test("the summary counts the changes, and lists them on demand, by keyboard", async ({ page }) => {
  await open(page, { locale: "fr" });
  const summary = page.getByRole("region", { name: "Depuis la dernière revue" });
  await expect(summary).toContainText("Depuis votre revue du");
  await expect(summary).toContainText("1 nouvelle boîte · 4 modifiées · 1 supprimée");
  const details = summary.getByRole("button", { name: "Détails" });
  await details.focus();
  await page.keyboard.press("Enter");
  await expect(summary.getByRole("button", { name: "Masquer les détails" })).toHaveAttribute("aria-expanded", "true");
  // What is gone from the map is listed here only (Q26).
  await expect(summary.getByRole("heading", { name: "Supprimées" })).toBeVisible();
  await expect(summary).toContainText("timonier-legacy");
  await expect(summary).toContainText("timonier-cli → timonier-mcp");
});

test("the filter quiets what did not change, and says it is on", async ({ page }) => {
  await open(page);
  await openChangedRoles(page);
  const filter = page.getByRole("button", { name: "Highlight the changes" });
  await expect(filter).toHaveAttribute("aria-pressed", "false");
  await filter.click();
  await expect(filter).toHaveAttribute("aria-pressed", "true");
  // A tick, not its colours alone.
  await expect(filter.locator("svg")).toHaveCount(1);
  await expect(box(page, "pkg:cargo/timonier-cli").locator(".tim-box--dimmed")).toHaveCount(1);
  await expect(box(page, "pkg:cargo/timonier-daemon").locator(".tim-box--dimmed")).toHaveCount(0);
  await expect(page.locator(".tim-edge--quiet").first()).toBeAttached();
  await expect(page.locator(".tim-edge--review.tim-edge--quiet")).toHaveCount(0);
  // Nothing moved: the filter only quiets (ADR 0021).
  await filter.click();
  await expect(page.locator(".tim-box--dimmed")).toHaveCount(0);
});

test("before the first review, the summary says what to do; marked, the diff starts again", async ({ page }) => {
  await open(page, { review: false });
  const summary = page.getByRole("region", { name: "Since the last review" });
  await expect(summary).toContainText("No review yet");
  // Nothing to filter yet (Q27).
  await expect(summary.getByRole("button", { name: "Highlight the changes" })).toHaveCount(0);
  await expect(page.locator(".tim-box__flag, .tim-map-changes, .tim-edge-flag")).toHaveCount(0);
  await summary.getByRole("button", { name: "Mark as reviewed" }).click();
  await expect(summary).toContainText("Nothing has changed since.");
  await expect(summary.getByRole("button", { name: "Highlight the changes" })).toHaveCount(1);
});

test("the filter, on before the first review, quiets nothing (Q27, review n° 1, I2)", async ({ page }) => {
  await open(page, { review: false, only: true });
  await page.locator('.react-flow__node[data-id="@role/entry_point"]').click();
  await expect(page.locator(".tim-box").first()).toBeVisible();
  await expect(page.locator(".tim-box--dimmed, .tim-map--quiet, .tim-edge--quiet")).toHaveCount(0);
});

test("marking a review with changes empties the diff", async ({ page }) => {
  await open(page);
  await expect(page.locator(".tim-map-changes")).not.toHaveCount(0);
  await page.getByRole("button", { name: "Mark as reviewed" }).click();
  await expect(page.locator("body")).toHaveAttribute("data-reviewed", "true");
  await expect(page.locator(".tim-map-changes, .tim-edge-flag")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Since the last review" })).toContainText("Nothing has changed since.");
});

for (const theme of themes) {
  test(`${theme}: the review diff at WCAG AA, its marks at 3:1`, async ({ page }) => {
    // Tall enough for the summary's lists to show whole: axe judges no text it cannot see.
    await page.setViewportSize({ width: 1100, height: 1600 });
    await open(page, { theme });
    await openChangedRoles(page);
    await page.getByRole("button", { name: "Details" }).click();
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    // As on the map's own test: where axe cannot decide, the text is measured below. Inside map
    // nodes (« bgOverlap », « imgNode »), a count of one figure (« shortTextContent »), the
    // marks of the lines, on the layer of their labels over the map (« bgOverlap »), and the
    // summary, whose opaque panel lies over the boxes (« elmPartiallyObscuring »).
    const measuredHere = (u: { inMapNode: boolean; reason: string | undefined; target: string }) =>
      (u.inMapNode && ["bgOverlap", "imgNode", "shortTextContent"].includes(u.reason ?? "")) ||
      (u.target.includes(".tim-edge-flag") && u.reason === "bgOverlap") ||
      (u.target.includes(".tim-review") && u.reason === "elmPartiallyObscuring");
    expect(findings.undecided.filter((u) => !measuredHere(u))).toEqual([]);
    const text = await textSamples(
      page,
      ".react-flow__node :is(span, li, .tim-box__name), .tim-edge-flag span, .tim-review :is(p, h3, li)",
    );
    expect(text.length).toBeGreaterThan(10);
    expect(weak(text, 4.5)).toEqual([]);
    const found = await samples(page);
    // The shapes of the badges, and the lines new or in the cycle.
    expect(found.filter((s) => s.what.startsWith("icon")).length).toBeGreaterThan(5);
    expect(weak(found)).toEqual([]);
    // Filtered: what stays in view keeps its contrast.
    await page.getByRole("button", { name: "Highlight the changes" }).click();
    await settled(page);
    expect(weak(await textSamples(page, ".react-flow__node :is(span, li, .tim-box__name)"), 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}

/** Whether two boxes of the screen overlap. */
function overlap(
  a: { x: number; y: number; width: number; height: number },
  b: { x: number; y: number; width: number; height: number },
): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

test("beside the editor, at 600 px, the panels hide neither each other nor the boxes (review n° 1, m6)", async ({
  page,
}) => {
  await page.setViewportSize({ width: 600, height: 500 });
  await open(page, { locale: "fr" });
  await settled(page);
  const rect = async (selector: string) => {
    const box = await page.locator(selector).first().boundingBox();
    if (!box) throw new Error(`${selector} is not on screen`);
    return box;
  };
  const toolbar = await rect(".tim-toolbar");
  const legend = await rect(".tim-legend");
  const summary = await rect(".tim-review");
  expect(overlap(toolbar, legend)).toBe(false);
  for (const node of await page.locator(".react-flow__node").all()) {
    const box = await node.boundingBox();
    if (box) expect(overlap(box, summary), (await node.getAttribute("data-id")) ?? undefined).toBe(false);
  }
  await page.locator('.react-flow__node[data-id="@role/interface"]').hover();
  const details = await rect(".tim-map-details");
  expect(overlap(details, await rect(".tim-review"))).toBe(false);
});
