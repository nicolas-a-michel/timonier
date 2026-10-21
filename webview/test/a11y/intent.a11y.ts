/**
 * The map compared with the whiteboard (PLAN.md 7.6, ADR 0042): a toggle of the toolbar, a
 * banner that lists what is missing and what is crossed, the drawn components on their boxes, the
 * line that crosses a forbidden edge marked in words; WCAG AA in every captured theme.
 */
import { expect, test, type Page } from "@playwright/test";

import { axeFindings, samples, settled, textSamples, themes, weak } from "./measure";

async function open(page: Page, { theme = "dark-modern", locale = "en", intent = false, extra = "" } = {}) {
  await page.goto(`/demo.html?map=timonier&theme=${theme}&locale=${locale}${intent ? "&intent=1" : ""}${extra}`);
  await page.waitForSelector("body[data-ready='true']", { state: "attached" });
  await page.waitForSelector(".react-flow__node");
}

const box = (page: Page, id: string) => page.locator(`.react-flow__node[data-id="${id}"]`);
const banner = (page: Page) => page.getByRole("region", { name: "The map compared with the whiteboard" });

test.describe.configure({ mode: "parallel" });

test("the toolbar turns the comparison on and off, and the banner says the drift", async ({ page }) => {
  await open(page);
  await expect(banner(page)).toHaveCount(0);
  const toggle = page.getByRole("button", { name: "Compare with the intent" });
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(banner(page)).toContainText(
    "6 drawn components, 5 found in the code, 1 missing · 1 forbidden edge crossed · 1 required edge missing",
  );
  await expect(banner(page).getByRole("heading", { name: "Drawn, not in the code" })).toBeVisible();
  await expect(banner(page)).toContainText(
    "Core → Storage: 2 dependencies (timonier-daemon → timonier-store, timonier-core → timonier-store)",
  );
  await expect(banner(page)).toContainText("Store → Daemon");
  await banner(page).getByRole("button", { name: "Open the whiteboard" }).click();
  await expect(page.locator("body")).toHaveAttribute("data-whiteboard", "true");
  await toggle.click();
  await expect(banner(page)).toHaveCount(0);
});

/** A box's whiteboard icon is drawn whole, inside the box's frame, with some width. */
async function markFits(page: Page, id: string, kind: "drawn" | "within") {
  const node = box(page, id);
  const mark = node.locator(`.tim-intent-mark--${kind}`);
  await expect(mark).toHaveCount(1);
  // Once the layout has settled: not halfway through a move.
  await settled(page);
  const frame = await node.locator(".tim-box").boundingBox();
  const icon = await mark.boundingBox();
  expect(icon && icon.width > 8 && icon.height > 8, `${id}: the icon has a size`).toBe(true);
  expect(
    frame &&
      icon &&
      icon.x >= frame.x &&
      icon.y >= frame.y &&
      icon.x + icon.width <= frame.x + frame.width &&
      icon.y + icon.height <= frame.y + frame.height,
    `${id}: the icon is inside the frame`,
  ).toBe(true);
  // Beside the review's mark, never over it (review n° 4, m-2).
  const review = node.locator(".tim-box__flag > :not(.tim-intent-mark)");
  if ((await review.count()) > 0) {
    const flagBox = await review.first().boundingBox();
    expect(flagBox && icon && flagBox.x + flagBox.width <= icon.x, `${id}: the marks do not overlap`).toBe(true);
  }
}

test("a drawn component shows on its role, then on its box; the forbidden crossing on its line", async ({ page }) => {
  await open(page, { intent: true });
  const entry = box(page, "@role/entry_point");
  // Member by member, for screen readers (review n° 4, m-5).
  await expect(entry).toHaveAttribute("aria-label", /timonier-daemon: Drawn: Daemon\./);
  await expect(entry).toHaveAttribute("aria-label", /timonier-cli: Drawn: Cli\./);
  await expect(box(page, "@role/utility")).toHaveAttribute("aria-label", /timonier-index: Holds drawn: Components\./);
  // An icon per member, its shape by what it is (review n° 3, m-1); no line more: the last
  // member stays whole (review n° 2, I-2).
  await expect(entry.locator(".tim-map-role__members .tim-intent-mark--drawn")).toHaveCount(2);
  await expect(box(page, "@role/utility").locator(".tim-map-role__members .tim-intent-mark--within")).toHaveCount(1);
  for (const role of ["entry_point", "persistence"]) {
    const card = await box(page, `@role/${role}`).locator(".tim-map-role").boundingBox();
    const last = await box(page, `@role/${role}`).locator(".tim-map-role__members li").last().boundingBox();
    expect(card && last && last.y + last.height <= card.y + card.height + 0.5, role).toBe(true);
  }
  await entry.click();
  const daemon = box(page, "pkg:cargo/timonier-daemon");
  await markFits(page, "pkg:cargo/timonier-daemon", "drawn");
  await expect(daemon).toHaveAttribute("aria-label", /Drawn: Daemon\./);
  await expect(daemon.locator(".tim-intent-mark")).toHaveAttribute("title", "Drawn: Daemon");
  await daemon.hover();
  await expect(page.locator(".tim-map-details")).toContainText("Drawn: Daemon.");
  await box(page, "@role/persistence").click();
  // The line from the daemon to the store crosses the forbidden edge: the tone, and the word.
  await expect(page.locator(".tim-edge--forbidden").first()).toBeAttached();
  await expect(page.locator(".tim-edge-flag--forbidden").first()).toHaveText("forbidden");
  await expect(page.locator(".tim-legend")).toContainText("a link that crosses a forbidden edge");
  await expect(page.locator(".tim-legend")).toContainText("a box that holds one, deeper");
});

test("a link both forbidden and new since the review keeps both marks, and is not thicker", async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 1000 });
  await open(page, { intent: true, extra: "&review=1" });
  for (const role of ["data_model", "persistence"]) await box(page, `@role/${role}`).dispatchEvent("click");
  // core → store is new, and in a cycle added: its review mark, as its icon, beside the word.
  const flag = page.locator(".tim-edge-flag--forbidden").filter({ has: page.locator(".tim-edge-flag__review") });
  await expect(flag).toHaveCount(1);
  await expect(flag).toContainText("forbidden");

  // Thickness is the traffic's (ADR 0021): the forbidden line keeps the width of the others.
  // Measured once the map is drawn: a line being replaced has no computed width yet (CI, Ubuntu).
  await expect
    .poll(() =>
      page
        .locator(".react-flow__edge-path.tim-edge")
        .evaluateAll((paths) => [
          ...new Set(paths.filter((p) => p.isConnected).map((p) => getComputedStyle(p).strokeWidth)),
        ]),
    )
    .toEqual(["1.5px"]);
});

/**
 * What is wrong with the « forbidden » words on the map (reviews n° 6 and 7 of Phase 7): a word
 * not seen whole (a box or another label over it), a word that covers a name, a mark or a title
 * bar, a word on a stretch of its line that a box hides, a word never measured.
 */
async function forbiddenWordsWrong(page: Page): Promise<string[]> {
  await settled(page);
  return page.evaluate(() => {
    const wrong: string[] = [];
    const flags = [...document.querySelectorAll<HTMLElement>(".tim-edge-flag")];
    const words = [...document.querySelectorAll<HTMLElement>(".tim-edge-flag--forbidden")];
    if (words.length === 0) wrong.push("no forbidden word");
    const titled = [
      // An open frame's title bar spans the frame, mostly empty: its words and marks count.
      ...document.querySelectorAll(
        ".tim-box__name, .tim-box__flag, .tim-map-role__title, .tim-map-frame__title, .tim-map-frame__meta, " +
          ".tim-map-frame__header .tim-map-changes, .tim-map-frame__header .tim-badge, .tim-map-frame__header .tim-intent-mark",
      ),
    ];
    const titles = titled.map((e) => e.getBoundingClientRect());
    const crosses = (a: DOMRect, b: DOMRect) =>
      Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 &&
      Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1;
    for (const word of words) {
      const r = word.getBoundingClientRect();
      const where = `${Math.round(r.left)},${Math.round(r.top)}`;
      // On a stretch of its line that no box hides: its middle is on the line.
      const under = document
        .elementsFromPoint(r.left + r.width / 2, r.top + r.height / 2)
        .find((e) => e.closest(".react-flow__node"));
      if (under?.closest(".tim-box, .tim-map-role, .tim-map-frame__header")) wrong.push(`${where}: its line is hidden`);
      const covered = titled.filter((_, i) => crosses(r, titles[i] as DOMRect));
      if (covered.length > 0) {
        const names = covered.map(
          (e) => `${e.getAttribute("class")} of ${e.closest(".react-flow__node")?.getAttribute("data-id")}`,
        );
        wrong.push(`${where}: covers ${names.join(", ")}`);
      }
    }
    // Seen whole: every label takes the pointer while measured, one over another is then found.
    for (const flag of flags) flag.style.pointerEvents = "auto";
    for (const word of words) {
      const badge = word.querySelector(".tim-badge");
      if (!badge) continue;
      const r = badge.getBoundingClientRect();
      let measured = 0;
      for (const f of [0.08, 0.25, 0.5, 0.75, 0.92]) {
        const [x, y] = [r.left + r.width * f, r.top + r.height / 2];
        if (x < 0 || y < 0 || x > innerWidth || y > innerHeight) continue;
        const hit = document.elementFromPoint(x, y);
        if (hit?.closest(".react-flow__panel")) continue;
        measured++;
        if (!badge.contains(hit))
          wrong.push(
            `${Math.round(r.left)},${Math.round(r.top)} (${f}): under ${hit?.tagName}.${hit?.getAttribute("class")}`,
          );
      }
      if (measured === 0) wrong.push(`${Math.round(r.left)},${Math.round(r.top)}: never measured`);
    }
    for (const flag of flags) flag.style.pointerEvents = "";
    return wrong;
  });
}

// Reviews n° 6 and 7 of Phase 7: React Flow draws the boxes over the lines and their labels, and
// opposite lines share their middle; the word goes on a stretch of its line no box hides, over
// the boxes, covering the least and no title bar.
for (const [width, height, density] of [
  [900, 700, "comfortable"],
  [1100, 900, "comfortable"],
  [1400, 1000, "comfortable"],
  [1800, 1000, "comfortable"],
  [1100, 900, "compact"],
] as const) {
  test(`${width}×${height}, ${density}: every « forbidden » word is seen, on its line, off the titles`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await open(page, { intent: true, extra: `&review=1&density=${density}` });
    for (const role of ["data_model", "persistence", "entry_point"]) {
      await box(page, `@role/${role}`).dispatchEvent("click");
    }
    // The whole map in view: every word is measured.
    await settled(page);
    await page.getByRole("button", { name: "Fit to the window" }).click();
    await expect(page.locator(".tim-edge-flag--forbidden").first()).toBeAttached();
    expect(await forbiddenWordsWrong(page)).toEqual([]);
  });
}

// Review n° 3 of Phase 7, I-1: with the debt layer, the review, compact or folded, the mark stays.
for (const [what, extra] of [
  ["the debt layer and the review", "&debt=1&review=1"],
  ["compact, with the review", "&review=1&density=compact"],
] as const) {
  for (const locale of ["en", "fr"]) {
    test(`${locale}: the mark of a closed box stays whole with ${what}`, async ({ page }) => {
      await open(page, { locale, intent: true, extra });
      // The second group may lie under the first once open: a click its own, not the pointer's.
      await box(page, "@role/entry_point").dispatchEvent("click");
      await box(page, "@role/utility").dispatchEvent("click");
      await markFits(page, "pkg:cargo/timonier-daemon", "drawn");
      await markFits(page, "pkg:cargo/timonier-index", "within");
    });
  }
}

test("speaks French when the IDE does", async ({ page }) => {
  await open(page, { locale: "fr", intent: true });
  await box(page, "@role/utility").click();
  await expect(box(page, "pkg:cargo/timonier-index")).toHaveAttribute(
    "aria-label",
    /timonier-index : Contient du dessiné : Components\.|Contient du dessiné : Components\./,
  );
  await expect(page.getByRole("region", { name: "La carte comparée au tableau blanc" })).toContainText(
    "6 composants dessinés, 5 trouvés dans le code, 1 manquant",
  );
});

for (const theme of themes) {
  test(`${theme}: the comparison at WCAG AA`, async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 1000 });
    await open(page, { theme, intent: true });
    for (const role of ["entry_point", "persistence"]) await box(page, `@role/${role}`).click();
    await settled(page);
    const findings = await axeFindings(page);
    expect(findings.violations).toEqual([]);
    const text = await textSamples(page, ".tim-review--intent :is(p, li, h3, strong, button), .tim-edge-flag");
    expect(text.length).toBeGreaterThan(5);
    expect(weak(text, 4.5)).toEqual([]);
    expect(weak(await samples(page))).toEqual([]);
  });
}
