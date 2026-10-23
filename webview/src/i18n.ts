import type { SymbolKind } from "./design/icons";
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
  /** The first indexing, with its bar (T-115). */
  progressTitle: string;
  progressText: string;
  progressFinding: string;
  progressReading: (done: number, total: number) => string;
  progressWriting: string;
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
  /** A line of the map, for screen readers: its ends, its links and how many are confirmed. */
  edgeLabel: (source: string, target: string, links: number, confirmed: number) => string;
  edgePartial: string;
  /** The certainty layer (PLAN.md 11.5). */
  certaintyLayer: string;
  sureWords: (percent: string) => string;
  sureTip: (confirmed: number, of: number) => string;
  mapSure: (percent: string) => string;
  mapSureNone: string;
  kinds: Record<"repo" | "component" | "package" | "file" | "symbol", string>;
  /** What a symbol is (T-114), and its tooltip: the kind, then its signature. */
  symbolKinds: Record<SymbolKind, string>;
  symbolTip: (kind: string, signature: string | null) => string;
  /** The review diff (ADR 0024). */
  review: ReviewMessages;
  /** The map compared with the whiteboard (ADR 0042). */
  intent: IntentMessages;
}

export interface IntentMessages {
  /** The toolbar's toggle. */
  layer: string;
  summaryLabel: string;
  title: string;
  /** « 3 drawn components: 2 found, 1 missing » */
  counts: (drawn: number, found: number) => string;
  forbiddenCount: (count: number) => string;
  requiredCount: (count: number) => string;
  uncheckedCount: (count: number) => string;
  follows: string;
  missingHeading: string;
  forbiddenHeading: string;
  requiredHeading: string;
  /** « Front → Data: 2 dependencies (app → store, …) » */
  crossing: (edge: string, count: number, examples: string) => string;
  nothingTitle: string;
  nothingText: string;
  openWhiteboard: string;
  unreadTitle: string;
  unavailable: string;
  /** On a box: drawn components it stands for, and those inside it. */
  drawnBadge: (names: string) => string;
  withinBadge: (names: string) => string;
  /** On a line: the forbidden edges it crosses. */
  forbiddenLine: (edges: string) => string;
  forbiddenMark: string;
  legendTitle: string;
  legendForbidden: string;
  legendDrawn: string;
  legendWithin: string;
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
  /** The review debt (ADR 0030, Q44). */
  debtLayer: string;
  debtFiles: (count: number) => string;
  /** The same, short, in a title. */
  debtShort: (count: number) => string;
  untracked: string;
  debtLegendTitle: string;
  debtLevels: [string, string, string];
  untrackedLegend: string;
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
    progressTitle: "Indexing the project",
    progressText: "Timonier reads the code once, then follows each change. The map appears when it is done.",
    progressFinding: "Finding the source files…",
    progressReading: (done, total) => `Files read: ${done} of ${total}`,
    progressWriting: "All files read; writing the graph…",
    indexingTitle:
      "No Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, C#, Kotlin, Swift, PHP or Scala file here",
    indexingText:
      "Timonier reads Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, C#, Kotlin, Swift, PHP and Scala today. When this folder has some, the map appears as soon as they are indexed; other languages will come with their packs.",
    roles: {
      entry_point: "Entry point",
      data_model: "Data model",
      persistence: "Persistence",
      interface: "Interface",
      background_task: "Background task",
      external_libraries: "External libraries",
      utility: "Utility",
      project: "Project",
    },
    noRole: "No role yet",
    items: (count) => (count === 1 ? "1 item" : `${count} items`),
    edgeConfirmed: "Certain: a manifest, a language server or SCIP",
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
    edgeLabel: (source, target, links, confirmed) =>
      `${source} to ${target}: ${links === 1 ? "1 link" : `${links} links`}, ${
        confirmed >= links ? "certain" : confirmed === 0 ? "approximate" : `${confirmed} confirmed`
      }`,
    edgePartial: "Partly confirmed",
    certaintyLayer: "Certainty",
    sureWords: (percent) => `${percent} sure`,
    sureTip: (confirmed, of) =>
      `${confirmed} of ${of} ${of === 1 ? "link" : "links"} confirmed by a manifest, a language server or SCIP; the others seen by tree-sitter alone`,
    mapSure: (percent) => `${percent} of the links drawn are confirmed`,
    mapSureNone: "No link drawn yet",
    kinds: { repo: "Repository", component: "Component", package: "Package", file: "File", symbol: "Symbol" },
    symbolKinds: {
      module: "Module",
      type: "Type",
      trait: "Trait",
      function: "Function",
      method: "Method",
      field: "Field",
      constant: "Constant",
    },
    symbolTip: (kind, signature) => (signature ? `${kind}: ${signature}` : kind),
    intent: {
      layer: "Compare with the intent",
      summaryLabel: "The map compared with the whiteboard",
      title: "Compared with the whiteboard",
      counts: (drawn, found) =>
        `${drawn === 1 ? "1 drawn component" : `${drawn} drawn components`}, ${found} found in the code, ${drawn - found} missing`,
      forbiddenCount: (count) => (count === 1 ? "1 forbidden edge crossed" : `${count} forbidden edges crossed`),
      requiredCount: (count) => (count === 1 ? "1 required edge missing" : `${count} required edges missing`),
      uncheckedCount: (count) => (count === 1 ? "1 edge not checked" : `${count} edges not checked`),
      follows: "the code follows the whiteboard.",
      missingHeading: "Drawn, not in the code",
      forbiddenHeading: "Forbidden edges crossed",
      requiredHeading: "Required edges missing",
      crossing: (edge, count, examples) =>
        `${edge}: ${count === 1 ? "1 dependency" : `${count} dependencies`} (${examples})`,
      nothingTitle: "Nothing drawn yet",
      nothingText:
        "Draw the architecture you want on the whiteboard: the map then shows how the code stands against it.",
      openWhiteboard: "Open the whiteboard",
      unreadTitle: "The whiteboard does not read",
      unavailable: "The comparison is not available while the daemon is not running.",
      drawnBadge: (names) => `Drawn: ${names}`,
      withinBadge: (names) => `Holds drawn: ${names}`,
      forbiddenLine: (edges) => `Crosses the forbidden edge ${edges}.`,
      forbiddenMark: "forbidden",
      legendTitle: "Compared with the whiteboard",
      legendForbidden: "a link that crosses a forbidden edge",
      legendDrawn: "the box a drawn component stands for",
      legendWithin: "a box that holds one, deeper",
    },
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
      debtLayer: "Review debt",
      debtFiles: (count) => (count === 1 ? "1 file to read again" : `${count} files to read again`),
      debtShort: (count) => `${count} to read again`,
      untracked: "untracked",
      debtLegendTitle: "Review debt: files changed since the review",
      debtLevels: ["Less than a third of its files", "Up to two thirds", "More than two thirds"],
      untrackedLegend: "Changed with no session open",
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
    progressTitle: "Indexation du projet",
    progressText: "Timonier lit le code une fois, puis suit chaque changement. La carte apparaît quand il a fini.",
    progressFinding: "Recherche des fichiers source…",
    progressReading: (done, total) => `Fichiers lus : ${done} sur ${total}`,
    progressWriting: "Tous les fichiers sont lus ; écriture du graphe…",
    indexingTitle:
      "Aucun fichier Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, C#, Kotlin, Swift, PHP ou Scala ici",
    indexingText:
      "Timonier lit aujourd'hui le Rust, le TypeScript, le JavaScript, le Python, le Go, le Java, le C, le C++, le C#, le Kotlin, le Swift, le PHP et le Scala. Quand ce dossier en contient, la carte apparaît dès qu'ils sont indexés ; les autres langages viendront avec leurs packs.",
    roles: {
      entry_point: "Point d'entrée",
      data_model: "Modèle de données",
      persistence: "Persistance",
      interface: "Interface",
      background_task: "Tâche de fond",
      external_libraries: "Bibliothèques externes",
      utility: "Utilitaire",
      project: "Projet",
    },
    noRole: "Pas encore de rôle",
    items: (count) => (count === 1 ? "1 élément" : `${count} éléments`),
    edgeConfirmed: "Sûre : manifeste, serveur de langage ou SCIP",
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
    edgeLabel: (source, target, links, confirmed) =>
      `${source} vers ${target} : ${links === 1 ? "1 lien" : `${links} liens`}, ${
        confirmed >= links
          ? links === 1
            ? "sûr"
            : "sûrs"
          : confirmed === 0
            ? "approché"
            : `${confirmed} ${confirmed === 1 ? "confirmé" : "confirmés"}`
      }`,
    edgePartial: "En partie confirmée",
    certaintyLayer: "Certitude",
    sureWords: (percent) => `${percent} sûrs`,
    sureTip: (confirmed, of) =>
      `${confirmed} ${confirmed <= 1 ? "lien" : "liens"} sur ${of} ${confirmed <= 1 ? "confirmé" : "confirmés"} par un manifeste, un serveur de langage ou SCIP ; les autres vus par tree-sitter seul`,
    mapSure: (percent) => `${percent} des liens dessinés sont confirmés`,
    mapSureNone: "Aucun lien dessiné pour l'instant",
    kinds: { repo: "Dépôt", component: "Composant", package: "Paquet", file: "Fichier", symbol: "Symbole" },
    symbolKinds: {
      module: "Module",
      type: "Type",
      trait: "Trait",
      function: "Fonction",
      method: "Méthode",
      field: "Champ",
      constant: "Constante",
    },
    symbolTip: (kind, signature) => (signature ? `${kind} : ${signature}` : kind),
    intent: {
      layer: "Comparer à l'intention",
      summaryLabel: "La carte comparée au tableau blanc",
      title: "Comparée au tableau blanc",
      counts: (drawn, found) =>
        `${drawn <= 1 ? `${drawn} composant dessiné` : `${drawn} composants dessinés`}, ${found} ${found <= 1 ? "trouvé" : "trouvés"} dans le code, ${drawn - found} ${drawn - found <= 1 ? "manquant" : "manquants"}`,
      forbiddenCount: (count) =>
        count <= 1 ? `${count} arête interdite franchie` : `${count} arêtes interdites franchies`,
      requiredCount: (count) => (count <= 1 ? `${count} arête requise absente` : `${count} arêtes requises absentes`),
      uncheckedCount: (count) => (count <= 1 ? `${count} arête non vérifiée` : `${count} arêtes non vérifiées`),
      follows: "le code suit le tableau blanc.",
      missingHeading: "Dessinés, absents du code",
      forbiddenHeading: "Arêtes interdites franchies",
      requiredHeading: "Arêtes requises absentes",
      crossing: (edge, count, examples) =>
        `${edge} : ${count <= 1 ? `${count} dépendance` : `${count} dépendances`} (${examples})`,
      nothingTitle: "Rien de dessiné encore",
      nothingText: "Dessinez l'architecture voulue sur le tableau blanc : la carte montre alors où en est le code.",
      openWhiteboard: "Ouvrir le tableau blanc",
      unreadTitle: "Le tableau blanc ne se lit pas",
      unavailable: "La comparaison n'est pas disponible tant que le démon ne tourne pas.",
      drawnBadge: (names) => `Dessiné : ${names}`,
      withinBadge: (names) => `Contient du dessiné : ${names}`,
      forbiddenLine: (edges) => `Franchit l'arête interdite ${edges}.`,
      forbiddenMark: "interdite",
      legendTitle: "Comparée au tableau blanc",
      legendForbidden: "un lien qui franchit une arête interdite",
      legendDrawn: "la boîte que désigne un composant dessiné",
      legendWithin: "une boîte qui en contient un, plus profond",
    },
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
      debtLayer: "Dette de revue",
      debtFiles: (count) => (count === 1 ? "1 fichier à relire" : `${count} fichiers à relire`),
      debtShort: (count) => `${count} à relire`,
      untracked: "non suivi",
      debtLegendTitle: "Dette de revue : fichiers changés depuis la revue",
      debtLevels: ["Moins d'un tiers de ses fichiers", "Jusqu'aux deux tiers", "Plus des deux tiers"],
      untrackedLegend: "Changé sans session ouverte",
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
