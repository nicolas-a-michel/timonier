import * as vscode from "vscode";

import { onSavedContent, sameText, UnsavedSources, type TrackedDocument } from "./content";
import { relativeInside } from "./positions";

/** The documents a question read, by their path relative to the root. */
export type Documents = Map<string, PromiseLike<vscode.TextDocument>>;

/** Notifications of `editor/saved` closer than this go as one. */
const SETTLE_MS = 100;

/**
 * Whether VSCode's source files are what is on disk (ADR 0022). Its language servers answer
 * about the open documents: while one has unsaved changes, or VSCode has yet to read a change
 * again, they answer about another content than the one the daemon indexed, and the daemon
 * would record it as verified at the hash of the disk. Source files are those the daemon's
 * packs read: a note being written changes no answer.
 */
export class SavedDocuments implements vscode.Disposable {
  private readonly sources: UnsavedSources;
  /**
   * What the last comparison with the disk found, per document version and file stamp. By
   * document: one closed then opened again is another, whose version starts again at 1.
   */
  private readonly compared = new WeakMap<vscode.TextDocument, { stamp: string; same: boolean }>();
  private readonly subscriptions: vscode.Disposable[];
  private timer: NodeJS.Timeout | undefined;

  /** `onSaved` runs once no source file differs from the disk, after a change. */
  constructor(
    private readonly root: string,
    private readonly onSaved: () => void,
  ) {
    this.sources = new UnsavedSources(this.documents());
    this.subscriptions = [
      vscode.workspace.onDidChangeTextDocument((event) =>
        this.changed(event.document, event.contentChanges.length > 0),
      ),
      vscode.workspace.onDidSaveTextDocument((document) => this.changed(document, false)),
      vscode.workspace.onDidCloseTextDocument((document) => {
        const file = this.file(document);
        if (file !== null && this.sources.closed(file)) this.settle();
      }),
    ];
  }

  dispose(): void {
    clearTimeout(this.timer);
    for (const subscription of this.subscriptions) subscription.dispose();
  }

  /** The extensions the daemon's packs read, from `initialize`. */
  setSources(extensions: unknown): void {
    this.sources.setSources(extensions, this.documents());
    this.settle();
  }

  /** `ask`, on saved content only (`onSavedContent`): the daemon holds what is refused. */
  answer<T>(ask: (documents: Documents) => Promise<T>): Promise<T> {
    return onSavedContent(
      () => this.sources.state(),
      ask,
      (document) => this.onDisk(document),
    );
  }

  /** Whether the document is the file on disk, compared once per version and file stamp. */
  private async onDisk(document: vscode.TextDocument): Promise<boolean> {
    try {
      const stat = await vscode.workspace.fs.stat(document.uri);
      const stamp = `${document.version}:${stat.mtime}:${stat.size}`;
      const known = this.compared.get(document);
      if (known?.stamp === stamp) return known.same;
      // Decoded as VSCode decoded the document: only a change it has not read makes them differ.
      const bytes = await vscode.workspace.fs.readFile(document.uri);
      const same = sameText(await vscode.workspace.decode(bytes, { encoding: document.encoding }), document.getText());
      this.compared.set(document, { stamp, same });
      return same;
    } catch {
      // Gone, or binary: not the document.
      return false;
    }
  }

  private changed(document: vscode.TextDocument, content: boolean): void {
    const file = this.file(document);
    if (file !== null && this.sources.changed({ file, dirty: document.isDirty }, content)) this.settle();
  }

  /** Nothing differs from the disk any more, as far as VSCode knows: say so, once settled. */
  private settle(): void {
    if (!this.sources.settled) return;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      if (this.sources.settled) this.onSaved();
    }, SETTLE_MS);
  }

  /** VSCode's documents of the repository. */
  private documents(): TrackedDocument[] {
    return vscode.workspace.textDocuments.flatMap((document) => {
      const file = this.file(document);
      return file === null ? [] : [{ file, dirty: document.isDirty }];
    });
  }

  /** The document's path relative to the root; `null` outside the repository. */
  private file(document: vscode.TextDocument): string | null {
    return document.uri.scheme === "file" ? relativeInside(this.root, document.uri.fsPath) : null;
  }
}
