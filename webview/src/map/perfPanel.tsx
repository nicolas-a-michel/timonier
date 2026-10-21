/**
 * The map's Performances layer (PLAN.md 8.7, ADR 0051): a banner above the map, as the review's,
 * that names the run shown and where its time went, and opens the Performances panel.
 */
import type { Locale } from "../i18n";
import { performanceMessages } from "../performance/messages";
import { duration } from "../performance/model";
import type { PerfMap } from "../protocol";

export interface PerfPanelProps {
  /** `null`: the host has none to give (no daemon). */
  view: PerfMap | null;
  locale: Locale;
  onOpenPerformance?: () => void;
}

/** A node's last name, what the banner says of it: `server.rs`, `parse`, `A.draw`. */
const short = (id: string) => id.replace(/^.*(::|[/:#])/, "");

/** The nodes where the most time went, three of them. */
export function busiest(view: PerfMap, locale: Locale): string[] {
  return [...view.nodes]
    .sort((a, b) => b.selfNs - a.selfNs)
    .slice(0, 3)
    .map((n) => `${short(n.node)} ${duration(n.selfNs, locale)}`);
}

export function PerfPanel({ view, locale, onOpenPerformance }: PerfPanelProps) {
  const t = performanceMessages[locale];
  const open = onOpenPerformance && (
    <div className="tim-review__actions">
      <button type="button" className="tim-button" onClick={onOpenPerformance}>
        {t.openPanel}
      </button>
    </div>
  );
  if (!view || !view.run) {
    return (
      <section className="tim-review tim-review--perf" aria-label={t.layer}>
        <p className="tim-review__title">{t.layer}</p>
        <p className="tim-review__text">{view ? t.layerEmpty : t.noDaemonText}</p>
        {view && open}
      </section>
    );
  }
  const top = busiest(view, locale);
  return (
    <section className="tim-review tim-review--perf" aria-label={t.layer}>
      <p className="tim-review__text">
        <strong className="tim-review__title">{t.layerTitle(view.run)}</strong> {top.join(" · ")}
      </p>
      {open}
    </section>
  );
}
