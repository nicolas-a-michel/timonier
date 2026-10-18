/**
 * What the accessibility tests measure (ADR 0020): axe-core's verdict, and the 3:1 contrast of
 * lines, icons and the focus ring that axe does not measure.
 */
import { readdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

import type { Page } from "@playwright/test";

import { composite, contrast, parseColor, type Rgba } from "../../src/design/contrast";

/** The VSCode themes captured in test/themes. */
export const themes = readdirSync(path.join(import.meta.dirname, "..", "themes"))
  .filter((file) => file.endsWith(".json"))
  .map((file) => file.replace(/\.json$/, ""))
  .sort();

const axe = createRequire(import.meta.url).resolve("axe-core/axe.min.js");

/**
 * Once the colours and the places have settled, not halfway through a transition: a frame goes
 * by, for the transitions a click or a hover starts, then they all end.
 */
export async function settled(page: Page): Promise<void> {
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.waitForFunction(() => document.getAnimations().length === 0);
}

/** A contrast axe could not decide: where, why, and whether it is text of a map node. */
export interface Undecided {
  target: string;
  reason: string | undefined;
  inMapNode: boolean;
}

function describe(rule: { id: string; targets: string[] }): string {
  return `${rule.id}: ${rule.targets.join(", ")}`;
}

/** The violations of WCAG A and AA axe-core finds, and the contrasts it could not decide. */
export async function axeFindings(page: Page): Promise<{ violations: string[]; undecided: Undecided[] }> {
  await page.addScriptTag({ path: axe });
  const found = await page.evaluate(async () => {
    interface Node {
      target: string[];
      element?: Element;
      any: { data?: { messageKey?: string } | null }[];
    }
    interface Rule {
      id: string;
      nodes: Node[];
    }
    const run = (
      window as unknown as {
        axe: { run: (context: Document, options: object) => Promise<{ violations: Rule[]; incomplete: Rule[] }> };
      }
    ).axe.run;
    const result = await run(document, {
      runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] },
      elementRef: true,
    });
    return {
      violations: result.violations.map((rule) => ({
        id: rule.id,
        targets: rule.nodes.map((n) => n.target.join(" ")),
      })),
      undecided: result.incomplete
        .filter((rule) => rule.id === "color-contrast")
        .flatMap((rule) =>
          rule.nodes.map((node) => ({
            target: node.target.join(" "),
            reason: node.any[0]?.data?.messageKey,
            inMapNode: !!node.element?.closest(".react-flow__node"),
          })),
        ),
    };
  });
  return { violations: found.violations.map(describe), undecided: found.undecided };
}

/** The colour of the text of `selector`s, over what is behind them. */
export async function textSamples(page: Page, selector: string): Promise<Sample[]> {
  return page.evaluate<Sample[], string>((selector) => {
    const found: Sample[] = [];
    for (const element of document.querySelectorAll(selector)) {
      const layers: string[] = [];
      for (let e: Element | null = element; e; e = e.parentElement) {
        const colour = getComputedStyle(e).backgroundColor;
        if (colour && colour !== "rgba(0, 0, 0, 0)" && colour !== "transparent") layers.push(colour);
      }
      found.push({
        what: `text of ${selector}: ${element.textContent?.trim().slice(0, 40)}`,
        colour: getComputedStyle(element).color,
        backgrounds: layers,
      });
    }
    return found;
  }, selector);
}

/** A colour and every background behind it, from the nearest. */
export interface Sample {
  what: string;
  colour: string;
  backgrounds: string[];
}

/** The colours of what carries `data-contrast`, and of the focused element's ring. */
export async function samples(page: Page): Promise<Sample[]> {
  return page.evaluate<Sample[]>(() => {
    const backgrounds = (element: Element | null): string[] => {
      const layers: string[] = [];
      for (let e = element; e; e = e.parentElement) {
        const colour = getComputedStyle(e).backgroundColor;
        if (colour && colour !== "rgba(0, 0, 0, 0)" && colour !== "transparent") layers.push(colour);
      }
      return layers;
    };
    const found: Sample[] = [];
    for (const element of document.querySelectorAll("[data-contrast]")) {
      const kind = element.getAttribute("data-contrast");
      const what = `${kind} of ${element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40)}`;
      if (kind === "outline") {
        found.push({
          what,
          colour: getComputedStyle(element).borderTopColor,
          backgrounds: backgrounds(element.parentElement),
        });
      } else if (kind === "stroke") {
        found.push({ what, colour: getComputedStyle(element).stroke, backgrounds: backgrounds(element.parentElement) });
      } else if (kind === "fill") {
        // A shape drawn by its fill: a pin of the map (HTML), or of the legend (SVG).
        const style = getComputedStyle(element);
        const colour = element instanceof SVGElement ? style.fill : style.backgroundColor;
        found.push({ what, colour, backgrounds: backgrounds(element.parentElement) });
        // A pin sits on the edge of its box: half over the canvas, half over the title bar.
        const bar = element
          .closest(".react-flow__node")
          ?.querySelector(".tim-box__header, .tim-map-role__title, .tim-map-frame__header");
        if (bar) found.push({ what: `${what}, over its title bar`, colour, backgrounds: backgrounds(bar) });
      } else if (kind === "icon") {
        const icon = element.querySelector(".tim-icon");
        if (icon) found.push({ what, colour: getComputedStyle(icon).color, backgrounds: backgrounds(element) });
      }
    }
    const focused = document.activeElement;
    if (focused && focused !== document.body) {
      found.push({
        what: "focus ring",
        colour: getComputedStyle(focused).outlineColor,
        backgrounds: backgrounds(focused.parentElement),
      });
    }
    return found;
  });
}

function opaque(sample: Sample): { colour: Rgba; background: Rgba } {
  let background: Rgba = { r: 255, g: 255, b: 255, a: 1 };
  for (const layer of [...sample.backgrounds].reverse()) {
    const parsed = parseColor(layer);
    if (!parsed) throw new Error(`${sample.what}: cannot read the background ${layer}`);
    background = composite(parsed, background);
  }
  const colour = parseColor(sample.colour);
  if (!colour) throw new Error(`${sample.what}: cannot read the colour ${sample.colour}`);
  return { colour: composite(colour, background), background };
}

/** The samples under `minimum`, with their ratio. */
export function weak(found: Sample[], minimum = 3): string[] {
  return found
    .map((sample) => {
      const { colour, background } = opaque(sample);
      return { what: sample.what, ratio: contrast(colour, background) };
    })
    .filter(({ ratio }) => ratio < minimum)
    .map(({ what, ratio }) => `${what}: ${ratio.toFixed(2)}:1`);
}
