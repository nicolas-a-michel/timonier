/**
 * Renaming a box of the map (ADR 0075, Q96): its name, then its role, asked in VSCode's own
 * input and quick pick, then given to the daemon (`names/set`), which keeps them in
 * `.timonier/names.json`. An empty name and « as Timonier computes it » give the box back.
 */
import type { Role } from "./protocol/generated";

/** What the dialogs answer: `undefined` when the developer cancels. */
export interface Ask {
  name(current: string): Thenable<string | undefined>;
  /** The role chosen: a role, `null` for the one Timonier computes, `undefined` to cancel. */
  role(current: Role | null): Thenable<Role | null | undefined>;
}

export interface Rename {
  target: string;
  name: string;
  role: Role | null;
}

/** What `names/set` is given, or `undefined` when cancelled. */
export async function askRename(
  box: Rename,
  ask: Ask,
): Promise<{ id: string; name: string | null; role: Role | null } | undefined> {
  const name = await ask.name(box.name);
  if (name === undefined) return undefined;
  const role = await ask.role(box.role);
  if (role === undefined) return undefined;
  const trimmed = name.trim();
  return { id: box.target, name: trimmed === "" ? null : trimmed, role };
}

/** The roles the quick pick offers, in the map's order. */
export const ROLES: Role[] = [
  "project",
  "entry_point",
  "interface",
  "background_task",
  "data_model",
  "persistence",
  "utility",
  "external_libraries",
];
