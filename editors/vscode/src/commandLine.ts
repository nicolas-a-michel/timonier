/**
 * The `timonier` program the extension ships (ADR 0059): the one in `bin/`, a copy of it in the
 * machine's data folder, and what puts that copy on the agents' PATH: links in `~/.local/bin` on
 * Linux and macOS, the folder itself in the user's PATH on Windows (no links without developer
 * mode there).
 */
import { accessSync, chmodSync, constants } from "node:fs";
import {
  access,
  chmod,
  copyFile,
  lstat,
  mkdir,
  readFile,
  readlink,
  rename,
  stat,
  symlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";

/** The program's file name on a platform. */
export function programName(platform: NodeJS.Platform, name = "timonier"): string {
  return platform === "win32" ? `${name}.exe` : name;
}

/** The program in the extension's `bin/`, when this package has one (a platform's `.vsix`). */
export function bundledProgram(
  extensionPath: string,
  platform: NodeJS.Platform = process.platform,
): string | undefined {
  const program = path.join(extensionPath, "bin", programName(platform));
  try {
    accessSync(program, constants.F_OK);
  } catch {
    return undefined;
  }
  if (platform !== "win32") {
    try {
      accessSync(program, constants.X_OK);
    } catch {
      // An unpacking that lost the mode the package keeps.
      chmodSync(program, 0o755);
    }
  }
  return program;
}

/** The machine's data folder, as the daemon finds it (ADR 0057). */
export function dataDir(env: NodeJS.ProcessEnv, home: string, platform: NodeJS.Platform = process.platform): string {
  if (env.TIMONIER_DATA_DIR) return env.TIMONIER_DATA_DIR;
  if (platform === "win32") return path.join(env.LOCALAPPDATA ?? path.join(home, "AppData", "Local"), "timonier");
  if (env.XDG_DATA_HOME) return path.join(env.XDG_DATA_HOME, "timonier");
  return path.join(home, ".local", "share", "timonier");
}

/** Where the program `name` is on `PATH`, if it is. */
export async function onPath(
  name: string,
  searchPath: string | undefined,
  platform: NodeJS.Platform = process.platform,
): Promise<string | undefined> {
  const separator = platform === "win32" ? ";" : ":";
  for (const dir of (searchPath ?? "").split(separator).filter(Boolean)) {
    const candidate = path.join(dir, programName(platform, name));
    try {
      await access(candidate, platform === "win32" ? constants.F_OK : constants.X_OK);
      return candidate;
    } catch {
      // Not there.
    }
  }
  return undefined;
}

/** What a copy was made from: the shipped program's path, size and time. */
async function origin(bundled: string): Promise<string> {
  const info = await stat(bundled);
  return `${bundled}\n${info.size}\n${info.mtimeMs}\n`;
}

/**
 * `<data>/bin/<name>`, a copy of the shipped program, made or replaced when it came from another
 * one: written beside, then renamed, so that a copy is never half written.
 */
async function copied(bundled: string, dir: string, name: string): Promise<string> {
  const copy = path.join(dir, name);
  const stamp = path.join(dir, `${name}.origin`);
  const wanted = await origin(bundled);
  const had = await readFile(stamp, "utf8").catch(() => "");
  const present = await access(copy).then(
    () => true,
    () => false,
  );
  if (had !== wanted || !present) {
    await mkdir(dir, { recursive: true });
    const next = path.join(dir, `.${name}.next`);
    await copyFile(bundled, next);
    await chmod(next, 0o755);
    await rename(next, copy);
    await writeFile(stamp, wanted);
  }
  return copy;
}

/** The copies in the data folder: `timonier`, and `tim` too on Windows. */
export async function refreshCopy(
  bundled: string,
  data: string,
  platform: NodeJS.Platform = process.platform,
): Promise<string> {
  const dir = path.join(data, "bin");
  const copy = await copied(bundled, dir, programName(platform));
  if (platform === "win32") await copied(bundled, dir, programName(platform, "tim"));
  return copy;
}

export interface Installed {
  /** The links or copies made, or already there, that run the copy. */
  linked: string[];
  /** The names left as they were: they hold something else. */
  left: string[];
  /** The folder is not on the PATH of this process: the agents see it once restarted, or not. */
  notOnPath: boolean;
}

export interface Machine {
  env: NodeJS.ProcessEnv;
  home: string;
  platform: NodeJS.Platform;
  /** Adds a folder to the user's PATH for the next processes (Windows). */
  addToUserPath: (dir: string) => Promise<void>;
}

/**
 * Puts the shipped program on the agents' PATH (ADR 0059). Linux and macOS: the copy, then
 * `~/.local/bin/timonier` and `~/.local/bin/tim` linked to it, a file or a link that points
 * elsewhere left. Windows: the copies, and their folder in the user's PATH.
 */
export async function installCommandLine(bundled: string, machine: Machine): Promise<Installed> {
  const { env, home, platform } = machine;
  const data = dataDir(env, home, platform);
  const copy = await refreshCopy(bundled, data, platform);
  const installed: Installed = { linked: [], left: [], notOnPath: false };
  const separator = platform === "win32" ? ";" : ":";
  const pathHas = (dir: string) =>
    (env.PATH ?? env.Path ?? "")
      .split(separator)
      .some((entry) => path.resolve(entry).toLowerCase() === path.resolve(dir).toLowerCase());
  if (platform === "win32") {
    const dir = path.dirname(copy);
    installed.linked.push(copy, path.join(dir, programName(platform, "tim")));
    if (!pathHas(dir)) {
      await machine.addToUserPath(dir);
      installed.notOnPath = true;
    }
    return installed;
  }
  const bin = path.join(home, ".local", "bin");
  await mkdir(bin, { recursive: true });
  for (const name of ["timonier", "tim"]) {
    const link = path.join(bin, name);
    const there = await lstat(link).catch(() => undefined);
    if (!there) {
      await symlink(copy, link);
      installed.linked.push(link);
    } else if (there.isSymbolicLink() && (await readlink(link)) === copy) {
      installed.linked.push(link);
    } else {
      installed.left.push(link);
    }
  }
  installed.notOnPath = !pathHas(bin);
  return installed;
}
