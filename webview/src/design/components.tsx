/**
 * Components of the design system (ADR 0020). They use the tokens only (tokens.css, design.css)
 * and carry their accessible names: the map (PLAN.md 4.7) and the other views build on them.
 */
import { cloneElement, useId, useRef, useState, type KeyboardEvent, type ReactElement, type ReactNode } from "react";

import { messages, type Locale } from "../i18n";
import type { Role } from "../protocol";
import { RoleIcon, ToneIcon, type Tone } from "./icons";

export interface ComponentBoxProps {
  name: string;
  role: Role | null;
  /** What it can be expanded into. */
  items: number;
  locale: Locale;
  selected?: boolean;
  /** Out of the current focus: shown, but quieter. */
  dimmed?: boolean;
  badges?: ReactNode;
  /** Takes the focus and carries its name. Off where a container does it (a map node). */
  focusable?: boolean;
}

/** A box of the map: a component, a package, a file. */
export function ComponentBox({
  name,
  role,
  items,
  locale,
  selected,
  dimmed,
  badges,
  focusable = true,
}: ComponentBoxProps) {
  const t = messages[locale];
  const roleLabel = role ? t.roles[role] : t.noRole;
  const classes = ["tim-box", selected ? "tim-box--selected" : "", dimmed ? "tim-box--dimmed" : ""].filter(Boolean);
  return (
    <div
      className={classes.join(" ")}
      tabIndex={focusable ? 0 : undefined}
      role={focusable ? "group" : undefined}
      aria-label={focusable ? `${name}, ${roleLabel}, ${t.items(items)}` : undefined}
      aria-current={selected ? "true" : undefined}
      data-contrast="outline"
      data-role={role ?? undefined}
    >
      <div className="tim-box__header" data-contrast="icon">
        <RoleIcon role={role} />
        <span className="tim-box__name">{name}</span>
      </div>
      <div className="tim-box__body">
        <span className="tim-box__role">{roleLabel}</span>
        <div className="tim-box__meta">
          <span>{t.items(items)}</span>
          {badges}
        </div>
      </div>
    </div>
  );
}

export interface EdgeLineProps {
  /** An SVG path. */
  d: string;
  /** Known from tree-sitter only (spec « Précision affichée »). */
  approximate: boolean;
  /** Its accessible name: what it links, and how sure it is. */
  label: string;
}

/** An edge of the map: dashed when approximate, solid when confirmed. Goes inside an `<svg>`. */
export function EdgeLine({ d, approximate, label }: EdgeLineProps) {
  const className = approximate ? "tim-edge tim-edge--approximate" : "tim-edge tim-edge--confirmed";
  return (
    <path className={className} d={d} role="img" aria-label={label} data-contrast="stroke">
      <title>{label}</title>
    </path>
  );
}

/** What the two kinds of edges mean. */
export function EdgeLegend({ locale }: { locale: Locale }) {
  const t = messages[locale];
  const sample = (approximate: boolean, label: string) => (
    <li className="tim-legend__item">
      <svg className="tim-legend__sample" viewBox="0 0 40 8" width="40" height="8" aria-hidden="true">
        <EdgeLine d="M1 4H39" approximate={approximate} label={label} />
      </svg>
      <span>{label}</span>
    </li>
  );
  return (
    <figure className="tim-legend">
      <figcaption className="tim-legend__title">{t.edgeLegend}</figcaption>
      <ul className="tim-legend__list">
        {sample(false, t.edgeConfirmed)}
        {sample(true, t.edgeApproximate)}
        <li className="tim-legend__item">
          <svg className="tim-legend__sample" viewBox="0 0 40 10" width="40" height="10" aria-hidden="true">
            <circle className="tim-legend__pin" cx="4" cy="5" r="4" data-contrast="fill" />
            <EdgeLine d="M8 5H32" approximate={false} label={t.edgeDirection} />
            <path className="tim-legend__pin" d="M32 1L40 5L32 9z" data-contrast="fill" />
          </svg>
          <span>{t.edgeDirection}</span>
        </li>
      </ul>
    </figure>
  );
}

export interface BadgeProps {
  tone: Tone;
  /** Says the meaning in words: the icon only repeats it. */
  label: string;
}

/** A discreet summary next to a box or a title: a shape and words, never a colour alone. */
export function Badge({ tone, label }: BadgeProps) {
  return (
    <span className={`tim-badge tim-badge--${tone}`} data-contrast="icon">
      <ToneIcon tone={tone} />
      <span>{label}</span>
    </span>
  );
}

export interface EmptyStateProps {
  title: string;
  text: string;
  /** Words of a program (an error, a path), shown as written. */
  detail?: string | null;
  action?: { label: string; onSelect: () => void };
  /** Announced when it changes: a state that follows the daemon. */
  live?: boolean;
}

/** A teaching empty state: what is happening, and what to do next. */
export function EmptyState({ title, text, detail, action, live }: EmptyStateProps) {
  const id = useId();
  return (
    <section className="tim-empty" aria-labelledby={id}>
      <div className="tim-empty__status" role={live ? "status" : undefined} aria-live={live ? "polite" : undefined}>
        <h1 id={id} className="tim-empty__title">
          {title}
        </h1>
        <p className="tim-empty__text">{text}</p>
        {detail && <p className="tim-empty__detail">{detail}</p>}
      </div>
      {action && (
        <button type="button" className="tim-button" onClick={action.onSelect}>
          {action.label}
        </button>
      )}
    </section>
  );
}

export interface TooltipProps {
  text: string;
  /** The element it describes: it must take focus. */
  children: ReactElement<{ "aria-describedby"?: string }>;
  /** Shown from the start: for the demo page and its contrast test. */
  open?: boolean;
}

/** Detail on hover and on focus; Escape hides it (WCAG 1.4.13). */
export function Tooltip({ text, children, open: initiallyOpen = false }: TooltipProps) {
  const id = useId();
  const [open, setOpen] = useState(initiallyOpen);
  return (
    <span
      className="tim-tooltip-anchor"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(initiallyOpen)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(initiallyOpen)}
      onKeyDown={(event) => {
        if (event.key === "Escape") setOpen(false);
      }}
    >
      {cloneElement(children, { "aria-describedby": id })}
      <span role="tooltip" id={id} className="tim-tooltip" hidden={!open}>
        {text}
      </span>
    </span>
  );
}

export interface ToolbarItem {
  id: string;
  label: string;
  icon?: ReactNode;
  /** A toggle: on or off. */
  pressed?: boolean;
  onSelect?: () => void;
}

/** Where the focus goes for `key` in a toolbar of `count` items; `undefined`: nowhere new. */
export function rovingIndex(current: number, key: string, count: number): number | undefined {
  if (count === 0) return undefined;
  switch (key) {
    case "ArrowRight":
    case "ArrowDown":
      return (current + 1) % count;
    case "ArrowLeft":
    case "ArrowUp":
      return (current - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return undefined;
  }
}

/** A row of actions: one tab stop, then the arrow keys (WAI-ARIA toolbar pattern). */
export function Toolbar({ label, items }: { label: string; items: ToolbarItem[] }) {
  const [active, setActive] = useState(0);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const next = rovingIndex(active, event.key, items.length);
    if (next === undefined) return;
    event.preventDefault();
    setActive(next);
    buttons.current[next]?.focus();
  };
  return (
    <div className="tim-toolbar" role="toolbar" aria-label={label} onKeyDown={onKeyDown}>
      {items.map((item, index) => (
        <button
          key={item.id}
          ref={(button) => {
            buttons.current[index] = button;
          }}
          type="button"
          className="tim-toolbar__button"
          tabIndex={index === active ? 0 : -1}
          aria-pressed={item.pressed}
          aria-label={item.icon ? item.label : undefined}
          title={item.icon ? item.label : undefined}
          onClick={() => {
            setActive(index);
            item.onSelect?.();
          }}
        >
          {item.icon ?? item.label}
        </button>
      ))}
    </div>
  );
}
