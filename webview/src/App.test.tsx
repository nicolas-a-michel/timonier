import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import { resolveLocale } from "./i18n";
import { isHostMessage, NO_REVIEW, PROTOCOL_VERSION, readyMessage, subscribeTo, type ViewNode } from "./protocol";
import { initialModel, type ViewModel } from "./viewModel";

function withDaemon(state: ViewModel["daemon"]["state"], detail: string | null = null): ViewModel {
  return { ...initialModel, daemon: { state, detail } };
}

function withBoxes(count: number): ViewModel {
  const nodes = new Map<string, ViewNode>();
  for (let i = 0; i < count; i++) {
    const id = `pkg:cargo/p${i}`;
    nodes.set(id, {
      id,
      kind: "package",
      name: `p${i}`,
      parent: null,
      role: null,
      language: null,
      children: 0,
      file: null,
      span: null,
      signature: null,
      review: null,
      debt: null,
    });
  }
  return {
    ...withDaemon("connected"),
    view: { subscription: 1, revision: 0, nodes, edges: new Map(), review: NO_REVIEW },
  };
}

describe("teaching states", () => {
  it("speaks French when the IDE does", () => {
    const html = renderToStaticMarkup(<App locale="fr" />);
    expect(html).toContain("Démarrage de Timonier");
    expect(html).toContain("Le démon indexe le projet");
  });

  it("speaks English otherwise", () => {
    expect(renderToStaticMarkup(<App locale="en" />)).toContain("Starting Timonier");
  });

  it("says what to do when the daemon cannot start, with its reason", () => {
    const html = renderToStaticMarkup(
      <App locale="en" model={withDaemon("unavailable", "The program timonier was not found.")} />,
    );
    expect(html).toContain("Timonier cannot start");
    expect(html).toContain("Show the Log");
    expect(html).toContain("The program timonier was not found.");
  });

  it("says the map comes back by itself while the daemon restarts", () => {
    const html = renderToStaticMarkup(<App locale="fr" model={withDaemon("restarting", "connection closed")} />);
    expect(html).toContain("Reconnexion");
    expect(html).toContain("la carte revient d&#x27;elle-même");
  });

  it("draws the map once the daemon has sent boxes, and says when it reconnects", () => {
    expect(renderToStaticMarkup(<App locale="en" model={withBoxes(0)} />)).toContain("Nothing indexed yet");
    const map = renderToStaticMarkup(<App locale="fr" model={withBoxes(5)} />);
    expect(map).toContain('aria-label="Carte d&#x27;architecture"');
    expect(map).not.toContain('role="status"');
    const restarting = { ...withBoxes(2), daemon: { state: "restarting" as const, detail: null } };
    expect(renderToStaticMarkup(<App locale="en" model={restarting} />)).toMatch(/role="status"[^>]*>Reconnecting/);
  });

  it("names its region after its title and announces changes, for screen readers", () => {
    const html = renderToStaticMarkup(<App locale="en" />);
    const labelledBy = /<section[^>]*aria-labelledby="([^"]+)"/.exec(html)?.[1];
    expect(labelledBy).toBeTruthy();
    expect(html).toContain(`<h1 id="${labelledBy}"`);
    expect(html).toMatch(/role="status" aria-live="polite"/);
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

  // Every stylesheet of src/, subfolders included.
  const sources = readdirSync(import.meta.dirname, { recursive: true, encoding: "utf8" }).filter(
    (f) => f.endsWith(".css") && path.basename(f) !== "tokens.css",
  );

  it("checks at least one stylesheet", () => {
    expect(sources.length).toBeGreaterThan(0);
  });

  it.each(sources)("%s uses tokens, never raw colours", (file) => {
    expect(rawColours(readFileSync(path.join(import.meta.dirname, file), "utf8"))).toEqual([]);
  });
});

describe("protocol", () => {
  it("announces readiness with the protocol version, then asks for the top of the map", () => {
    expect(readyMessage()).toEqual({ type: "ready", protocol: PROTOCOL_VERSION });
    expect(subscribeTo()).toEqual({ type: "subscribe", view: { expanded: [] } });
    expect(subscribeTo(["pkg:cargo/a"])).toEqual({ type: "subscribe", view: { expanded: ["pkg:cargo/a"] } });
  });

  it("recognises the host's messages only", () => {
    expect(isHostMessage({ type: "daemon", state: "connected", detail: null })).toBe(true);
    expect(isHostMessage({ type: "snapshot" })).toBe(true);
    expect(isHostMessage({ type: "ready" })).toBe(false);
    expect(isHostMessage("snapshot")).toBe(false);
    expect(isHostMessage(null)).toBe(false);
  });
});
