/**
 * Curves drawn with uPlot (ADR 0051): the colours of the theme's tokens, read once drawn and
 * again when the theme changes; a table carries the same numbers for screen readers, the
 * canvas being an image to them.
 */
import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

export interface Curve {
  label: string;
  values: (number | null)[];
  /** A token of tokens.css: `--tim-perf-series-1`. */
  color: string;
  /** Its own axis, on the right. */
  scale: string;
  format: (value: number) => string;
}

export interface CurvesProps {
  /** What the image says, for screen readers. */
  label: string;
  xs: number[];
  /** What `x` is (« Time », « Run »), and how to say one. */
  xLabel: string;
  formatX: (x: number) => string;
  curves: Curve[];
  height?: number;
}

/**
 * A token's colour, resolved: a custom property reads as written (`var(--vscode-…)`), so a probe
 * takes it as its colour and gives it back computed, as the canvas needs it.
 */
function resolve(probe: HTMLElement, name: string): string {
  probe.style.color = `var(${name})`;
  return getComputedStyle(probe).color;
}

export function Curves({ label, xs, xLabel, formatX, curves, height = 160 }: CurvesProps) {
  const holder = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const target = holder.current;
    if (!target) return;
    const probe = document.createElement("span");
    probe.setAttribute("aria-hidden", "true");
    target.appendChild(probe);
    const color = (name: string) => resolve(probe, name);
    const muted = color("--tim-color-muted");
    const grid = color("--tim-grid-major");
    // The panel's font, as the holder computes it (`--tim-font-size-small` is a calc()).
    const style = getComputedStyle(target);
    const font = `${style.fontSize} ${style.fontFamily}`;
    const axis = (scale: string, side: uPlot.Axis.Side, format: (v: number) => string): uPlot.Axis => ({
      scale,
      side,
      stroke: muted,
      font,
      grid: { stroke: grid, width: 1 },
      ticks: { stroke: grid, width: 1 },
      // Room for « 180 MB », « 500 ms »: uPlot's default cuts them.
      size: scale === "x" ? 36 : 72,
      values: (_, ticks) => ticks.map(format),
    });
    // One axis per scale: two curves in seconds share theirs.
    const scales = [...new Map(curves.map((c) => [c.scale, c])).values()];
    const plot = new uPlot(
      {
        width: target.clientWidth || 600,
        height,
        legend: { show: true },
        cursor: { drag: { x: false, y: false } },
        scales: { x: { time: false } },
        series: [
          { label: xLabel, value: (_, v) => (v === null ? "" : formatX(v)) },
          ...curves.map((curve): uPlot.Series => ({
            label: curve.label,
            scale: curve.scale,
            stroke: color(curve.color),
            width: 2,
            points: { show: xs.length < 40 },
            value: (_, v) => (v === null ? "—" : curve.format(v)),
          })),
        ],
        axes: [axis("x", 2, formatX), ...scales.map((curve, i) => axis(curve.scale, i === 0 ? 3 : 1, curve.format))],
      },
      [xs, ...curves.map((c) => c.values)],
      target,
    );
    const resize = new ResizeObserver(() => plot.setSize({ width: target.clientWidth || 600, height }));
    resize.observe(target);
    // VSCode changes the theme's variables in place: the colours are read again.
    const theme = new MutationObserver(() => {
      plot.series.forEach((series, i) => {
        const curve = curves[i - 1];
        if (curve) series.stroke = () => color(curve.color);
      });
      plot.redraw();
    });
    theme.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    return () => {
      resize.disconnect();
      theme.disconnect();
      plot.destroy();
      probe.remove();
    };
  }, [xs, curves, xLabel, formatX, height]);
  return (
    <figure className="tim-curves">
      <div ref={holder} className="tim-curves__plot" role="img" aria-label={label} />
      <table className="tim-visually-hidden">
        <caption>{label}</caption>
        <thead>
          <tr>
            <th scope="col">{xLabel}</th>
            {curves.map((c) => (
              <th key={c.label} scope="col">
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {xs.map((x, i) => (
            <tr key={x}>
              <th scope="row">{formatX(x)}</th>
              {curves.map((c) => {
                const value = c.values[i];
                return <td key={c.label}>{value === null || value === undefined ? "—" : c.format(value)}</td>;
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}
