import { useEffect, useMemo } from "react";

import { EmptyState } from "./design/components";
import { messages, type Locale } from "./i18n";
import type { Toggle } from "./map/mapModel";
import type { BudgetsView, Density, DriftView, PerfMap, Span } from "./protocol";
import type { Pins } from "./map/layout";
import { MapView, type MapCommands } from "./map/MapView";
import { initialUi, type UiState } from "./uiState";
import { initialModel, type ViewModel } from "./viewModel";

export interface AppProps {
  locale: Locale;
  model?: ViewModel;
  ui?: UiState;
  onToggle?: (toggle: Toggle) => void;
  onCloseAll?: () => void;
  onOpenCode?: (file: string, span: Span | null) => void;
  /** The files the map draws, when they change. */
  onDisplayed?: (paths: string[]) => void;
  /** The boxes moved by hand, and where they go when moved again (ADR 0021). */
  pins?: Pins;
  onPins?: (pins: Pins) => void;
  /** How dense the map is drawn (Q32). */
  density?: Density;
  /** The map's « Mark as reviewed » button (ADR 0024). */
  onMarkReviewed?: () => void;
  /** The filter « only the changes » (ADR 0024). */
  onToggleChanges?: () => void;
  onToggleDebt?: () => void;
  onReveal?: (target: string, path: string[]) => void;
  /** The map compared with the whiteboard (ADR 0042): the toggle, and the drift the host gave. */
  onToggleIntent?: () => void;
  intent?: DriftView | null;
  onOpenWhiteboard?: () => void;
  /** The Performances layer (ADR 0051): the toggle, and what the host gave for it. */
  onTogglePerformance?: () => void;
  performance?: PerfMap | null;
  onOpenPerformance?: () => void;
  /** The whiteboard's budgets on the last runs, shown when exceeded (ADR 0052). */
  budgets?: BudgetsView | null;
  /** Where the palette's commands for the map come in. */
  commandsRef?: MapCommands;
  /** Called once, after the first render reached the DOM. */
  onReady?: () => void;
}

/** The map once the daemon has sent boxes; before, a teaching state for each situation. */
export function App({
  locale,
  model = initialModel,
  ui = initialUi,
  onToggle,
  onCloseAll,
  onOpenCode,
  onDisplayed,
  pins,
  onPins,
  density,
  onMarkReviewed,
  onToggleChanges,
  onToggleDebt,
  onReveal,
  onToggleIntent,
  intent,
  onOpenWhiteboard,
  onTogglePerformance,
  performance,
  onOpenPerformance,
  budgets,
  commandsRef,
  onReady,
}: AppProps) {
  useEffect(() => {
    onReady?.();
  }, [onReady]);
  const openRoles = useMemo(() => new Set(ui.openRoles), [ui.openRoles]);

  const t = messages[locale];
  const { daemon, view } = model;
  if (view && view.nodes.size > 0) {
    const status =
      daemon.state === "restarting" ? t.restartingTitle : daemon.state === "unavailable" ? t.unavailableTitle : null;
    return (
      <main className="tim-app">
        {status && (
          <p className="tim-app__status" role="status">
            {status}
          </p>
        )}
        <MapView
          view={view}
          locale={locale}
          openRoles={openRoles}
          focus={ui.focus}
          onToggle={(toggle) => onToggle?.(toggle)}
          onCloseAll={() => onCloseAll?.()}
          onOpenCode={onOpenCode}
          onDisplayed={onDisplayed}
          pins={pins}
          onPins={onPins}
          density={density}
          onMarkReviewed={onMarkReviewed}
          onlyChanges={ui.onlyChanges ?? false}
          onToggleChanges={onToggleChanges}
          showDebt={ui.debt ?? false}
          onToggleDebt={onToggleDebt}
          onReveal={onReveal}
          compareIntent={ui.intent ?? false}
          intent={intent}
          onToggleIntent={onToggleIntent}
          onOpenWhiteboard={onOpenWhiteboard}
          showPerformance={ui.performance ?? false}
          performance={performance}
          onTogglePerformance={onTogglePerformance}
          onOpenPerformance={onOpenPerformance}
          budgets={budgets}
          commandsRef={commandsRef}
        />
      </main>
    );
  }
  let title: string;
  let text: string;
  let detail: string | null = null;
  if (daemon.state === "unavailable") {
    [title, text, detail] = [t.unavailableTitle, t.unavailableText, daemon.detail];
  } else if (daemon.state === "restarting") {
    [title, text, detail] = [t.restartingTitle, t.restartingText, daemon.detail];
  } else if (!view) {
    [title, text] = [t.startingTitle, t.startingText];
  } else {
    [title, text] = [t.indexingTitle, t.indexingText];
  }
  return (
    <main className="tim-app">
      <EmptyState title={title} text={text} detail={detail} live />
    </main>
  );
}
