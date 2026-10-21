import type { Locale } from "../i18n";
import type { TaskState } from "../protocol";

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
  },
};
