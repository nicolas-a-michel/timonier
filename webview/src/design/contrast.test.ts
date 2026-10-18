import { describe, expect, it } from "vitest";

import { composite, contrast, parseColor } from "./contrast";

describe("contrast", () => {
  it("reads the colours browsers compute", () => {
    expect(parseColor("rgb(255, 255, 255)")).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(parseColor("rgba(204, 204, 204, 0.7)")).toEqual({ r: 204, g: 204, b: 204, a: 0.7 });
    expect(parseColor("rgb(0 0 0 / 50%)")).toEqual({ r: 0, g: 0, b: 0, a: 0.5 });
    expect(parseColor("color(srgb 1 0.5 0)")).toEqual({ r: 255, g: 127.5, b: 0, a: 1 });
    expect(parseColor("#0f0")).toEqual({ r: 0, g: 255, b: 0, a: 1 });
    expect(parseColor("#00000080")?.a).toBeCloseTo(0.5, 2);
    expect(parseColor("transparent")?.a).toBe(0);
    expect(parseColor("oklch(0.5 0.1 0)")).toBeUndefined();
  });

  it("gives the ratios of WCAG", () => {
    const white = { r: 255, g: 255, b: 255, a: 1 };
    const black = { r: 0, g: 0, b: 0, a: 1 };
    expect(contrast(white, black)).toBeCloseTo(21, 5);
    expect(contrast(black, white)).toBeCloseTo(21, 5);
    // #767676 on white: the classic 4.54:1.
    expect(contrast({ r: 0x76, g: 0x76, b: 0x76, a: 1 }, white)).toBeCloseTo(4.54, 2);
  });

  it("paints a translucent colour over its background first", () => {
    const half = composite({ r: 0, g: 0, b: 0, a: 0.5 }, { r: 255, g: 255, b: 255, a: 1 });
    expect(half.a).toBe(1);
    expect(half.r).toBeCloseTo(127.5, 5);
  });
});
