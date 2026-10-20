import type { Density } from "./protocol";

/**
 * Applies the density the host gives (Q32, ADR 0026) to the page: the tokens follow from
 * `data-density` on <html>; the layout, from the density given back to the map.
 */
export function applyDensity(density: Density, root: Pick<HTMLElement, "dataset"> = document.documentElement): Density {
  root.dataset.density = density;
  return density;
}
