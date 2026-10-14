import { useEffect } from "react";

import { messages, type Locale } from "./i18n";

export interface AppProps {
  locale: Locale;
  /** Called once, after the first render reached the DOM. */
  onReady?: () => void;
}

/** Phase 1: the webview only shows a teaching empty state. */
export function App({ locale, onReady }: AppProps) {
  useEffect(() => {
    onReady?.();
  }, [onReady]);

  const t = messages[locale];
  return (
    <main className="tim-empty" aria-labelledby="tim-empty-title">
      <h1 id="tim-empty-title" className="tim-empty__title">
        {t.emptyTitle}
      </h1>
      <p className="tim-empty__text">{t.emptyText}</p>
    </main>
  );
}
