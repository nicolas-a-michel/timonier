import { randomBytes } from "node:crypto";

export interface WebviewHtmlOptions {
  /** `webview.cspSource`: the origin allowed to serve the extension's resources. */
  cspSource: string;
  nonce: string;
  scriptUri: string;
  styleUris: readonly string[];
  /** BCP 47 tag of the IDE's display language; the webview picks its language from it. */
  lang: string;
  title: string;
}

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ESCAPES[c] ?? c);
}

/** A fresh nonce for each rendering: only scripts carrying it may run. */
export function createNonce(): string {
  return randomBytes(16).toString("base64");
}

/**
 * Strict policy: nothing by default; styles, images and fonts only from the extension;
 * scripts only with the nonce. The integration tests prove that the bundle runs under it.
 */
export function contentSecurityPolicy(cspSource: string, nonce: string): string {
  return [
    "default-src 'none'",
    `img-src ${cspSource} data:`,
    `font-src ${cspSource}`,
    `style-src ${cspSource}`,
    `script-src 'nonce-${nonce}'`,
  ].join("; ");
}

export function renderWebviewHtml(options: WebviewHtmlOptions): string {
  const csp = contentSecurityPolicy(options.cspSource, options.nonce);
  const styles = options.styleUris.map((uri) => `    <link rel="stylesheet" href="${escapeHtml(uri)}" />`);
  return [
    "<!doctype html>",
    `<html lang="${escapeHtml(options.lang)}">`,
    "  <head>",
    '    <meta charset="UTF-8" />',
    `    <meta http-equiv="Content-Security-Policy" content="${escapeHtml(csp)}" />`,
    '    <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    ...styles,
    `    <title>${escapeHtml(options.title)}</title>`,
    "  </head>",
    "  <body>",
    '    <div id="root"></div>',
    `    <script type="module" nonce="${escapeHtml(options.nonce)}" src="${escapeHtml(options.scriptUri)}"></script>`,
    "  </body>",
    "</html>",
    "",
  ].join("\n");
}
