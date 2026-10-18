/**
 * Whether VSCode's documents are the files on disk, and the rule of the relay that follows
 * (ADR 0022). Pure, for the unit tests.
 */
import { ResponseError } from "vscode-jsonrpc/node";

import { ERRORS } from "./protocol/generated";

/** What the answers depend on: an unsaved source file, if any, and the changes so far. */
export interface DocumentState {
  readonly unsaved: string | undefined;
  readonly changes: number;
}

/**
 * `ask`, on saved content only. Refused with `ContentModified`, as in LSP:
 * - at once, without asking the servers, while a source file has unsaved changes;
 * - when one has them at the end, or a source file changed during the question;
 * - when a document the question read (`documents`) is not what is on disk.
 */
export async function onSavedContent<T, D>(
  state: () => DocumentState,
  ask: (documents: Map<string, PromiseLike<D>>) => Promise<T>,
  onDisk: (document: D) => Promise<boolean>,
): Promise<T> {
  const before = state();
  if (before.unsaved !== undefined) throw modified(`unsaved changes in ${before.unsaved}`);
  const documents = new Map<string, PromiseLike<D>>();
  const answer = await ask(documents);
  const after = state();
  if (after.unsaved !== undefined) throw modified(`unsaved changes in ${after.unsaved}`);
  if (after.changes !== before.changes) throw modified("a source file changed meanwhile");
  for (const [file, document] of documents) {
    if (!(await onDisk(await document))) throw modified(`${file} is not what is on disk`);
  }
  return answer;
}

/** A document of the repository, as the tracking of unsaved source files sees it. */
export interface TrackedDocument {
  /** Its path relative to the root. */
  readonly file: string;
  readonly dirty: boolean;
}

/**
 * The source files with unsaved changes, and the changes to source files so far: what
 * `onSavedContent` asks. Source files are those the daemon's packs read, once `initialize` says
 * which (`sourceExtensions`); before, or from a daemon that does not say, every file counts.
 */
export class UnsavedSources {
  private readonly unsaved = new Set<string>();
  private changes = 0;
  private extensions: ReadonlySet<string> | undefined;

  constructor(documents: Iterable<TrackedDocument>) {
    this.track(documents);
  }

  /** The extensions from `initialize`, and the documents open now, to count them again. */
  setSources(extensions: unknown, documents: Iterable<TrackedDocument>): void {
    this.extensions =
      Array.isArray(extensions) && extensions.every((e) => typeof e === "string") ? new Set(extensions) : undefined;
    this.track(documents);
  }

  /** A document changed: its text (`content`), or only its dirty state. Whether it counts. */
  changed(document: TrackedDocument, content: boolean): boolean {
    if (!isSource(document.file, this.extensions)) return false;
    if (content) this.changes++;
    if (document.dirty) this.unsaved.add(document.file);
    else this.unsaved.delete(document.file);
    return true;
  }

  /** A document closed, saved or not: VSCode's servers read the disk again. Whether it counts. */
  closed(file: string): boolean {
    if (!isSource(file, this.extensions)) return false;
    this.unsaved.delete(file);
    return true;
  }

  state(): DocumentState {
    return { unsaved: this.unsaved.values().next().value, changes: this.changes };
  }

  /** No source file differs from the disk, as far as VSCode knows. */
  get settled(): boolean {
    return this.unsaved.size === 0;
  }

  private track(documents: Iterable<TrackedDocument>): void {
    this.unsaved.clear();
    for (const document of documents) {
      if (document.dirty && isSource(document.file, this.extensions)) this.unsaved.add(document.file);
    }
  }
}

/**
 * The file's text, decoded as VSCode decodes it, against a document's text. A document with
 * mixed `\r\n` and `\n` line ends gets one of them: that changes no line or column of the
 * model. A lone `\r` does: VSCode ends a line there, the daemon does not.
 */
export function sameText(onDisk: string, text: string): boolean {
  return lineEnds(onDisk) === lineEnds(text);
}

/**
 * Whether a file, by its path, is one the daemon's packs read (`extensions`, from
 * `initialize`): its unsaved changes are what the servers would answer about. Unknown yet, any
 * file is.
 */
export function isSource(file: string, extensions: ReadonlySet<string> | undefined): boolean {
  if (extensions === undefined) return true;
  const name = file.slice(file.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0 && extensions.has(name.slice(dot + 1));
}

function lineEnds(text: string): string {
  return text.replace(/\r\n/g, "\n");
}

function modified(reason: string): ResponseError {
  return new ResponseError(ERRORS.CONTENT_MODIFIED, reason);
}
