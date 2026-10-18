import * as vscode from "vscode";

import { resolveInside, utf8ToUtf16 } from "./positions";
import type { Span } from "./protocol/generated";

/** A position of the model in `document`, inside it even if the file changed since. */
function position(document: vscode.TextDocument, at: Span["start"]): vscode.Position {
  const line = Math.min(Math.max(at.line, 0), Math.max(document.lineCount - 1, 0));
  return new vscode.Position(line, utf8ToUtf16(document.lineAt(line).text, at.column));
}

/**
 * Opens `file` (relative to `root`) beside the map, the cursor at the start of `span` and the
 * span in view (PLAN.md 4.9). Columns of the model are UTF-8 bytes; VSCode's are UTF-16 units.
 */
export async function revealCode(root: string, file: string, span: Span | null): Promise<vscode.TextEditor> {
  // The webview names the file: it must stay inside the project.
  const inside = resolveInside(root, file);
  if (!inside) throw new Error(`${file} is not inside ${root}`);
  const uri = vscode.Uri.file(inside);
  const document = await vscode.workspace.openTextDocument(uri);
  const start = span ? position(document, span.start) : new vscode.Position(0, 0);
  const end = span ? position(document, span.end) : start;
  const editor = await vscode.window.showTextDocument(document, {
    viewColumn: vscode.ViewColumn.Beside,
    selection: new vscode.Range(start, start),
    preview: true,
  });
  editor.revealRange(new vscode.Range(start, end), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
  return editor;
}
