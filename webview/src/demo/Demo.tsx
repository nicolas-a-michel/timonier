import { useEffect, useState } from "react";

import {
  Badge,
  ComponentBox,
  EdgeLegend,
  EdgeLine,
  EmptyState,
  ProgressBar,
  Toolbar,
  Tooltip,
} from "../design/components";
import { messages, type Locale } from "../i18n";
import type { SymbolKind } from "../design/icons";
import type { Role } from "../protocol";

/** One symbol of each kind, with a signature as the packs write them (T-114). */
const SYMBOLS: [SymbolKind, string, string | null][] = [
  ["function", "parse", "pub fn parse(text: &str) -> Result<Tree>"],
  ["method", "new", "pub fn new(a: u32) -> Self"],
  ["type", "Tree", "pub struct Tree"],
  ["trait", "Shape", "pub trait Shape: Clone"],
  ["field", "size", "pub size: u32"],
  ["constant", "LIMIT", "pub const LIMIT: usize ="],
  ["module", "inner", "mod inner"],
];

const ROLES: Role[] = [
  "entry_point",
  "interface",
  "persistence",
  "data_model",
  "background_task",
  "external_libraries",
  "utility",
];

export interface DemoProps {
  locale: Locale;
  theme: string | null;
  themes: string[];
  onRendered?: () => void;
}

/** Every component of the design system, in every state (ADR 0020). */
export function Demo({ locale, theme, themes, onRendered }: DemoProps) {
  const t = messages[locale];
  const [density, setDensity] = useState<"comfortable" | "compact">("comfortable");
  useEffect(() => {
    onRendered?.();
  }, [onRendered]);
  const link = (name: string | null, label: string) => {
    const params = new URLSearchParams();
    if (name) params.set("theme", name);
    if (locale !== "en") params.set("locale", locale);
    return (
      <a href={`?${params.toString()}`} aria-current={name === theme ? "page" : undefined}>
        {label}
      </a>
    );
  };
  return (
    <main className="tim-demo" data-density={density}>
      <h1 className="tim-demo__title">Timonier design system</h1>
      <nav className="tim-demo__themes" aria-label="Theme">
        {link(null, "fallbacks")}
        {themes.map((name) => (
          <span key={name}>{link(name, name)}</span>
        ))}
      </nav>

      <section className="tim-demo__section" aria-labelledby="demo-text">
        <h2 id="demo-text">Text</h2>
        <p>Foreground text, the default.</p>
        <p className="tim-demo__muted">Muted text, for what is secondary.</p>
        <p className="tim-demo__code">crates/timonier-daemon/src/server.rs</p>
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-toolbar">
        <h2 id="demo-toolbar">Toolbar</h2>
        <Toolbar
          label="Density"
          items={[
            {
              id: "comfortable",
              label: "Comfortable",
              pressed: density === "comfortable",
              onSelect: () => setDensity("comfortable"),
            },
            { id: "compact", label: "Compact", pressed: density === "compact", onSelect: () => setDensity("compact") },
          ]}
        />
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-boxes">
        <h2 id="demo-boxes">Component boxes</h2>
        <div className="tim-demo__grid">
          {ROLES.map((role, i) => (
            <ComponentBox key={role} name={`component-${i + 1}`} role={role} items={i * 3 + 1} locale={locale} />
          ))}
          <ComponentBox name="no-role-yet" role={null} items={0} locale={locale} />
          <ComponentBox
            name="selected"
            role="interface"
            items={4}
            locale={locale}
            selected
            badges={<Badge tone="attention" label="2 to review" />}
          />
          <ComponentBox name="dimmed" role="utility" items={2} locale={locale} dimmed />
        </div>
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-progress">
        <h2 id="demo-progress">Progress</h2>
        <ProgressBar progress={{ done: 120, total: 247 }} locale={locale} />
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-symbols">
        <h2 id="demo-symbols">Symbols</h2>
        <div className="tim-demo__grid">
          {SYMBOLS.map(([kind, name, signature]) => (
            <ComponentBox key={kind} name={name} role={null} items={0} locale={locale} symbol={{ kind, signature }} />
          ))}
        </div>
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-edges">
        <h2 id="demo-edges">Edges</h2>
        <svg
          className="tim-demo__edges"
          viewBox="0 0 320 90"
          width="320"
          height="90"
          role="group"
          aria-label="Edge samples"
        >
          <EdgeLine d="M10 15H310" approximate={false} label="a → b, certain" />
          <EdgeLine d="M10 45H310" approximate label="a → c, approximate" />
          <EdgeLine d="M10 75H310" approximate={false} label="a → d, certain, many calls" />
        </svg>
        <EdgeLegend locale={locale} />
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-badges">
        <h2 id="demo-badges">Badges</h2>
        <div className="tim-demo__row">
          <Badge tone="neutral" label="12 files" />
          <Badge tone="violation" label="Violation: depends on the store" />
          <Badge tone="attention" label="Attention: 3 changes to review" />
          <Badge tone="conformity" label="Conforms to the whiteboard" />
        </div>
      </section>

      <section className="tim-demo__section" aria-labelledby="demo-tooltip">
        <h2 id="demo-tooltip">Tooltip</h2>
        <Tooltip text="Opens the file at its first line" open>
          <button type="button" className="tim-button">
            Open
          </button>
        </Tooltip>
      </section>

      <section className="tim-demo__section tim-demo__empty" aria-labelledby="demo-empty">
        <h2 id="demo-empty">Empty state</h2>
        <EmptyState
          title={t.unavailableTitle}
          text={t.unavailableText}
          detail="The program timonier was not found."
          action={{ label: "Show the Log", onSelect: () => undefined }}
        />
      </section>
    </main>
  );
}
