/**
 * The whiteboard as the extension ships it (ADR 0037): its production build, in the page the
 * extension writes, under its CSP, with a host that answers as the extension does and keeps the
 * board in the session in place of `.timonier/intent.json`. For the production tests and the
 * bench.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

import type { Page } from "@playwright/test";

import { webviewEntry } from "../../../editors/vscode/src/viteManifest";
import { renderWebviewHtml } from "../../../editors/vscode/src/webviewHtml";
import { PRODUCTION } from "./build";

/** The origin the build is served from, as `webview.cspSource` is in VSCode. */
const ORIGIN = "https://webview.timonier.test";

const TYPES: Record<string, string> = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml" };

/** Opens the page; `kept`: what VSCode gives back to `getState`. */
/** Opens the page; `preset`: the board the host gives first, as `intent.json` would hold it. */
export async function open(page: Page, { locale = "en", preset }: { locale?: string; preset?: unknown } = {}) {
  const entry = webviewEntry(readFileSync(path.join(PRODUCTION, ".vite", "manifest.json"), "utf8"), "whiteboard.html");
  const html = renderWebviewHtml({
    cspSource: ORIGIN,
    nonce: "dGVzdC1ub25jZS0xMjM0NQ==",
    scriptUri: `${ORIGIN}/${entry.script}`,
    styleUris: entry.styles.map((file) => `${ORIGIN}/${file}`),
    lang: locale,
    title: "Timonier whiteboard",
  });
  await page.route(`${ORIGIN}/**`, async (route) => {
    const file = new URL(route.request().url()).pathname.slice(1);
    if (file === "") return route.fulfill({ contentType: "text/html", body: html });
    return route.fulfill({
      contentType: TYPES[path.extname(file)] ?? "application/octet-stream",
      body: readFileSync(path.join(PRODUCTION, file)),
    });
  });
  // VSCode's API, as the webview sees it; and what the CSP refuses, if anything.
  await page.addInitScript(
    (first: string) => {
      const w = window as unknown as Record<string, unknown>;
      w.sent = [];
      w.violations = [];
      document.addEventListener("securitypolicyviolation", (event) =>
        (w.violations as string[]).push(`${event.violatedDirective} ${event.blockedURI}`),
      );
      // As the extension does (ADR 0039): it answers « ready » with the board the file holds, and
      // keeps each board the page sends; the session stands for `.timonier/intent.json`.
      w.acquireVsCodeApi = () => ({
        postMessage: (message: { type: string; intent?: unknown }) => {
          (w.sent as unknown[]).push(message);
          if (message.type === "ready") {
            const intent = JSON.parse(sessionStorage.getItem("intent") ?? first) as unknown;
            setTimeout(() => window.postMessage({ type: "intent", intent, problem: null }, "*"), 0);
          } else if (message.type === "intent") {
            sessionStorage.setItem("intent", JSON.stringify(message.intent));
          }
        },
      });
    },
    JSON.stringify(preset ?? null),
  );
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await page.goto(`${ORIGIN}/`);
  await page.waitForFunction(() =>
    ((window as unknown as { sent: { type: string }[] }).sent ?? []).some((m) => m.type === "ready"),
  );
  // The board comes from the host once the page said it is ready.
  await page.locator(".tim-whiteboard").waitFor();
  return { errors };
}
