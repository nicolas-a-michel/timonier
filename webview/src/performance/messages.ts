import type { Locale } from "../i18n";
import type { CpuSource, LimitKind, RegressionNote, TestStatus } from "../protocol";

/** A limit's figure, with its unit: « 7.2 ms », « 52 MB », « 340 ». */
function limitWords(locale: Locale, kind: LimitKind, measured: number, limit: number): string {
  const number = (v: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(v);
  const unit =
    kind === "p50_ms" || kind === "p95_ms" ? " ms" : kind === "memory_mb" ? (locale === "fr" ? " Mo" : " MB") : "";
  const what = {
    p50_ms: "p50",
    p95_ms: "p95",
    memory_mb: locale === "fr" ? "mémoire" : "memory",
    allocations: "allocations",
    calls: locale === "fr" ? "appels" : "calls",
  }[kind];
  return `${what} ${number(measured)}${unit} > ${number(limit)}${unit}`;
}

/** The words of the Performances panel and of the map's layer (ADR 0051). */
export interface PerformanceMessages {
  title: string;
  run: string;
  /** « R-12 · main 0123456 · 3 min ago ». */
  runOption: (run: string, where: string | null, ago: string) => string;
  running: string;
  withoutTraces: string;
  /** A smoke test of `timonier doctor` (relecture n° 3, I-2). */
  smoke: string;
  /** How long ago, from milliseconds. */
  ago: (ms: number) => string;
  wall: string;
  cpu: (source: CpuSource | null) => string;
  instructions: (billions: string) => string;
  memory: (whole: boolean) => string;
  tests: (count: number, failed: number) => string;
  spans: (count: number) => string;
  tabs: { timeline: string; tree: string; evolution: string };
  testList: string;
  status: Record<TestStatus, string>;
  /** « parses, passed, 4.2 ms ». */
  testLabel: (name: string, status: string, duration: string) => string;
  undated: string;
  noSpans: string;
  spansOf: (test: string) => string;
  /** On a bar of the timeline. */
  barTitle: (name: string, duration: string, node: string | null, failed: boolean) => string;
  spanError: string;
  resources: string;
  noResources: string;
  /** The time axis of the resources, from the run's start. */
  time: string;
  memoryAxis: string;
  cpuAxis: string;
  columns: { name: string; total: string; self: string; calls: string; memory: string; cpu: string };
  callTree: string;
  /** Under the call tree: what « resources » mean at a level. */
  resourcesNote: string;
  evolution: string;
  evolutionNote: string;
  points: { run: string; commit: string; wall: string; cpu: string; failed: string };
  leftOut: (count: number) => string;
  emptyTitle: string;
  emptyText: string;
  noDaemonTitle: string;
  noDaemonText: string;
  gone: string;
  /** The map's layer. */
  layer: string;
  layerTitle: (run: string) => string;
  layerEmpty: string;
  openPanel: string;
  /** On a box: the time spent in it and its calls. */
  boxWords: (total: string, own: string, calls: number) => string;
  boxShort: (time: string) => string;
  /** On a link: the measured traffic. */
  linkWords: (calls: number) => string;
  /** The regressions of the latest batch (ADR 0053). */
  regressionsTitle: string;
  regressionStatus: { probable: string; confirmed: string };
  regressionWords: (label: string, change: number, p: number) => string;
  noRegression: (checked: number) => string;
  regressionNote: Record<RegressionNote, (main: string) => string>;
  theRun: string;
  testNamed: (name: string) => string;
  theTests: string;
  /** A budget the last runs exceed (ADR 0052): « N-1 on Parser: p95 7.2 ms > 5 ms ». */
  budgetWords: (id: string, on: string, over: string) => string;
  limitWords: (kind: LimitKind, measured: number, limit: number) => string;
  budgetMark: string;
  legendBudget: string;
  /** In a session of the journal. */
  budgetsHeading: string;
  /** The legend of the layer. */
  legendTraffic: string;
  legendTime: string;
  legendTimeSample: string;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

export const performanceMessages: Record<Locale, PerformanceMessages> = {
  en: {
    title: "Performances",
    run: "Run",
    runOption: (run, where, ago) => [run, where, ago].filter(Boolean).join(" · "),
    running: "running",
    withoutTraces: "without traces",
    smoke: "smoke test of doctor",
    ago: (ms) => {
      const minutes = Math.round(ms / 60_000);
      if (minutes < 1) return "just now";
      if (minutes < 60) return `${minutes} min ago`;
      const hours = Math.round(minutes / 60);
      if (hours < 24) return `${hours} h ago`;
      return `${Math.round(hours / 24)} d ago`;
    },
    wall: "Duration",
    cpu: (source) =>
      source === "instructions" ? "CPU" : source === "wall_time" ? "Wall time (no CPU time here)" : "CPU time",
    instructions: (billions) => `${billions} G instructions`,
    memory: (whole) => (whole ? "Peak memory, all processes" : "Peak memory, largest process"),
    tests: (count, failed) => `${count} ${plural(count, "test", "tests")}${failed > 0 ? `, ${failed} failed` : ""}`,
    spans: (count) => `${count} ${plural(count, "span", "spans")}`,
    tabs: { timeline: "Timeline", tree: "Call tree", evolution: "Evolution" },
    testList: "Tests of the run",
    status: { passed: "passed", failed: "failed", error: "error", skipped: "skipped" },
    testLabel: (name, status, duration) => `${name}, ${status}, ${duration}`,
    undated: "not dated",
    noSpans:
      "No span for this test: its code is not instrumented yet. The conventions say how (get_instrumentation_conventions).",
    spansOf: (test) => `Spans of ${test}`,
    barTitle: (name, duration, node, failed) =>
      `${name} · ${duration}${node ? ` · ${node}` : ""}${failed ? " · error" : ""}`,
    spanError: "error",
    resources: "Resources of the process",
    noResources:
      "No resource curve for this run: it was measured without a systemd scope (Linux with systemd gives one).",
    time: "Since the start",
    memoryAxis: "Memory",
    cpuAxis: "CPU time",
    columns: { name: "Name", total: "Total", self: "Self", calls: "Calls", memory: "Memory", cpu: "CPU" },
    callTree: "Call tree of the run",
    resourcesNote: "Memory and CPU are those of the whole process while the level ran, from its samples every 100 ms.",
    evolution: "Duration and CPU time of the runs",
    evolutionNote:
      "The median of the runs of each commit, from the oldest; the same command on the same machine as the run shown. « * »: a modified tree, not the commit.",
    points: { run: "Run", commit: "Commit", wall: "Duration", cpu: "CPU time", failed: "Failed" },
    leftOut: (count) => `${count} more spans are not shown: the panel takes the first 20,000.`,
    emptyTitle: "No run yet",
    emptyText: "Run the tests with `timonier test -- <command>`: their duration, CPU, memory and traces show here.",
    noDaemonTitle: "No daemon",
    noDaemonText: "The Timonier daemon does not answer: the panel fills once it is back.",
    gone: "This run is no longer kept.",
    layer: "Performances",
    layerTitle: (run) => `Performances of ${run}`,
    layerEmpty: "No run with traces yet: run the tests with `timonier test`.",
    openPanel: "Open the Performances panel",
    boxWords: (total, own, calls) =>
      `${total} in it and what it called, ${own} of its own, ${calls} ${plural(calls, "call", "calls")}`,
    boxShort: (time) => time,
    linkWords: (calls) => `${calls} measured ${plural(calls, "call", "calls")}`,
    regressionsTitle: "Regressions of the last batch, against the main branch",
    regressionStatus: { probable: "Probable", confirmed: "Confirmed" },
    regressionWords: (label, change, p) =>
      `${label}: ${new Intl.NumberFormat("en", { signDisplay: "always", maximumFractionDigits: 1 }).format(change * 100)} % (p = ${new Intl.NumberFormat("en", { maximumSignificantDigits: 2 }).format(p)})`,
    noRegression: (checked) => `No regression: ${checked} measures compared with the main branch.`,
    regressionNote: {
      no_run: () => "No run yet.",
      no_batch: () => "No batch yet: run `timonier test --runs auto -- <command>`.",
      no_finished_run: () => "No finished run in this batch.",
      no_reference: (main) => `No reference yet: run the same tests on \`${main}\` first.`,
      machine_changed: (main) => `The machine changed: the reference starts again with the next runs on \`${main}\`.`,
      batch_too_small: () => "One run in this batch: run `timonier test --runs auto` to compare.",
    },
    theRun: "the run",
    testNamed: (name) => `test ${name}`,
    theTests: "the tests",
    budgetWords: (id, on, over) => `Budget ${id} exceeded on ${on}: ${over}`,
    limitWords: (kind, measured, limit) => limitWords("en", kind, measured, limit),
    budgetMark: "Budget exceeded",
    legendBudget: "A budget of the whiteboard the last runs exceed",
    budgetsHeading: "Budgets exceeded",
    legendTraffic: "Thicker: more calls measured",
    legendTime: "time spent in the box and what it called; its own time in its tooltip",
    legendTimeSample: "12 ms",
  },
  fr: {
    title: "Performances",
    run: "Run",
    runOption: (run, where, ago) => [run, where, ago].filter(Boolean).join(" · "),
    running: "en cours",
    withoutTraces: "sans traces",
    smoke: "test de fumée de doctor",
    ago: (ms) => {
      const minutes = Math.round(ms / 60_000);
      if (minutes < 1) return "à l'instant";
      if (minutes < 60) return `il y a ${minutes} min`;
      const hours = Math.round(minutes / 60);
      if (hours < 24) return `il y a ${hours} h`;
      return `il y a ${Math.round(hours / 24)} j`;
    },
    wall: "Durée",
    cpu: (source) =>
      source === "instructions" ? "CPU" : source === "wall_time" ? "Durée réelle (pas de temps CPU ici)" : "Temps CPU",
    instructions: (billions) => `${billions} G instructions`,
    memory: (whole) => (whole ? "Mémoire max, tous les processus" : "Mémoire max, plus gros processus"),
    tests: (count, failed) => `${count} ${plural(count, "test", "tests")}${failed > 0 ? `, ${failed} en échec` : ""}`,
    spans: (count) => `${count} spans`,
    tabs: { timeline: "Timeline", tree: "Arbre des appels", evolution: "Évolution" },
    testList: "Tests du run",
    status: { passed: "réussi", failed: "en échec", error: "en erreur", skipped: "sauté" },
    testLabel: (name, status, duration) => `${name}, ${status}, ${duration}`,
    undated: "non daté",
    noSpans:
      "Aucun span pour ce test : son code n'est pas encore instrumenté. Les conventions disent comment (get_instrumentation_conventions).",
    spansOf: (test) => `Spans de ${test}`,
    barTitle: (name, duration, node, failed) =>
      `${name} · ${duration}${node ? ` · ${node}` : ""}${failed ? " · en erreur" : ""}`,
    spanError: "erreur",
    resources: "Ressources du processus",
    noResources:
      "Pas de courbe des ressources pour ce run : il a été mesuré sans scope systemd (Linux avec systemd en donne une).",
    time: "Depuis le début",
    memoryAxis: "Mémoire",
    cpuAxis: "Temps CPU",
    columns: { name: "Nom", total: "Total", self: "Propre", calls: "Appels", memory: "Mémoire", cpu: "CPU" },
    callTree: "Arbre des appels du run",
    resourcesNote:
      "Mémoire et CPU sont ceux de tout le processus pendant que le niveau tournait, d'après ses échantillons toutes les 100 ms.",
    evolution: "Durée et temps CPU des runs",
    evolutionNote:
      "La médiane des runs de chaque commit, du plus ancien au plus récent ; même commande, même machine que le run affiché. « * » : un arbre modifié, pas le commit.",
    points: { run: "Run", commit: "Commit", wall: "Durée", cpu: "Temps CPU", failed: "En échec" },
    leftOut: (count) => `${count} spans de plus ne sont pas montrés : le panneau prend les 20 000 premiers.`,
    emptyTitle: "Aucun run pour l'instant",
    emptyText:
      "Lancez les tests avec `timonier test -- <commande>` : leur durée, leur CPU, leur mémoire et leurs traces s'affichent ici.",
    noDaemonTitle: "Pas de démon",
    noDaemonText: "Le démon Timonier ne répond pas : le panneau se remplit dès son retour.",
    gone: "Ce run n'est plus gardé.",
    layer: "Performances",
    layerTitle: (run) => `Performances de ${run}`,
    layerEmpty: "Aucun run avec traces pour l'instant : lancez les tests avec `timonier test`.",
    openPanel: "Ouvrir le panneau Performances",
    boxWords: (total, own, calls) =>
      `${total} dedans et dans ce qu'elle appelle, ${own} en propre, ${calls} ${plural(calls, "appel", "appels")}`,
    boxShort: (time) => time,
    linkWords: (calls) => `${calls} ${plural(calls, "appel mesuré", "appels mesurés")}`,
    regressionsTitle: "Régressions du dernier lot, face à la branche principale",
    regressionStatus: { probable: "Probable", confirmed: "Confirmée" },
    regressionWords: (label, change, p) =>
      `${label} : ${new Intl.NumberFormat("fr", { signDisplay: "always", maximumFractionDigits: 1 }).format(change * 100)} % (p = ${new Intl.NumberFormat("fr", { maximumSignificantDigits: 2 }).format(p)})`,
    noRegression: (checked) => `Aucune régression : ${checked} mesures comparées à la branche principale.`,
    regressionNote: {
      no_run: () => "Aucun run pour l'instant.",
      no_batch: () => "Aucun lot pour l'instant : lancez `timonier test --runs auto -- <commande>`.",
      no_finished_run: () => "Aucun run fini dans ce lot.",
      no_reference: (main) => `Pas encore de référence : lancez d'abord les mêmes tests sur \`${main}\`.`,
      machine_changed: (main) => `La machine a changé : la référence repart avec les prochains runs sur \`${main}\`.`,
      batch_too_small: () => "Un seul run dans ce lot : lancez `timonier test --runs auto` pour comparer.",
    },
    theRun: "le run",
    testNamed: (name) => `test ${name}`,
    theTests: "les tests",
    budgetWords: (id, on, over) => `Budget ${id} dépassé sur ${on} : ${over}`,
    limitWords: (kind, measured, limit) => limitWords("fr", kind, measured, limit),
    budgetMark: "Budget dépassé",
    legendBudget: "Un budget du tableau blanc que les derniers runs dépassent",
    budgetsHeading: "Budgets dépassés",
    legendTraffic: "Plus épais : plus d'appels mesurés",
    legendTime: "temps passé dans la boîte et ce qu'elle appelle ; son temps propre dans l'infobulle",
    legendTimeSample: "12 ms",
  },
};
