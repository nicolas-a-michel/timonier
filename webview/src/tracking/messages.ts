import type { Locale } from "../i18n";
import type { TaskState, TokenUsage } from "../protocol";

/** The words of the plan, todo and journal panels (PLAN.md 6.6, ADR 0031). */
export interface TrackingMessages {
  startingTitle: string;
  startingText: string;
  restartingTitle: string;
  restartingText: string;
  unavailableTitle: string;
  /** A file of `.timonier/` that does not read. */
  problem: (file: string) => string;
  problemText: string;
  planLabel: string;
  noPlanTitle: string;
  noPlanText: string;
  /** « 2 of 5 tasks done ». */
  progress: (done: number, of: number) => string;
  /** The progress bar's name: the milestone, then its progress. */
  progressLabel: (title: string, done: number, of: number) => string;
  emptyMilestone: string;
  states: Record<TaskState, string>;
  criterion: string;
  notes: string;
  todoLabel: string;
  /** Above the objective, in the Todo panel: whose todo it is. */
  currentSession: (objective: string) => string;
  /** The open session's todo is in its own panel. */
  seeTodo: string;
  noSessionTitle: string;
  noSessionText: string;
  emptyTodo: string;
  todoProgress: (done: number, of: number) => string;
  itemDone: string;
  itemOpen: string;
  sessionsLabel: string;
  noSessionsTitle: string;
  noSessionsText: string;
  /** Only the latest sessions come. */
  latest: (shown: number, total: number) => string;
  open: string;
  /** The open session's diff follows the code. */
  live: string;
  by: (agent: string) => string;
  files: (count: number) => string;
  summary: string;
  tasks: string;
  noTasks: string;
  components: string;
  noChanges: string;
  /** A session that ended before its diff was kept (ADR 0031). */
  unknownChanges: string;
  added: string;
  modified: string;
  removed: string;
  dependencies: string;
  linked: string;
  unlinked: string;
  todo: string;
  tests: string;
  /** The Tests part of a session: no budget exceeded at its end, or checked when it ends. */
  noTests: string;
  testsAtEnd: string;
  /** The AI cost of a session or a task (ADR 0058): tokens first, the dollars an indication. */
  aiCost: string;
  tokens: (usage: TokenUsage) => string;
  notMeasured: string;
  /** A task whose sessions served other tasks too. */
  sharedTokens: string;
  /** The bridge (ADR 0032). */
  bridgeLabel: string;
  planHeading: string;
  /** Tasks in progress and blocked, of the current milestone. */
  underway: (inProgress: number, blocked: number) => string;
  noOpenSession: string;
  sinceReview: (date: string) => string;
  noReviewHeading: string;
  noReviewText: string;
  nothingChanged: string;
  newBoxes: (count: number) => string;
  modifiedBoxes: (count: number) => string;
  removedBoxes: (count: number) => string;
  newDependencies: (count: number) => string;
  goneDependencies: (count: number) => string;
  sessionsSince: (count: number) => string;
  toRead: (files: number, untracked: number) => string;
  openMap: string;
  openPerformance: string;
  openWhiteboard: string;
  budgetExceeded: string;
  markReviewed: string;
  lookHeading: string;
  lookEmpty: string;
  lookProblem: (file: string) => string;
  lookBlocked: (task: string, title: string) => string;
  lookCycle: (names: string) => string;
  lookDebt: (files: number) => string;
  untracked: string;
  /** What a click on a box does. */
  revealHint: string;
  /** The project is not set up for the agents (Q52). */
  notConfiguredTitle: string;
  notConfiguredText: string;
  configure: string;
  /** The development process's settings (ADR 0056). */
  workflowTitle: string;
  workflowLoading: string;
  workflowPreset: string;
  presets: Record<"solo_fast" | "solo_disciplined" | "team", { name: string; text: string }> & {
    custom: { text: string };
  };
  workflowSettings: string;
  ticketsLabel: string;
  none: string;
  branchPerTask: string;
  branchPattern: string;
  commitConvention: string;
  free: string;
  referenceTicket: string;
  prAtEnd: string;
  prDraft: string;
  humanApproval: string;
  autoMerge: string;
  strategy: string;
  ciLabel: string;
  workflowRefused: (problem: string) => string;
  workflowAgent: string;
  /** The agents to set up (T-118). */
  agentsLegend: string;
  agentNotFound: string;
  configuring: string;
  configureDone: string;
  configureFailed: (detail: string | null) => string;
}

/** « 1.2M tokens in · 340 out · 45K from the cache · 12 requests · ≈ $0.04 (the agent's estimate) ». */
export function tokenWords(usage: TokenUsage, locale: Locale): string {
  const n = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 });
  const dollars = new Intl.NumberFormat(locale, { style: "currency", currency: "USD", maximumFractionDigits: 2 });
  const cache = usage.cache_read + usage.cache_creation;
  const cost = dollars.format(usage.cost_usd_micros / 1e6);
  return locale === "fr"
    ? `${n.format(usage.input)} tokens en entrée · ${n.format(usage.output)} en sortie · ${n.format(cache)} du cache · ` +
        `${usage.requests} requête${usage.requests > 1 ? "s" : ""} · ≈ ${cost} (estimation de l'agent)`
    : `${n.format(usage.input)} tokens in · ${n.format(usage.output)} out · ${n.format(cache)} from the cache · ` +
        `${usage.requests} request${usage.requests > 1 ? "s" : ""} · ≈ ${cost} (the agent's estimate)`;
}

export const trackingMessages: Record<Locale, TrackingMessages> = {
  en: {
    startingTitle: "Starting Timonier",
    startingText: "The plan, the todo and the sessions appear once the daemon answers.",
    restartingTitle: "Reconnecting",
    restartingText: "The daemon stopped. Timonier launches it again and this panel comes back by itself.",
    unavailableTitle: "Timonier cannot start",
    problem: (file) => `.timonier/${file} cannot be read`,
    problemText: "Fix it, or restore it with git: until then the agent's writes to it are refused.",
    planLabel: "Plan",
    noPlanTitle: "No plan yet",
    noPlanText:
      "The agent adds milestones and tasks with the plan_add_task tool, once it has opened a session. They appear here as it goes.",
    progress: (done, of) => `${done} of ${of} done`,
    progressLabel: (title, done, of) => `${title}: ${done} of ${of} tasks done`,
    emptyMilestone: "No task in this milestone yet.",
    states: {
      todo: "to do",
      inProgress: "in progress",
      blocked: "blocked",
      done: "done",
      dropped: "dropped",
    },
    criterion: "Done when",
    notes: "Notes",
    todoLabel: "Todo of the session",
    currentSession: (objective) => `Session in progress: ${objective}`,
    seeTodo: "the items are under Todo",
    noSessionTitle: "No session open",
    noSessionText:
      "The agent opens one with the session_start tool; its todo fills here as it splits the work, and empties as it goes.",
    emptyTodo: "The agent has not split the work yet.",
    todoProgress: (done, of) => `${done} of ${of} done`,
    itemDone: "done",
    itemOpen: "to do",
    sessionsLabel: "Sessions",
    noSessionsTitle: "No session yet",
    noSessionsText:
      "Each session of an agent lands here: its objective, its tasks, the components it touched and what it changed.",
    latest: (shown, total) => `The latest ${shown} of ${total} sessions.`,
    open: "in progress",
    live: "Since its start, live",
    by: (agent) => `by ${agent}`,
    files: (count) => (count === 1 ? "1 file" : `${count} files`),
    summary: "Summary",
    tasks: "Tasks",
    noTasks: "No task of the plan.",
    components: "Components touched",
    noChanges: "No change to the code.",
    unknownChanges: "Not recorded for this session.",
    added: "new",
    modified: "modified",
    removed: "removed",
    dependencies: "Dependencies",
    linked: "added",
    unlinked: "removed",
    todo: "Todo",
    tests: "Tests",
    noTests: "No budget of the whiteboard exceeded when it ended.",
    testsAtEnd: "The budgets of the whiteboard are checked on the last runs when it ends.",
    aiCost: "AI cost",
    tokens: (usage) => tokenWords(usage, "en"),
    notMeasured: "Not measured: no token count was received from the agent.",
    sharedTokens: "counted with the other tasks of its sessions",
    bridgeLabel: "Where things stand",
    planHeading: "Plan",
    underway: (inProgress, blocked) =>
      [inProgress > 0 ? `${inProgress} in progress` : "", blocked > 0 ? `${blocked} blocked` : ""]
        .filter(Boolean)
        .join(" · "),
    noOpenSession: "No agent session open.",
    sinceReview: (date) => `Since your review of ${date}`,
    noReviewHeading: "No review yet",
    noReviewText: "Mark the map as reviewed: what changes next shows here.",
    nothingChanged: "Nothing changed since.",
    newBoxes: (count) => (count === 1 ? "1 new box" : `${count} new boxes`),
    modifiedBoxes: (count) => `${count} modified`,
    removedBoxes: (count) => `${count} removed`,
    newDependencies: (count) => (count === 1 ? "1 dependency added" : `${count} dependencies added`),
    goneDependencies: (count) => (count === 1 ? "1 dependency removed" : `${count} dependencies removed`),
    sessionsSince: (count) => (count === 1 ? "1 agent session" : `${count} agent sessions`),
    toRead: (files, untracked) =>
      `${files === 1 ? "1 file" : `${files} files`} to read again${untracked > 0 ? `, ${untracked} untracked` : ""}`,
    openMap: "Open Map",
    openPerformance: "Performances",
    openWhiteboard: "Whiteboard",
    budgetExceeded: "Budget exceeded",
    markReviewed: "Mark as reviewed",
    lookHeading: "Where to look",
    lookEmpty: "Nothing to read again.",
    lookProblem: (file) => `.timonier/${file} cannot be read`,
    lookBlocked: (task, title) => `Blocked: ${task} ${title}`,
    lookCycle: (names) => `Cycle added: ${names}`,
    lookDebt: (files) => (files === 1 ? "1 file to read again" : `${files} files to read again`),
    untracked: "untracked",
    revealHint: "Shows it on the map",
    notConfiguredTitle: "This project is not set up for your agents",
    notConfiguredText:
      "Timonier writes their instructions, declares its MCP server and their hooks, and opens the plan. A terminal shows each step, and asks what it cannot find out.",
    configure: "Set up",
    workflowTitle: "Development process",
    workflowLoading: "The process appears once the daemon answers.",
    workflowPreset: "Preset",
    presets: {
      solo_fast: { name: "Solo, fast", text: "commits straight on the main branch: a prototype, an exploration" },
      solo_disciplined: {
        name: "Solo, disciplined",
        text: "a branch and a pull request per task, merged when the tests, budgets and rules pass",
      },
      team: { name: "Team", text: "ticket, branch, pull request, a person's review, then merge; the CI blocks" },
      custom: { text: "Custom: one setting differs from the presets." },
    },
    workflowSettings: "Settings",
    ticketsLabel: "Tickets",
    none: "None",
    branchPerTask: "A branch per task",
    branchPattern: "Branch name",
    commitConvention: "Commit messages",
    free: "Free",
    referenceTicket: "The ticket in each commit",
    prAtEnd: "A pull request at the end of each session",
    prDraft: "As a draft",
    humanApproval: "A person's approval before merging",
    autoMerge: "GitHub merges once the checks pass",
    strategy: "Merge strategy",
    ciLabel: "CI",
    workflowRefused: (problem) => `Not saved: ${problem}.`,
    workflowAgent: "What the agents do",
    agentsLegend: "Agents to set up",
    agentNotFound: "not found here",
    configuring: "Setting up…",
    configureDone: "Set up: the agents read Timonier's instructions at their next session.",
    configureFailed: (detail) =>
      `The setup failed${detail ? `: ${detail}` : ""}. « Timonier: Show the Log » says more.`,
  },
  fr: {
    startingTitle: "Démarrage de Timonier",
    startingText: "Le plan, la todo et les sessions apparaissent dès que le démon répond.",
    restartingTitle: "Reconnexion",
    restartingText: "Le démon s'est arrêté. Timonier le relance et ce panneau revient tout seul.",
    unavailableTitle: "Timonier ne peut pas démarrer",
    problem: (file) => `.timonier/${file} ne se lit pas`,
    problemText: "Corrigez-le, ou restaurez-le avec git : d'ici là, les écritures de l'agent y sont refusées.",
    planLabel: "Plan",
    noPlanTitle: "Pas encore de plan",
    noPlanText:
      "L'agent ajoute jalons et tâches avec l'outil plan_add_task, une fois sa session ouverte. Ils apparaissent ici au fil de l'eau.",
    progress: (done, of) => `${done} sur ${of} faites`,
    progressLabel: (title, done, of) => `${title} : ${done} tâches faites sur ${of}`,
    emptyMilestone: "Pas encore de tâche dans ce jalon.",
    states: {
      todo: "à faire",
      inProgress: "en cours",
      blocked: "bloquée",
      done: "faite",
      dropped: "abandonnée",
    },
    criterion: "Faite quand",
    notes: "Notes",
    todoLabel: "Todo de la session",
    currentSession: (objective) => `Session en cours : ${objective}`,
    seeTodo: "le détail est sous Todo",
    noSessionTitle: "Aucune session ouverte",
    noSessionText:
      "L'agent en ouvre une avec l'outil session_start ; sa todo se remplit ici quand il découpe le travail, et se vide au fil de l'exécution.",
    emptyTodo: "L'agent n'a pas encore découpé le travail.",
    todoProgress: (done, of) => `${done} sur ${of} faits`,
    itemDone: "fait",
    itemOpen: "à faire",
    sessionsLabel: "Sessions",
    noSessionsTitle: "Pas encore de session",
    noSessionsText:
      "Chaque session d'un agent arrive ici : son objectif, ses tâches, les composants touchés et ce qu'elle a changé.",
    latest: (shown, total) => `Les ${shown} dernières sessions sur ${total}.`,
    open: "en cours",
    live: "Depuis son début, en direct",
    by: (agent) => `par ${agent}`,
    files: (count) => (count === 1 ? "1 fichier" : `${count} fichiers`),
    summary: "Résumé",
    tasks: "Tâches",
    noTasks: "Aucune tâche du plan.",
    components: "Composants touchés",
    noChanges: "Aucun changement du code.",
    unknownChanges: "Non enregistré pour cette session.",
    added: "nouveau",
    modified: "modifié",
    removed: "supprimé",
    dependencies: "Dépendances",
    linked: "ajoutée",
    unlinked: "retirée",
    todo: "Todo",
    tests: "Tests",
    noTests: "Aucun budget du tableau blanc dépassé à sa fin.",
    testsAtEnd: "Les budgets du tableau blanc sont vérifiés sur les derniers runs à sa fin.",
    aiCost: "Coût de l'IA",
    tokens: (usage) => tokenWords(usage, "fr"),
    notMeasured: "Non mesuré : aucun compte de tokens n'est venu de l'agent.",
    sharedTokens: "comptés aussi pour les autres tâches de ses sessions",
    bridgeLabel: "Où en sont les choses",
    planHeading: "Plan",
    underway: (inProgress, blocked) =>
      [inProgress > 0 ? `${inProgress} en cours` : "", blocked > 0 ? `${blocked} bloquée${blocked > 1 ? "s" : ""}` : ""]
        .filter(Boolean)
        .join(" · "),
    noOpenSession: "Aucune session d'agent ouverte.",
    sinceReview: (date) => `Depuis votre revue du ${date}`,
    noReviewHeading: "Pas encore de revue",
    noReviewText: "Marquez la carte comme revue : ce qui change ensuite apparaîtra ici.",
    nothingChanged: "Rien n'a changé depuis.",
    newBoxes: (count) => (count === 1 ? "1 nouvelle boîte" : `${count} nouvelles boîtes`),
    modifiedBoxes: (count) => (count === 1 ? "1 modifiée" : `${count} modifiées`),
    removedBoxes: (count) => (count === 1 ? "1 supprimée" : `${count} supprimées`),
    newDependencies: (count) => (count === 1 ? "1 dépendance ajoutée" : `${count} dépendances ajoutées`),
    goneDependencies: (count) => (count === 1 ? "1 dépendance retirée" : `${count} dépendances retirées`),
    sessionsSince: (count) => (count === 1 ? "1 session d'agent" : `${count} sessions d'agent`),
    toRead: (files, untracked) =>
      `${files === 1 ? "1 fichier" : `${files} fichiers`} à relire${untracked > 0 ? `, dont ${untracked} non suivi${untracked > 1 ? "s" : ""}` : ""}`,
    openMap: "Ouvrir la carte",
    openPerformance: "Performances",
    openWhiteboard: "Tableau blanc",
    budgetExceeded: "Budget dépassé",
    markReviewed: "Marquer comme revu",
    lookHeading: "Où regarder",
    lookEmpty: "Rien à relire.",
    lookProblem: (file) => `.timonier/${file} ne se lit pas`,
    lookBlocked: (task, title) => `Bloquée : ${task} ${title}`,
    lookCycle: (names) => `Cycle ajouté : ${names}`,
    lookDebt: (files) => (files === 1 ? "1 fichier à relire" : `${files} fichiers à relire`),
    untracked: "non suivi",
    revealHint: "La montre sur la carte",
    notConfiguredTitle: "Ce projet n'est pas configuré pour vos agents",
    notConfiguredText:
      "Timonier écrit leurs instructions, déclare son serveur MCP et leurs hooks, et ouvre le plan. Un terminal montre chaque étape, et demande ce qu'il ne peut pas trouver seul.",
    configure: "Configurer",
    workflowTitle: "Processus de développement",
    workflowLoading: "Le processus apparaît dès que le démon répond.",
    workflowPreset: "Préréglage",
    presets: {
      solo_fast: { name: "Solo rapide", text: "commits directs sur la branche principale : prototype, exploration" },
      solo_disciplined: {
        name: "Solo discipliné",
        text: "une branche et une pull request par tâche, fusionnée si tests, budgets et règles passent",
      },
      team: { name: "Équipe", text: "ticket, branche, pull request, revue humaine, puis fusion ; la CI bloque" },
      custom: { text: "Personnalisé : un réglage diffère des préréglages." },
    },
    workflowSettings: "Réglages",
    ticketsLabel: "Tickets",
    none: "Aucun",
    branchPerTask: "Une branche par tâche",
    branchPattern: "Nom de branche",
    commitConvention: "Messages de commit",
    free: "Libres",
    referenceTicket: "Le ticket dans chaque commit",
    prAtEnd: "Une pull request à la fin de chaque session",
    prDraft: "En brouillon",
    humanApproval: "Une approbation humaine avant la fusion",
    autoMerge: "GitHub fusionne quand les contrôles passent",
    strategy: "Stratégie de fusion",
    ciLabel: "CI",
    workflowRefused: (problem) => `Non enregistré : ${problem}.`,
    workflowAgent: "Ce que font les agents",
    agentsLegend: "Agents à configurer",
    agentNotFound: "non trouvé ici",
    configuring: "Configuration…",
    configureDone: "Configuré : les agents lisent les consignes de Timonier à leur prochaine session.",
    configureFailed: (detail) =>
      `La configuration a échoué${detail ? ` : ${detail}` : ""}. « Timonier : Afficher le journal » en dit plus.`,
  },
};
