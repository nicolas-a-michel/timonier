// Renders media/icon.svg to the project's images (ADR 0059): media/icon.png, the Marketplace
// icon (256 px; vsce refuses SVG); docs/images/logo.png, the READMEs' logo (192 px, shown at 96);
// docs/images/social-preview.png, the repository's preview on GitHub (1280 × 640, uploaded by
// hand in the repository's settings). Run from the repository root after `npm ci`:
// node editors/vscode/scripts/icon.mjs
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "@playwright/test";

const here = path.dirname(fileURLToPath(import.meta.url));
const media = path.join(here, "..", "media");
const images = path.join(here, "..", "..", "..", "docs", "images");
const svg = readFileSync(path.join(media, "icon.svg"), "utf8");
const sized = (size) => svg.replace('width="256" height="256"', `width="${size}" height="${size}"`);

const browser = await chromium.launch();
const shoot = async (width, height, html, file) => {
  const page = await browser.newPage({ viewport: { width, height } });
  await page.setContent(html);
  await page.locator("#shot").screenshot({ path: file, omitBackground: true });
  await page.close();
};
const alone = (size) =>
  `<html><body style="margin:0;background:transparent"><div id="shot" style="width:${size}px;height:${size}px">${sized(size)}</div></body></html>`;

await shoot(256, 256, alone(256), path.join(media, "icon.png"));
await shoot(192, 192, alone(192), path.join(images, "logo.png"));
// The preview: the wheel, the name and what it does, on the icon's sea blue.
await shoot(
  1280,
  640,
  `<html><body style="margin:0">
    <div id="shot" style="width:1280px;height:640px;background:#1d4458;display:flex;align-items:center;gap:72px;padding:0 120px;box-sizing:border-box;font-family:'Segoe UI',Ubuntu,'Helvetica Neue',Arial,sans-serif;color:#e8f1f5">
      <div style="flex:none">${sized(280)}</div>
      <div>
        <div style="font-size:112px;font-weight:600;letter-spacing:-1px;line-height:1">Timonier</div>
        <div style="font-size:38px;line-height:1.35;margin-top:28px;max-width:640px;opacity:.92">Supervise your AI coding agents without reading every line they write.</div>
      </div>
    </div>
  </body></html>`,
  path.join(images, "social-preview.png"),
);
await browser.close();
