/** UI languages. English is the base; French follows VSCode's display language. */
export type Locale = "en" | "fr";

export interface Messages {
  emptyTitle: string;
  emptyText: string;
}

export const messages: Record<Locale, Messages> = {
  en: {
    emptyTitle: "No map yet",
    emptyText:
      "The architecture map will appear here once the project is indexed. This version only contains the extension skeleton; indexing comes in a later version.",
  },
  fr: {
    emptyTitle: "Pas encore de carte",
    emptyText:
      "La carte d'architecture s'affichera ici une fois le projet indexé. Cette version ne contient que le squelette de l'extension : l'indexation arrive dans une prochaine version.",
  },
};

/** Picks the UI language from a BCP 47 tag such as VSCode's `env.language`. */
export function resolveLocale(tag: string | undefined): Locale {
  return tag?.toLowerCase().startsWith("fr") ? "fr" : "en";
}
