import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import { resolveLocale } from "./i18n";
import { PROTOCOL_VERSION, readyMessage } from "./protocol";

describe("empty state", () => {
  it("speaks French when the IDE does", () => {
    const html = renderToStaticMarkup(<App locale="fr" />);
    expect(html).toContain("Pas encore de carte");
    expect(html).toContain("une fois le projet indexé");
  });

  it("speaks English otherwise", () => {
    expect(renderToStaticMarkup(<App locale="en" />)).toContain("No map yet");
  });

  it("names its region after its title, for screen readers", () => {
    const html = renderToStaticMarkup(<App locale="en" />);
    expect(html).toMatch(/<main[^>]*aria-labelledby="tim-empty-title"/);
    expect(html).toMatch(/<h1 id="tim-empty-title"/);
  });
});

describe("resolveLocale", () => {
  it.each([
    ["fr", "fr"],
    ["fr-CA", "fr"],
    ["FR", "fr"],
    ["en-US", "en"],
    ["de", "en"],
    ["", "en"],
    [undefined, "en"],
  ])("maps %s to %s", (tag, expected) => {
    expect(resolveLocale(tag)).toBe(expected);
  });
});

// Every colour-bearing declaration outside tokens.css must go through a --tim-* token.
const COLOUR_PROPERTY =
  /(?:^|[;{\s])((?:background|border|outline|text-decoration|caret)-color|color|background|border(?:-(?:top|right|bottom|left))?|outline|box-shadow|fill|stroke)\s*:\s*([^;}]+)/gi;
const RAW_COLOUR_FUNCTION = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color-mix|color)\(/i;
const KEYWORD = /^[a-z]+$/i;
const NOT_A_COLOUR = /^(inherit|initial|unset|transparent|none|currentcolor|solid|dashed|dotted|double|inset)$/i;

/** Returns the declarations of `css` that use a colour without going through a token. */
function rawColours(css: string): string[] {
  const code = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const offenders: string[] = [];
  for (const [, property = "", value = ""] of code.matchAll(COLOUR_PROPERTY)) {
    const withoutTokens = value.replace(/var\(--tim-[\w-]+\)/g, "");
    const namedColour = withoutTokens.split(/[\s,]+/).some((word) => KEYWORD.test(word) && !NOT_A_COLOUR.test(word));
    if (RAW_COLOUR_FUNCTION.test(withoutTokens) || namedColour || /var\(--(?!tim-)/.test(withoutTokens)) {
      offenders.push(`${property}: ${value.trim()}`);
    }
  }
  return offenders;
}

describe("design tokens", () => {
  it("the checker catches raw colours and accepts tokens", () => {
    expect(
      rawColours("a { color: red; background: #fff; border: 1px solid tomato; fill: oklch(0.5 0.1 0) }"),
    ).toHaveLength(4);
    expect(rawColours("a { color: var(--vscode-foreground) }")).toHaveLength(1);
    expect(
      rawColours("a { color: var(--tim-color-fg); border: 1px solid var(--tim-color-border); outline: none }"),
    ).toEqual([]);
  });

  const sources = readdirSync(import.meta.dirname).filter((f) => f.endsWith(".css") && f !== "tokens.css");

  it("checks at least one stylesheet", () => {
    expect(sources.length).toBeGreaterThan(0);
  });

  it.each(sources)("%s uses tokens, never raw colours", (file) => {
    expect(rawColours(readFileSync(path.join(import.meta.dirname, file), "utf8"))).toEqual([]);
  });
});

describe("protocol", () => {
  it("announces readiness with the protocol version", () => {
    expect(readyMessage()).toEqual({ type: "ready", protocol: PROTOCOL_VERSION });
  });
});
