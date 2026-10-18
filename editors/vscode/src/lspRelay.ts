import path from "node:path";

import * as vscode from "vscode";
import { ResponseError, type CancellationToken } from "vscode-jsonrpc/node";

import { askAgain, inParallel } from "./parallel";
import { relativeInside, utf16ToUtf8, utf8ToUtf16 } from "./positions";
import {
  ERRORS,
  type DefinitionQuestion,
  type DefinitionTarget,
  type DefinitionsParams,
  type OutgoingCall,
  type OutgoingCallsParams,
  type ReferencesParams,
} from "./protocol/generated";
import type { Documents } from "./savedDocuments";

/** LSP's code for a request that failed: here, no server, or not one ready yet. */
const REQUEST_FAILED = ERRORS.REQUEST_FAILED;

/** At most this many questions to the language servers at once, for one batch (ADR 0022). */
const PARALLEL = 8;

/**
 * The pauses before asking again a batch where a name that tree-sitter places in another file
 * is « defined » in this one, or nowhere. While a project loads, VSCode's TypeScript syntax
 * server answers alone, from the file asked about: a type imported from elsewhere is defined
 * at its import. The daemon keeps tree-sitter's usages if the answer stays so (ADR 0022).
 */
const SETTLE_MS = [1000, 2000, 4000];

/**
 * A file of the repository, by its path relative to `root`, and a position of the model in it.
 * The document goes into `documents`, which the answer must match on disk (ADR 0022).
 */
async function at(root: string, file: string, position: { line: number; column: number }, documents: Documents) {
  const uri = vscode.Uri.file(path.join(root, ...file.split("/")));
  let opened = documents.get(file);
  if (!opened) {
    opened = vscode.workspace.openTextDocument(uri);
    documents.set(file, opened);
  }
  const document = await opened;
  const line = Math.min(position.line, Math.max(document.lineCount - 1, 0));
  return {
    uri,
    document,
    position: new vscode.Position(line, utf8ToUtf16(document.lineAt(line).text, position.column)),
  };
}

/**
 * A place VSCode names, as the model writes it: a path relative to `root` (`null` outside it),
 * and a UTF-8 column. The documents read are kept in `documents`, for the next places.
 */
async function place(
  root: string,
  uri: vscode.Uri,
  start: vscode.Position,
  documents: Documents,
): Promise<DefinitionTarget> {
  const relative = uri.scheme === "file" ? relativeInside(root, uri.fsPath) : null;
  if (relative === null) return { path: null, at: { line: start.line, column: start.character } };
  let document = documents.get(relative);
  if (!document) {
    document = vscode.workspace.openTextDocument(uri);
    documents.set(relative, document);
  }
  const text = (await document).lineAt(start.line).text;
  return { path: relative, at: { line: start.line, column: utf16ToUtf8(text, start.character) } };
}

/**
 * `lsp/outgoingCalls` (ADR 0014): the daemon asks for the calls of the function whose name
 * starts at a position; the IDE's language servers answer through the call hierarchy.
 */
export async function outgoingCalls(
  root: string,
  params: OutgoingCallsParams,
  documents: Documents,
): Promise<OutgoingCall[]> {
  const { uri, position } = await at(root, params.path, params.position, documents);
  const items = await vscode.commands.executeCommand<vscode.CallHierarchyItem[] | undefined>(
    "vscode.prepareCallHierarchy",
    uri,
    position,
  );
  const item = items?.[0];
  // No server for this language, or not ready yet: not « no calls ». The file stays unverified.
  if (!item) throw new ResponseError(REQUEST_FAILED, `no call hierarchy at ${params.path}:${params.position.line + 1}`);
  const calls =
    (await vscode.commands.executeCommand<vscode.CallHierarchyOutgoingCall[] | undefined>(
      "vscode.provideOutgoingCalls",
      item,
    )) ?? [];
  const answer: OutgoingCall[] = [];
  for (const call of calls) {
    const target = await place(root, call.to.uri, call.to.selectionRange.start, documents);
    answer.push({ ...target, sites: call.fromRanges.length });
  }
  return answer;
}

/**
 * `lsp/definitions` (ADR 0022): where the names at these positions of a file are defined, one
 * list per question, asked in parallel; asked again while doubtful (`SETTLE_MS`). Nothing
 * defined anywhere means no server, or one not ready: a failure, not « defined nowhere », or the
 * daemon would drop what tree-sitter saw.
 */
export async function definitions(
  root: string,
  params: DefinitionsParams,
  documents: Documents,
  token?: CancellationToken,
): Promise<DefinitionTarget[][]> {
  // The questions still doubtful: placed elsewhere by tree-sitter, here or nowhere by the server.
  const doubtful = (answer: readonly DefinitionTarget[][]) =>
    params.questions.flatMap((question, i) =>
      question.elsewhere && (answer[i] ?? []).every((target) => target.path === params.path) ? [i] : [],
    );
  const answer = await definitionsOnce(root, params.path, params.questions, documents, token);
  // The doubtful questions alone, asked again, while the daemon waits.
  await askAgain(
    answer,
    doubtful,
    SETTLE_MS,
    () => token?.isCancellationRequested ?? false,
    (again) =>
      definitionsOnce(
        root,
        params.path,
        again.map((i) => params.questions[i] as DefinitionQuestion),
        documents,
        token,
      ),
  );
  if (answer.length > 0 && answer.every((targets) => targets.length === 0)) {
    throw new ResponseError(REQUEST_FAILED, `no definitions in ${params.path}`);
  }
  return answer;
}

async function definitionsOnce(
  root: string,
  file: string,
  questions: readonly DefinitionQuestion[],
  documents: Documents,
  token?: CancellationToken,
): Promise<DefinitionTarget[][]> {
  const answer = await inParallel(questions, PARALLEL, async (question) => {
    // The daemon no longer waits: nothing more to ask.
    if (token?.isCancellationRequested) return [];
    const { uri, position: name } = await at(root, file, question.position, documents);
    let found = await defined(uri, name);
    // A definition in the same file may be an import that binds the name (TypeScript answers
    // so for `import type`): the declaration is one step further, asked from there.
    for (let hop = 0; hop < 2; hop++) {
      const first = found[0];
      if (!first || !found.every((d) => d.uri.toString() === uri.toString())) break;
      const further = await defined(uri, first.start);
      if (further.length === 0 || further[0]?.start.isEqual(first.start)) break;
      found = further;
    }
    return Promise.all(found.map((d) => place(root, d.uri, d.start, documents)));
  });
  return answer;
}

/** Where VSCode's definition providers say the name at `position` is defined. */
async function defined(
  uri: vscode.Uri,
  position: vscode.Position,
): Promise<{ uri: vscode.Uri; start: vscode.Position }[]> {
  const found =
    (await vscode.commands.executeCommand<(vscode.Location | vscode.LocationLink)[] | undefined>(
      "vscode.executeDefinitionProvider",
      uri,
      position,
    )) ?? [];
  return found.map((definition) =>
    "targetUri" in definition
      ? { uri: definition.targetUri, start: (definition.targetSelectionRange ?? definition.targetRange).start }
      : { uri: definition.uri, start: definition.range.start },
  );
}

/**
 * `lsp/references` (ADR 0022): the files of the repository that use the symbol named at a
 * position. VSCode counts its declaration among them: none at all means no server, or one not
 * ready, and the daemon falls back on the name.
 */
export async function references(root: string, params: ReferencesParams, documents: Documents): Promise<string[]> {
  const { uri, position } = await at(root, params.path, params.position, documents);
  // Users that stay in the file prove nothing: the daemon then falls back on the name.
  const found =
    (await vscode.commands.executeCommand<vscode.Location[] | undefined>(
      "vscode.executeReferenceProvider",
      uri,
      position,
    )) ?? [];
  if (found.length === 0) throw new ResponseError(REQUEST_FAILED, `no references at ${params.path}`);
  const files = new Set<string>();
  for (const location of found) {
    if (location.uri.scheme !== "file") continue;
    const relative = relativeInside(root, location.uri.fsPath);
    if (relative !== null) files.add(relative);
  }
  return [...files].sort();
}

/** The files of the visible editors, relative to `root`: `editor/opened` (ADR 0014). */
export function openedFiles(root: string): string[] {
  const files = new Set<string>();
  for (const editor of vscode.window.visibleTextEditors) {
    const uri = editor.document.uri;
    if (uri.scheme !== "file") continue;
    const relative = relativeInside(root, uri.fsPath);
    if (relative !== null) files.add(relative);
  }
  return [...files].sort();
}
