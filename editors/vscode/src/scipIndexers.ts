/**
 * « Install the SCIP indexers » (ADR 0073, Q94): what `timonier scip --json` says of the
 * project's languages, the commands that install the missing indexers, shown before they run,
 * since this machine approves them by this click; then `timonier scip --install --approve` in a
 * terminal, which asks the daemon to index the last commit at once.
 */

/** One language of the project and its indexer, as `timonier scip --json` says it. */
export interface IndexerLine {
  language: string;
  indexer: string;
  found: boolean;
  install: string | null;
  note: string | null;
}

/** What the click installs, and what is left to do by hand. */
export interface Missing {
  commands: string[];
  byHand: string[];
}

/** The missing indexers of `json`: their install commands, once each, and the others' notes. */
export function missingIndexers(json: string): Missing | null {
  let lines: IndexerLine[];
  try {
    const parsed = JSON.parse(json) as unknown;
    if (!Array.isArray(parsed)) return null;
    lines = parsed as IndexerLine[];
  } catch {
    return null;
  }
  const missing = lines.filter((line) => !line.found);
  const commands = [...new Set(missing.flatMap((line) => (line.install ? [line.install] : [])))];
  const byHand = missing
    .filter((line) => !line.install)
    .map((line) => `${line.language}, ${line.indexer}: ${line.note ?? ""}`.trim());
  return { commands, byHand };
}

/** What the terminal runs: the program itself, no shell to quote for. */
export const SCIP_INSTALL_ARGS = ["scip", "--install", "--approve"];
