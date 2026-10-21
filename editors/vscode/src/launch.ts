/** How the extension launches the daemon: pure, for the unit tests (ADR 0019). */

/** Overrides `timonier.daemon.path`: for the integration tests and development. */
export const DAEMON_PATH_VARIABLE = "TIMONIER_DAEMON_PATH";

/** Delay before launch number `failures + 1`: doubles from 1 s, up to 30 s (ADR 0019). */
export function backoff(failures: number): number {
  return Math.min(30_000, 1_000 * 2 ** Math.max(0, failures - 1));
}

/**
 * The program to launch: the environment, then the setting, then the one this package ships
 * (ADR 0059), then `timonier` on the PATH.
 */
export function daemonProgram(env: NodeJS.ProcessEnv, setting: string | undefined, bundled?: string): string {
  const fromEnv = env[DAEMON_PATH_VARIABLE];
  if (fromEnv) return fromEnv;
  if (setting && setting.trim() !== "") return setting.trim();
  return bundled ?? "timonier";
}

/** The line `timonier daemon run` writes once its socket listens (ADR 0019). */
export interface Announce {
  socket: string;
  /** The daemon's process; `null` when another process serves. */
  pid: number | null;
}

export function parseAnnounce(line: string): Announce | undefined {
  try {
    const value: unknown = JSON.parse(line);
    if (typeof value !== "object" || value === null) return undefined;
    const { socket, pid } = value as { socket?: unknown; pid?: unknown };
    if (typeof socket !== "string") return undefined;
    if (pid !== null && typeof pid !== "number") return undefined;
    return { socket, pid };
  } catch {
    return undefined;
  }
}

/** The command line that runs `timonier init` in a shell (Q53): the program quoted for POSIX shells. */
export function initCommand(program: string): string {
  return `'${program.replace(/'/g, "'\\''")}' init`;
}
