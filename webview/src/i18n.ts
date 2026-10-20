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
  markReviewed: string;
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
  /** The review diff (ADR 0024). */
  review: ReviewMessages;
}

export interface ReviewMessages {
  /** The words of the badges. */
  added: string;
  modified: string;
  cycle: string;
  fanIn: (before: number, after: number) => string;
  fanOut: (before: number, after: number) => string;
  /** The same, in full: the details panel and screen readers. */
  addedLong: string;
  modifiedLong: string;
  cycleLong: string;
  fanInLong: (before: number, after: number) => string;
  fanOutLong: (before: number, after: number) => string;
  /** The count of a closed role's changed members, in words (« 2 modifiés »). */
  changedCount: (count: number) => string;
  /** The legend of the marks. */
  legendTitle: string;
  legendCycle: string;
  legendJump: string;
  /** On a line of the map. */
  linkAdded: string;
  linkCycle: string;
  /** Members of a role changed since the review. */
  changes: (count: number) => string;
  changedMembers: (names: string) => string;
  /** The filter that quiets what did not change (Q31). */
  onlyChanges: string;
  /** The summary. */
  summaryLabel: string;
  noReviewTitle: string;
  noReviewText: string;
  since: (date: string) => string;
  nothing: string;
  details: string;
  hideDetails: string;
  addedBoxes: (count: number) => string;
  modifiedBoxes: (count: number) => string;
  removedBoxes: (count: number) => string;
  dependenciesAdded: (count: number) => string;
  dependenciesRemoved: (count: number) => string;
  cycles: (count: number) => string;
  jumps: (count: number) => string;
  /** The headings of the lists. */
  addedHeading: string;
  modifiedHeading: string;
  removedHeading: string;
  dependenciesAddedHeading: string;
  dependenciesRemovedHeading: string;
  cyclesHeading: string;
  jumpsHeading: string;
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
    markReviewed: "Mark as reviewed",
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
    review: {
      added: "New",
      modified: "Modified",
      cycle: "Cycle",
      fanIn: (before, after) => `Fan-in ${before} → ${after}`,
      fanOut: (before, after) => `Fan-out ${before} → ${after}`,
      addedLong: "New since the last review",
      modifiedLong: "Modified since the last review",
      cycleLong: "In a cycle added since the last review",
      fanInLong: (before, after) => `${after} boxes depend on it, against ${before} at the review`,
      fanOutLong: (before, after) => `Depends on ${after} boxes, against ${before} at the review`,
      linkAdded: "New",
      linkCycle: "Cycle",
      changes: (count) => (count === 1 ? "1 change" : `${count} changes`),
      changedCount: (count) => `${count} modified`,
      legendTitle: "Changes since the review",
      legendCycle: "In a cycle added",
      legendJump: "Fan-in or fan-out that jumped",
      changedMembers: (names) => `Changed since the last review: ${names}.`,
      onlyChanges: "Highlight the changes",
      summaryLabel: "Since the last review",
      noReviewTitle: "No review yet",
      noReviewText: "Mark the map as reviewed: what changes afterwards shows here and on the map.",
      since: (date) => `Since your review of ${date}`,
      nothing: "Nothing has changed since.",
      details: "Details",
      hideDetails: "Hide the details",
      addedBoxes: (count) => (count === 1 ? "1 new box" : `${count} new boxes`),
      modifiedBoxes: (count) => (count === 1 ? "1 modified" : `${count} modified`),
      removedBoxes: (count) => (count === 1 ? "1 removed" : `${count} removed`),
      dependenciesAdded: (count) => (count === 1 ? "1 dependency added" : `${count} dependencies added`),
      dependenciesRemoved: (count) => (count === 1 ? "1 dependency removed" : `${count} dependencies removed`),
      cycles: (count) => (count === 1 ? "1 cycle added" : `${count} cycles added`),
      jumps: (count) => (count === 1 ? "1 jump of fan-in or fan-out" : `${count} jumps of fan-in or fan-out`),
      addedHeading: "New",
      modifiedHeading: "Modified",
      removedHeading: "Removed",
      dependenciesAddedHeading: "Dependencies added",
      dependenciesRemovedHeading: "Dependencies removed",
      cyclesHeading: "Cycles added",
      jumpsHeading: "Fan-in and fan-out that jumped",
    },
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
    markReviewed: "Marquer comme revu",
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
    review: {
      added: "Nouveau",
      modified: "Modifié",
      cycle: "Cycle",
      fanIn: (before, after) => `Fan-in ${before} → ${after}`,
      fanOut: (before, after) => `Fan-out ${before} → ${after}`,
      addedLong: "Nouveau depuis la dernière revue",
      modifiedLong: "Modifié depuis la dernière revue",
      cycleLong: "Dans un cycle ajouté depuis la dernière revue",
      fanInLong: (before, after) => `${after} boîtes en dépendent, contre ${before} à la revue`,
      fanOutLong: (before, after) => `Dépend de ${after} boîtes, contre ${before} à la revue`,
      linkAdded: "Nouveau",
      linkCycle: "Cycle",
      changes: (count) => (count === 1 ? "1 changement" : `${count} changements`),
      changedCount: (count) => (count === 1 ? "1 modifié" : `${count} modifiés`),
      legendTitle: "Changements depuis la revue",
      legendCycle: "Dans un cycle ajouté",
      legendJump: "Fan-in ou fan-out qui a bondi",
      changedMembers: (names) => `Changé depuis la dernière revue : ${names}.`,
      onlyChanges: "Mettre en avant les changements",
      summaryLabel: "Depuis la dernière revue",
      noReviewTitle: "Pas encore de revue",
      noReviewText: "Marquez la carte comme revue : ce qui change ensuite apparaîtra ici et sur la carte.",
      since: (date) => `Depuis votre revue du ${date}`,
      nothing: "Rien n'a changé depuis.",
      details: "Détails",
      hideDetails: "Masquer les détails",
      addedBoxes: (count) => (count === 1 ? "1 nouvelle boîte" : `${count} nouvelles boîtes`),
      modifiedBoxes: (count) => (count === 1 ? "1 modifiée" : `${count} modifiées`),
      removedBoxes: (count) => (count === 1 ? "1 supprimée" : `${count} supprimées`),
      dependenciesAdded: (count) => (count === 1 ? "1 dépendance ajoutée" : `${count} dépendances ajoutées`),
      dependenciesRemoved: (count) => (count === 1 ? "1 dépendance retirée" : `${count} dépendances retirées`),
      cycles: (count) => (count === 1 ? "1 cycle ajouté" : `${count} cycles ajoutés`),
      jumps: (count) => (count === 1 ? "1 bond de fan-in ou de fan-out" : `${count} bonds de fan-in ou de fan-out`),
      addedHeading: "Nouvelles",
      modifiedHeading: "Modifiées",
      removedHeading: "Supprimées",
      dependenciesAddedHeading: "Dépendances ajoutées",
      dependenciesRemovedHeading: "Dépendances retirées",
      cyclesHeading: "Cycles ajoutés",
      jumpsHeading: "Fan-in et fan-out qui ont bondi",
    },
  },
};

/** Picks the UI language from a BCP 47 tag such as VSCode's `env.language`. */
export function resolveLocale(tag: string | undefined): Locale {
  return tag?.toLowerCase().startsWith("fr") ? "fr" : "en";
}
