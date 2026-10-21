// Renders media/icon.svg to media/icon.png, the Marketplace icon (256 px; vsce refuses SVG).
// Run from the repository root after `npm ci`: node editors/vscode/scripts/icon.mjs
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "@playwright/test";

const media = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "media");
const svg = readFileSync(path.join(media, "icon.svg"), "utf8");
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 256, height: 256 } });
await page.setContent(`<html><body style="margin:0;background:transparent">${svg}</body></html>`);
await page.locator("svg").screenshot({ path: path.join(media, "icon.png"), omitBackground: true });
await browser.close();
