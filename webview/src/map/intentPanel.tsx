/**
 * The map compared with the whiteboard (PLAN.md 7.6, ADR 0042): a banner above the map, as the
 * review's, that says how the code stands against what was drawn, and lists what is missing,
 * the forbidden edges crossed and the required ones absent. The tone says it with a shape and
 * words, never a colour alone.
 */
import { ToneIcon, type Tone } from "../design/icons";
import { messages, type Locale } from "../i18n";
import type { DriftView } from "../protocol";
import { drawnName } from "./intent";

export interface IntentPanelProps {
  /** `null`: the host has none to give (no daemon). */
  view: DriftView | null;
  locale: Locale;
  onOpenWhiteboard?: () => void;
}

/** The real boxes a crossing goes through, said short: their last name. */
const short = (id: string) => id.replace(/^.*[/:]/, "");

/** What the banner lists, by section; empty sections are left out. */
export function intentSections(view: DriftView, locale: Locale): { heading: string; items: string[] }[] {
  const t = messages[locale].intent;
  const { drift } = view;
  const name = (id: string) => drawnName(view, id);
  return [
    { heading: t.missingHeading, items: drift.missing.map(name) },
    {
      heading: t.forbiddenHeading,
      items: drift.forbidden.map((c) =>
        t.crossing(
          `${name(c.src)} → ${name(c.dst)}`,
          c.dependencies.length,
          c.dependencies
            .slice(0, 3)
            .map((d) => `${short(d.from)} → ${short(d.to)}`)
            .join(", ") + (c.dependencies.length > 3 ? ", …" : ""),
        ),
      ),
    },
    { heading: t.requiredHeading, items: drift.required.map((c) => `${name(c.src)} → ${name(c.dst)}`) },
  ].filter((section) => section.items.length > 0);
}

/** The banner's line, and its tone. */
export function intentSummary(view: DriftView, locale: Locale): { tone: Tone; text: string } {
  const t = messages[locale].intent;
  const { drift } = view;
  const found = drift.anchors.length - drift.missing.length;
  const parts = [t.counts(drift.anchors.length, found)];
  if (drift.forbidden.length > 0) parts.push(t.forbiddenCount(drift.forbidden.length));
  if (drift.required.length > 0) parts.push(t.requiredCount(drift.required.length));
  if (drift.unchecked.length > 0) parts.push(t.uncheckedCount(drift.unchecked.length));
  const broken = drift.forbidden.length > 0 || drift.required.length > 0;
  const tone: Tone = broken ? "violation" : drift.missing.length > 0 ? "attention" : "conformity";
  return { tone, text: tone === "conformity" ? `${parts.join(" · ")} · ${t.follows}` : parts.join(" · ") };
}

export function IntentPanel({ view, locale, onOpenWhiteboard }: IntentPanelProps) {
  const t = messages[locale].intent;
  if (!view || !view.drawn) {
    const title = !view ? t.layer : view.problem ? t.unreadTitle : t.nothingTitle;
    const text = !view ? t.unavailable : (view.problem ?? t.nothingText);
    return (
      <section className="tim-review tim-review--intent" aria-label={t.summaryLabel}>
        <p className="tim-review__title">{title}</p>
        <p className="tim-review__text">{text}</p>
        {view && onOpenWhiteboard && (
          <div className="tim-review__actions">
            <button type="button" className="tim-button" onClick={onOpenWhiteboard}>
              {t.openWhiteboard}
            </button>
          </div>
        )}
      </section>
    );
  }
  const { tone, text } = intentSummary(view, locale);
  const sections = intentSections(view, locale);
  return (
    <section className="tim-review tim-review--intent tim-review--open" aria-label={t.summaryLabel}>
      <div className="tim-review__summary">
        <span className="tim-review__mark" data-tone={tone} data-contrast="icon">
          <ToneIcon tone={tone} />
        </span>
        <p className="tim-review__text">
          <strong className="tim-review__title">{t.title}</strong> {text}
        </p>
      </div>
      {onOpenWhiteboard && (
        <div className="tim-review__actions">
          <button type="button" className="tim-button" onClick={onOpenWhiteboard}>
            {t.openWhiteboard}
          </button>
        </div>
      )}
      {sections.length > 0 && (
        // It scrolls when long: the keyboard reaches it too (WCAG 2.1.1).
        <div className="tim-review__lists" role="group" aria-label={t.summaryLabel} tabIndex={0}>
          {sections.map((section) => (
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
