import type { Locale } from "../i18n";
import type { WhiteboardTool } from "../protocol";
import type { Limits, Policy } from "./board";

/** The words of the whiteboard (ADR 0036 to 0038). */
export interface WhiteboardMessages {
  label: string;
  tools: string;
  /** Each tool, with the key that picks it. */
  tool: Record<WhiteboardTool, string>;
  undo: string;
  redo: string;
  deleteSelection: string;
  fitView: string;
  zoomIn: string;
  zoomOut: string;
  /** A stroke's name, for screen readers: its rank in the drawing order. */
  stroke: (rank: number) => string;
  /** What every element shares, besides its name: what the keyboard does with it. */
  strokeHint: string;
  emptyTitle: string;
  emptyText: string;
  /** Said when the board changes: how many strokes and shapes it holds. */
  count: (strokes: number, shapes: number) => string;
  /** What the arrow tool asks for, then. */
  arrowFrom: string;
  arrowTo: (from: string) => string;
  /** The typed shapes. */
  kind: { box: string; zone: string; arrow: string; note: string; budget: string };
  unnamedBox: string;
  unnamedZone: string;
  emptyNote: string;
  noLimit: string;
  /** « Parser, intended component, in Domain. Turns tokens into a tree. » */
  boxLabel: (name: string, layer: string | null, role: string) => string;
  zoneLabel: (name: string, boxes: number) => string;
  arrowLabel: (policy: Policy, from: string, to: string) => string;
  noteLabel: (text: string, on: string | null) => string;
  budgetLabel: (limits: string, on: string | null) => string;
  /** On a note or a budget: what it is attached to. */
  on: (target: string) => string;
  policy: Record<Policy, string>;
  /** The short word an arrow carries on the board, when it is not merely allowed. */
  policyMark: Record<Policy, string>;
  limit: Record<keyof Limits, string>;
  /** « p95 < 5 ms » */
  limitLine: (key: keyof Limits, value: number) => string;
  /** The properties panel. */
  properties: string;
  name: string;
  role: string;
  text: string;
  width: string;
  height: string;
  attachedTo: string;
  nothing: string;
  from: string;
  to: string;
  arrowToField: string;
  addArrow: string;
  remove: string;
  /** Saving to `.timonier/intent.json` (Q66). */
  saved: string;
  notSaved: (reason: string) => string;
  /** The file does not read. */
  unreadTitle: string;
  unreadText: string;
  startOver: string;
  /** The daemon read the file, but the whiteboard cannot draw it. */
  cannotDraw: string;
}

const UNITS: Record<keyof Limits, string> = { p50Ms: "ms", p95Ms: "ms", memoryMb: "MB", allocations: "", calls: "" };
const UNITS_FR: Record<keyof Limits, string> = { p50Ms: "ms", p95Ms: "ms", memoryMb: "Mo", allocations: "", calls: "" };

export const whiteboardMessages: Record<Locale, WhiteboardMessages> = {
  en: {
    label: "Whiteboard",
    tools: "Whiteboard tools",
    tool: {
      select: "Select (V)",
      pen: "Draw (P)",
      eraser: "Erase (E)",
      box: "Box (B)",
      zone: "Zone (Z)",
      arrow: "Arrow (A)",
      note: "Note (N)",
      budget: "Budget (M)",
    },
    undo: "Undo",
    redo: "Redo",
    deleteSelection: "Delete",
    fitView: "Fit to view",
    zoomIn: "Zoom in",
    zoomOut: "Zoom out",
    stroke: (rank) => `Stroke ${rank}`,
    strokeHint: "Enter selects it, the arrow keys move it, Delete removes it.",
    emptyTitle: "An empty whiteboard",
    emptyText:
      "Draw the architecture you want: press P and draw freely, or B for a component, Z for a layer, A for an arrow between them.",
    count: (strokes, shapes) =>
      `${strokes === 1 ? "1 stroke" : `${strokes} strokes`}, ${shapes === 1 ? "1 shape" : `${shapes} shapes`}`,
    arrowFrom: "Click the box or zone the arrow leaves from.",
    arrowTo: (from) => `From ${from}: click the box or zone it goes to. Escape cancels.`,
    kind: { box: "Component", zone: "Layer", arrow: "Dependency", note: "Note", budget: "Budget" },
    unnamedBox: "Unnamed component",
    unnamedZone: "Unnamed layer",
    emptyNote: "Empty note",
    noLimit: "No limit yet",
    boxLabel: (name, layer, role) =>
      `${name}, intended component${layer ? `, in ${layer}` : ""}.${role ? ` ${role}` : ""}`,
    zoneLabel: (name, boxes) => `${name}, layer, ${boxes === 1 ? "1 component" : `${boxes} components`}.`,
    arrowLabel: (policy, from, to) =>
      `${{ allowed: "Allowed", forbidden: "Forbidden", required: "Required" }[policy]} dependency from ${from} to ${to}.`,
    noteLabel: (text, on) => `Note${on ? ` on ${on}` : ""}: ${text}`,
    budgetLabel: (limits, on) => `Budget${on ? ` of ${on}` : ""}: ${limits}`,
    on: (target) => `On ${target}`,
    policy: { allowed: "Allowed", forbidden: "Forbidden", required: "Required" },
    policyMark: { allowed: "allowed", forbidden: "forbidden", required: "required" },
    limit: {
      p50Ms: "Latency p50 (ms)",
      p95Ms: "Latency p95 (ms)",
      memoryMb: "Peak memory (MB)",
      allocations: "Allocations",
      calls: "Calls",
    },
    limitLine: (key, value) =>
      `${{ p50Ms: "p50", p95Ms: "p95", memoryMb: "memory", allocations: "allocations", calls: "calls" }[key]} < ${value}${UNITS[key] ? ` ${UNITS[key]}` : ""}`,
    properties: "Properties",
    name: "Name",
    role: "What it does",
    text: "Text",
    width: "Width",
    height: "Height",
    attachedTo: "Attached to",
    nothing: "Nothing",
    from: "From",
    to: "To",
    arrowToField: "New arrow to",
    addArrow: "Add the arrow",
    remove: "Delete",
    saved: "Saved in .timonier/intent.json",
    notSaved: (reason) => `Not saved: ${reason}`,
    unreadTitle: "The whiteboard does not read",
    unreadText:
      "Timonier cannot read .timonier/intent.json. Fix the file and open the whiteboard again, or start over: your next gesture replaces it.",
    startOver: "Start over with an empty whiteboard",
    cannotDraw: "The whiteboard cannot draw what .timonier/intent.json holds (a shape it does not know how to draw).",
  },
  fr: {
    label: "Tableau blanc",
    tools: "Outils du tableau blanc",
    tool: {
      select: "Sélectionner (V)",
      pen: "Dessiner (P)",
      eraser: "Gommer (E)",
      box: "Boîte (B)",
      zone: "Zone (Z)",
      arrow: "Flèche (A)",
      note: "Note (N)",
      budget: "Budget (M)",
    },
    undo: "Annuler",
    redo: "Rétablir",
    deleteSelection: "Supprimer",
    fitView: "Tout afficher",
    zoomIn: "Zoom avant",
    zoomOut: "Zoom arrière",
    stroke: (rank) => `Trait ${rank}`,
    strokeHint: "Entrée le sélectionne, les flèches le déplacent, Suppr l'efface.",
    emptyTitle: "Un tableau blanc vide",
    emptyText:
      "Dessinez l'architecture que vous voulez : P pour dessiner librement, B pour un composant, Z pour une couche, A pour une flèche entre eux.",
    count: (strokes, shapes) =>
      `${strokes <= 1 ? `${strokes} trait` : `${strokes} traits`}, ${shapes <= 1 ? `${shapes} forme` : `${shapes} formes`}`,
    arrowFrom: "Cliquez la boîte ou la zone d'où part la flèche.",
    arrowTo: (from) => `Depuis ${from} : cliquez la boîte ou la zone où elle va. Échap annule.`,
    kind: { box: "Composant", zone: "Couche", arrow: "Dépendance", note: "Note", budget: "Budget" },
    unnamedBox: "Composant sans nom",
    unnamedZone: "Couche sans nom",
    emptyNote: "Note vide",
    noLimit: "Aucune limite encore",
    boxLabel: (name, layer, role) =>
      `${name}, composant voulu${layer ? `, dans ${layer}` : ""}.${role ? ` ${role}` : ""}`,
    zoneLabel: (name, boxes) => `${name}, couche, ${boxes <= 1 ? `${boxes} composant` : `${boxes} composants`}.`,
    arrowLabel: (policy, from, to) =>
      `Dépendance ${{ allowed: "autorisée", forbidden: "interdite", required: "requise" }[policy]} de ${from} vers ${to}.`,
    noteLabel: (text, on) => `Note${on ? ` sur ${on}` : ""} : ${text}`,
    budgetLabel: (limits, on) => `Budget${on ? ` de ${on}` : ""} : ${limits}`,
    on: (target) => `Sur ${target}`,
    policy: { allowed: "Autorisée", forbidden: "Interdite", required: "Requise" },
    policyMark: { allowed: "autorisée", forbidden: "interdite", required: "requise" },
    limit: {
      p50Ms: "Latence p50 (ms)",
      p95Ms: "Latence p95 (ms)",
      memoryMb: "Mémoire max (Mo)",
      allocations: "Allocations",
      calls: "Appels",
    },
    limitLine: (key, value) =>
      `${{ p50Ms: "p50", p95Ms: "p95", memoryMb: "mémoire", allocations: "allocations", calls: "appels" }[key]} < ${String(value).replace(".", ",")}${UNITS_FR[key] ? ` ${UNITS_FR[key]}` : ""}`,
    properties: "Propriétés",
    name: "Nom",
    role: "Ce qu'il fait",
    text: "Texte",
    width: "Largeur",
    height: "Hauteur",
    attachedTo: "Attachée à",
    nothing: "Rien",
    from: "De",
    to: "Vers",
    arrowToField: "Nouvelle flèche vers",
    addArrow: "Ajouter la flèche",
    remove: "Supprimer",
    saved: "Enregistré dans .timonier/intent.json",
    notSaved: (reason) => `Non enregistré : ${reason}`,
    unreadTitle: "Le tableau blanc ne se lit pas",
    unreadText:
      "Timonier ne peut pas lire .timonier/intent.json. Corrigez le fichier et rouvrez le tableau blanc, ou repartez de zéro : votre prochain geste le remplacera.",
    startOver: "Repartir d'un tableau blanc vide",
    cannotDraw:
      "Le tableau blanc ne sait pas dessiner ce que contient .timonier/intent.json (une forme qu'il ne sait pas dessiner).",
  },
};
