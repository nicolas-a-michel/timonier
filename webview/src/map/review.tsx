/**
 * The review diff on the map (ADR 0024): the badge a box shows, what it says in full, and the
 * summary of what changed since the last review. What is gone from the map is only in the
 * summary (Q26).
 */
import { useState } from "react";

import { Badge } from "../design/components";
import { ChangeIcon, CheckIcon, type ChangeKind } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { EdgeReview, NodeReview, ReviewBox, ReviewSummary } from "../protocol";

/** The one change a box shows on the map, the most telling first; the details say them all. */
export function primaryChange(review: NodeReview): ChangeKind | null {
  if (review.change === "added") return "added";
  if (review.cycle) return "cycle";
  if (review.fanIn) return "fanIn";
  if (review.fanOut) return "fanOut";
  if (review.change === "modified") return "modified";
  return null;
}

function badgeLabel(review: NodeReview, kind: ChangeKind, locale: Locale): string {
  const t = messages[locale].review;
  switch (kind) {
    case "added":
      return t.added;
    case "modified":
      return t.modified;
    case "cycle":
      return t.cycle;
    case "fanIn":
      return review.fanIn ? t.fanIn(review.fanIn.before, review.fanIn.after) : t.modified;
    case "fanOut":
      return review.fanOut ? t.fanOut(review.fanOut.before, review.fanOut.after) : t.modified;
  }
}

/** A box's change since the last review: a shape and a word, in the colour of attention. */
export function ReviewBadge({ review, locale }: { review: NodeReview; locale: Locale }) {
  const kind = primaryChange(review);
  if (!kind) return null;
  return <Badge tone="attention" icon={<ChangeIcon kind={kind} />} label={badgeLabel(review, kind, locale)} />;
}

/** The mark of a new line, or of one in a cycle added: halfway along it. */
export function LinkBadge({ review, locale }: { review: EdgeReview; locale: Locale }) {
  const t = messages[locale].review;
  const kind: ChangeKind = review.cycle ? "cycle" : "added";
  return <Badge tone="attention" icon={<ChangeIcon kind={kind} />} label={review.cycle ? t.linkCycle : t.linkAdded} />;
}

/** Everything that changed for a box, in full: the details panel and screen readers. */
export function reviewLines(review: NodeReview | null, locale: Locale): string[] {
  if (!review) return [];
  const t = messages[locale].review;
  const lines: string[] = [];
  if (review.change === "added") lines.push(t.addedLong);
  if (review.change === "modified") lines.push(t.modifiedLong);
  if (review.cycle) lines.push(t.cycleLong);
  if (review.fanIn) lines.push(t.fanInLong(review.fanIn.before, review.fanIn.after));
  if (review.fanOut) lines.push(t.fanOutLong(review.fanOut.before, review.fanOut.after));
  return lines;
}

/** The counts of the summary, in reading order; none when nothing changed. */
export function summaryCounts(summary: ReviewSummary, locale: Locale): string[] {
  const t = messages[locale].review;
  const counts: [number, (count: number) => string][] = [
    [summary.added.length, t.addedBoxes],
    [summary.modified.length, t.modifiedBoxes],
    [summary.removed.length, t.removedBoxes],
    [summary.dependenciesAdded.length, t.dependenciesAdded],
    [summary.dependenciesRemoved.length, t.dependenciesRemoved],
    [summary.cycles.length, t.cycles],
    [summary.fans.length, t.jumps],
  ];
  return counts.filter(([count]) => count > 0).map(([count, say]) => say(count));
}

/** When the review was marked, in the reader's language. */
export function reviewDate(timestampMs: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(timestampMs));
}

/** The lists of the summary, under their headings: the boxes by name. */
export function summarySections(summary: ReviewSummary, locale: Locale): { heading: string; items: string[] }[] {
  const t = messages[locale].review;
  // With the box that holds it: Timonier has more than one component named « tests ».
  const label = (b: ReviewBox) => (b.within ? `${b.name} (${b.within})` : b.name);
  const names = (boxes: ReviewBox[]) => boxes.map(label);
  const link = (d: { src: ReviewBox; dst: ReviewBox }) => `${label(d.src)} → ${label(d.dst)}`;
  return [
    { heading: t.addedHeading, items: names(summary.added) },
    { heading: t.modifiedHeading, items: names(summary.modified) },
    { heading: t.removedHeading, items: names(summary.removed) },
    { heading: t.dependenciesAddedHeading, items: summary.dependenciesAdded.map(link) },
    { heading: t.dependenciesRemovedHeading, items: summary.dependenciesRemoved.map(link) },
    { heading: t.cyclesHeading, items: summary.cycles.map((cycle) => names(cycle).join(", ")) },
    {
      heading: t.jumpsHeading,
      items: summary.fans.map((fan) =>
        [
          label(fan.item),
          fan.fanIn ? t.fanIn(fan.fanIn.before, fan.fanIn.after) : undefined,
          fan.fanOut ? t.fanOut(fan.fanOut.before, fan.fanOut.after) : undefined,
        ]
          .filter(Boolean)
          .join(" · "),
      ),
    },
  ].filter((section) => section.items.length > 0);
}

export interface ReviewSummaryProps {
  summary: ReviewSummary;
  locale: Locale;
  onMarkReviewed?: () => void;
  /** The filter « highlight the changes »: nothing to filter before the first review (Q27). */
  onlyChanges?: boolean;
  onToggleChanges?: () => void;
  /** The lists shown, when the map holds that state: its palette command opens them too. */
  detailsOpen?: boolean;
  onToggleDetails?: () => void;
}

/**
 * What changed since the last review: one line, the lists on demand. Before the first review,
 * what to do (Q27).
 */
export function ReviewSummaryPanel({
  summary,
  locale,
  onMarkReviewed,
  onlyChanges = false,
  onToggleChanges,
  detailsOpen,
  onToggleDetails,
}: ReviewSummaryProps) {
  const t = messages[locale].review;
  const [openHere, setOpenHere] = useState(false);
  const open = detailsOpen ?? openHere;
  const toggle = onToggleDetails ?? (() => setOpenHere(!openHere));
  if (!summary.since) {
    return (
      <section className="tim-review" aria-label={t.summaryLabel}>
        <p className="tim-review__title">{t.noReviewTitle}</p>
        <p className="tim-review__text">{t.noReviewText}</p>
        {onMarkReviewed && (
          <div className="tim-review__actions">
            <button type="button" className="tim-button tim-button--primary" onClick={onMarkReviewed}>
              {messages[locale].markReviewed}
            </button>
          </div>
        )}
      </section>
    );
  }
  const counts = summaryCounts(summary, locale);
  return (
    <section className={open ? "tim-review tim-review--open" : "tim-review"} aria-label={t.summaryLabel}>
      <div className="tim-review__summary">
        {counts.length > 0 && (
          <span className="tim-review__mark" data-contrast="icon">
            <ChangeIcon kind="modified" />
          </span>
        )}
        <p className="tim-review__text">
          <strong className="tim-review__title">{t.since(reviewDate(summary.since.timestampMs, locale))}</strong>{" "}
          {counts.length > 0 ? counts.join(" · ") : t.nothing}
        </p>
      </div>
      <div className="tim-review__actions">
        {counts.length > 0 && (
          <button type="button" className="tim-button" aria-expanded={open} onClick={toggle}>
            {open ? t.hideDetails : t.details}
          </button>
        )}
        {onToggleChanges && (
          <button
            type="button"
            className="tim-button tim-review__more"
            aria-pressed={onlyChanges}
            onClick={onToggleChanges}
          >
            {onlyChanges && <CheckIcon />}
            {t.onlyChanges}
          </button>
        )}
        {onMarkReviewed && (
          <button type="button" className="tim-button tim-button--primary tim-review__more" onClick={onMarkReviewed}>
            {messages[locale].markReviewed}
          </button>
        )}
      </div>
      {open && (
        // It scrolls when long: the keyboard reaches it too (WCAG 2.1.1).
        <div className="tim-review__lists" role="group" aria-label={t.details} tabIndex={0}>
          {summarySections(summary, locale).map((section) => (
            <section key={section.heading}>
              <h3 className="tim-review__heading">{section.heading}</h3>
              <ul className="tim-review__list">
                {section.items.map((item, index) => (
                  <li key={`${index}-${item}`}>{item}</li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}
    </section>
  );
}
