/**
 * Contrast ratios (WCAG 2.2, « relative luminance »), for the design system's tests (ADR 0020).
 * Colours come as browsers compute them: `rgb()`, `rgba()`, or `color(srgb …)` for a mix.
 */

/** Channels from 0 to 255, alpha from 0 to 1. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

const NUMBER = String.raw`[-+]?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?`;

/** A computed colour; `undefined` for what this does not read. */
export function parseColor(css: string): Rgba | undefined {
  const value = css.trim().toLowerCase();
  if (value === "transparent") return { r: 0, g: 0, b: 0, a: 0 };
  const hex = /^#([0-9a-f]{3,8})$/.exec(value)?.[1];
  if (hex) {
    const full = hex.length <= 4 ? [...hex].map((c) => c + c).join("") : hex;
    if (full.length !== 6 && full.length !== 8) return undefined;
    const channel = (i: number) => parseInt(full.slice(i, i + 2), 16);
    return { r: channel(0), g: channel(2), b: channel(4), a: full.length === 8 ? channel(6) / 255 : 1 };
  }
  const rgb = new RegExp(
    String.raw`^rgba?\(\s*(${NUMBER})[\s,]+(${NUMBER})[\s,]+(${NUMBER})(?:\s*[,/]\s*(${NUMBER})(%?))?\s*\)$`,
  ).exec(value);
  if (rgb) {
    const alpha = rgb[4] === undefined ? 1 : Number(rgb[4]) / (rgb[5] ? 100 : 1);
    return { r: Number(rgb[1]), g: Number(rgb[2]), b: Number(rgb[3]), a: alpha };
  }
  const srgb = new RegExp(
    String.raw`^color\(srgb\s+(${NUMBER})\s+(${NUMBER})\s+(${NUMBER})(?:\s*/\s*(${NUMBER})(%?))?\s*\)$`,
  ).exec(value);
  if (srgb) {
    const alpha = srgb[4] === undefined ? 1 : Number(srgb[4]) / (srgb[5] ? 100 : 1);
    return { r: Number(srgb[1]) * 255, g: Number(srgb[2]) * 255, b: Number(srgb[3]) * 255, a: alpha };
  }
  return undefined;
}

/** `top` painted over `bottom`. */
export function composite(top: Rgba, bottom: Rgba): Rgba {
  const a = top.a + bottom.a * (1 - top.a);
  if (a === 0) return { r: 0, g: 0, b: 0, a: 0 };
  const mix = (t: number, b: number) => (t * top.a + b * bottom.a * (1 - top.a)) / a;
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a };
}

function luminance({ r, g, b }: Rgba): number {
  const linear = (channel: number) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** The contrast ratio of two opaque colours, from 1 to 21. */
export function contrast(a: Rgba, b: Rgba): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (light + 0.05) / (dark + 0.05);
}
