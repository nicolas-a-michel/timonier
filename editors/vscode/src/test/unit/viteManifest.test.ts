import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { webviewEntry } from "../../viteManifest";

describe("webview manifest", () => {
  it("returns the entry script and its stylesheets", () => {
    const manifest = JSON.stringify({
      "index.html": { file: "assets/index-1.js", css: ["assets/index-2.css"], isEntry: true, src: "index.html" },
    });
    assert.deepEqual(webviewEntry(manifest), { script: "assets/index-1.js", styles: ["assets/index-2.css"] });
  });

  it("accepts an entry without stylesheets", () => {
    const manifest = JSON.stringify({ "index.html": { file: "a.js", isEntry: true } });
    assert.deepEqual(webviewEntry(manifest).styles, []);
  });

  it("refuses a manifest without the entry", () => {
    assert.throws(
      () => webviewEntry(JSON.stringify({ "other.html": { file: "a.js", isEntry: true } })),
      /no entry "index\.html"/,
    );
    assert.throws(() => webviewEntry(JSON.stringify({ "index.html": { file: "a.js" } })), /no entry/);
  });
});
