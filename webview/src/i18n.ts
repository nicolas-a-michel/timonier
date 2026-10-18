import type { Role } from "./protocol";

/** UI languages. English is the base; French follows VSCode's display language. */
export type Locale = "en" | "fr";

export interface Messages {
  startingTitle: string;
  startingText: string;
  restartingTitle: string;
  restartingText: string;
  unavailableTitle: string;
  unavailableText: string;
  indexingTitle: string;
  indexingText: string;
  /** Architectural roles (ADR 0017). */
  roles: Record<Role, string>;
  /** A box without a role yet. */
  noRole: string;
  items: (count: number) => string;
  edgeConfirmed: string;
  edgeApproximate: string;
  /** Which way an edge goes: out of a box on its right, into the other on its left. */
  edgeDirection: string;
  edgeLegend: string;
  mapLabel: string;
  members: (count: number) => string;
  moreMembers: (count: number) => string;
  fitView: string;
  zoomIn: string;
  zoomOut: string;
  closeAll: string;
  /** Forget the boxes moved by hand (ADR 0021). */
  resetLayout: string;
  mapTools: string;
  openHint: string;
  /** What every node says to screen readers, besides its own name: how to move on. */
  nodeHint: string;
  /** The buttons of the details panel that move a box (WCAG 2.5.7). */
  moveGroup: string;
  moveTo: Record<"left" | "right" | "up" | "down", string>;
  closeHint: string;
  codeHint: string;
  goToCode: string;
  /** A box folded to keep the map fluid (PLAN.md 4.10). */
  folded: (count: number) => string;
  foldedHint: string;
  /** An edge of the map, for screen readers. */
  edgeLabel: (source: string, target: string, links: number, approximate: boolean) => string;
  kinds: Record<"repo" | "component" | "package" | "file" | "symbol", string>;
}

export const messages: Record<Locale, Messages> = {
  en: {
    startingTitle: "Starting Timonier",
    startingText: "The daemon is indexing the project. The map appears in a moment.",
    restartingTitle: "Reconnecting",
    restartingText: "The daemon stopped. Timonier launches it again and the map comes back by itself.",
    unavailableTitle: "Timonier cannot start",
    unavailableText: "Run « Timonier: Show the Log » from the command palette for the details.",
    indexingTitle: "Nothing indexed yet",
    indexingText: "Timonier found no source file it reads in this folder yet.",
    roles: {
      entry_point: "Entry point",
      data_model: "Data model",
      persistence: "Persistence",
      interface: "Interface",
      background_task: "Background task",
      external_libraries: "External libraries",
      utility: "Utility",
    },
    noRole: "No role yet",
    items: (count) => (count === 1 ? "1 item" : `${count} items`),
    edgeConfirmed: "Certain: declared in a manifest, or confirmed by a language server",
    edgeApproximate: "Seen by tree-sitter only: approximate",
    edgeDirection: "Leaves a box on the right, enters the other on the left",
    edgeLegend: "Links: dependencies and calls",
    mapLabel: "Architecture map",
    members: (count) => (count === 1 ? "1 member" : `${count} members`),
    moreMembers: (count) => `and ${count} more`,
    fitView: "Fit to the window",
    zoomIn: "Zoom in",
    zoomOut: "Zoom out",
    closeAll: "Back to the roles",
    resetLayout: "Reset the layout",
    mapTools: "Map",
    openHint: "Enter opens it.",
    nodeHint: "Arrow keys go to the nearest box; Alt and an arrow move it.",
    moveGroup: "Move",
    moveTo: { left: "Move left", right: "Move right", up: "Move up", down: "Move down" },
    closeHint: "Enter closes it.",
    codeHint: "Enter opens its code; Shift+Enter, from any box.",
    goToCode: "Go to the code",
    folded: (count) => `${count} folded`,
    foldedHint: "Folded to keep the map fluid: Enter shows its contents.",
    edgeLabel: (source, target, links, approximate) =>
      `${source} to ${target}: ${links === 1 ? "1 link" : `${links} links`}, ${approximate ? "approximate" : "certain"}`,
    kinds: { repo: "Repository", component: "Component", package: "Package", file: "File", symbol: "Symbol" },
  },
  fr: {
    startingTitle: "Démarrage de Timonier",
    startingText: "Le démon indexe le projet. La carte s'affiche dans un instant.",
    restartingTitle: "Reconnexion",
    restartingText: "Le démon s'est arrêté. Timonier le relance, et la carte revient d'elle-même.",
    unavailableTitle: "Timonier ne peut pas démarrer",
    unavailableText: "Lancez « Timonier : Afficher le journal » depuis la palette de commandes pour les détails.",
    indexingTitle: "Rien d'indexé pour l'instant",
    indexingText: "Timonier n'a encore trouvé dans ce dossier aucun fichier source qu'il sait lire.",
    roles: {
      entry_point: "Point d'entrée",
      data_model: "Modèle de données",
      persistence: "Persistance",
      interface: "Interface",
      background_task: "Tâche de fond",
      external_libraries: "Bibliothèques externes",
      utility: "Utilitaire",
    },
    noRole: "Pas encore de rôle",
    items: (count) => (count === 1 ? "1 élément" : `${count} éléments`),
    edgeConfirmed: "Sûre : déclarée dans un manifeste, ou confirmée par un serveur de langage",
    edgeApproximate: "Vue par tree-sitter seulement : approchée",
    edgeDirection: "Sort d'une boîte à droite, entre dans l'autre à gauche",
    edgeLegend: "Liens : dépendances et appels",
    mapLabel: "Carte d'architecture",
    members: (count) => (count === 1 ? "1 membre" : `${count} membres`),
    moreMembers: (count) => `et ${count} autres`,
    fitView: "Ajuster à la fenêtre",
    zoomIn: "Zoom avant",
    zoomOut: "Zoom arrière",
    closeAll: "Revenir aux rôles",
    resetLayout: "Réinitialiser la disposition",
    mapTools: "Carte",
    openHint: "Entrée l'ouvre.",
    nodeHint: "Les flèches mènent à la boîte la plus proche ; Alt et une flèche la déplacent.",
    moveGroup: "Déplacer",
    moveTo: {
      left: "Déplacer à gauche",
      right: "Déplacer à droite",
      up: "Déplacer vers le haut",
      down: "Déplacer vers le bas",
    },
    closeHint: "Entrée le ferme.",
    codeHint: "Entrée ouvre son code ; Maj+Entrée, depuis toute boîte.",
    goToCode: "Aller au code",
    folded: (count) => `${count} repliés`,
    foldedHint: "Replié pour garder la carte fluide : Entrée montre son contenu.",
    edgeLabel: (source, target, links, approximate) =>
      `${source} vers ${target} : ${links === 1 ? "1 lien" : `${links} liens`}, ${approximate ? "approché" : "sûr"}`,
    kinds: { repo: "Dépôt", component: "Composant", package: "Paquet", file: "Fichier", symbol: "Symbole" },
  },
};

/** Picks the UI language from a BCP 47 tag such as VSCode's `env.language`. */
export function resolveLocale(tag: string | undefined): Locale {
  return tag?.toLowerCase().startsWith("fr") ? "fr" : "en";
}
