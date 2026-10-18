import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { Badge, ComponentBox, EdgeLegend, EdgeLine, EmptyState, rovingIndex, Toolbar, Tooltip } from "./components";

describe("component box", () => {
  it("is named for screen readers by its name, role and size", () => {
    const html = renderToStaticMarkup(<ComponentBox name="timonier-store" role="persistence" items={3} locale="en" />);
    expect(html).toContain('aria-label="timonier-store, Persistence, 3 items"');
    expect(html).toContain('tabindex="0"');
    // The role is written, not only drawn.
    expect(html).toContain('<span class="tim-box__role">Persistence</span>');
    expect(html).toMatch(/<svg[^>]*aria-hidden="true"/);
  });

  it("speaks French, and says when there is no role yet", () => {
    const html = renderToStaticMarkup(<ComponentBox name="x" role={null} items={1} locale="fr" />);
    expect(html).toContain('aria-label="x, Pas encore de rôle, 1 élément"');
  });

  it("marks the selected box, not by colour alone", () => {
    const html = renderToStaticMarkup(<ComponentBox name="x" role="utility" items={0} locale="en" selected />);
    expect(html).toContain('aria-current="true"');
    expect(html).toContain("tim-box--selected");
  });
});

describe("edges", () => {
  it("are dashed when approximate, solid when confirmed, and named", () => {
    const svg = (approximate: boolean) =>
      renderToStaticMarkup(
        <svg>
          <EdgeLine d="M0 0H10" approximate={approximate} label="a → b" />
        </svg>,
      );
    expect(svg(true)).toContain("tim-edge--approximate");
    expect(svg(false)).toContain("tim-edge--confirmed");
    expect(svg(false)).toContain('aria-label="a → b"');
    expect(svg(false)).toContain("<title>a → b</title>");
  });

  it("have a legend in words", () => {
    const html = renderToStaticMarkup(<EdgeLegend locale="fr" />);
    expect(html).toContain("Sûre : déclarée dans un manifeste, ou confirmée par un serveur de langage");
    expect(html).toContain("Vue par tree-sitter seulement : approchée");
  });
});

describe("badges", () => {
  it("draw a different shape for each tone, next to their words", () => {
    const shapes = (["neutral", "violation", "attention", "conformity"] as const).map((tone) =>
      renderToStaticMarkup(<Badge tone={tone} label="words" />).replace(/class="[^"]*"/g, ""),
    );
    expect(new Set(shapes).size).toBe(4);
    for (const shape of shapes) expect(shape).toContain("<span>words</span>");
  });
});

describe("empty state", () => {
  it("names its region by its title, and offers what to do next", () => {
    const html = renderToStaticMarkup(
      <EmptyState
        title="Nothing yet"
        text="Open a folder."
        action={{ label: "Open", onSelect: () => undefined }}
        live
      />,
    );
    const id = /aria-labelledby="([^"]+)"/.exec(html)?.[1];
    expect(html).toContain(`<h1 id="${id}"`);
    expect(html).toContain('role="status"');
    expect(html).toContain('<button type="button" class="tim-button">Open</button>');
  });
});

describe("tooltip", () => {
  it("describes its anchor, and stays hidden until asked", () => {
    const html = renderToStaticMarkup(
      <Tooltip text="Opens the file">
        <button type="button">Open</button>
      </Tooltip>,
    );
    const id = /aria-describedby="([^"]+)"/.exec(html)?.[1];
    expect(html).toContain(`role="tooltip" id="${id}"`);
    expect(html).toMatch(/role="tooltip"[^>]*hidden=""/);
  });
});

describe("toolbar", () => {
  it("has one tab stop, and says which toggle is on", () => {
    const html = renderToStaticMarkup(
      <Toolbar
        label="Density"
        items={[
          { id: "a", label: "Comfortable", pressed: true },
          { id: "b", label: "Compact", pressed: false },
        ]}
      />,
    );
    expect(html).toContain('role="toolbar" aria-label="Density"');
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html.match(/tabindex="-1"/g)).toHaveLength(1);
    expect(html).toContain('aria-pressed="true"');
  });

  it("moves the focus with the arrows, Home and End, round the ends", () => {
    expect(rovingIndex(0, "ArrowRight", 3)).toBe(1);
    expect(rovingIndex(2, "ArrowRight", 3)).toBe(0);
    expect(rovingIndex(0, "ArrowLeft", 3)).toBe(2);
    expect(rovingIndex(1, "Home", 3)).toBe(0);
    expect(rovingIndex(1, "End", 3)).toBe(2);
    expect(rovingIndex(1, "a", 3)).toBeUndefined();
    expect(rovingIndex(0, "ArrowRight", 0)).toBeUndefined();
  });
});
