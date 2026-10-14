import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { contentSecurityPolicy, createNonce, escapeHtml, renderWebviewHtml } from "../../webviewHtml";

const options = {
  cspSource: "https://file+.vscode-resource.vscode-cdn.net",
  nonce: "abc123",
  scriptUri: "https://res/assets/index.js",
  styleUris: ["https://res/assets/index.css"],
  lang: "fr",
  title: "Carte Timonier",
};

describe("content security policy", () => {
  const csp = contentSecurityPolicy(options.cspSource, options.nonce);

  it("denies everything by default", () => {
    assert.match(csp, /^default-src 'none'/);
  });

  it("allows scripts only with the nonce: no origin, no inline, no eval", () => {
    const scriptSrc = csp.split("; ").find((directive) => directive.startsWith("script-src"));
    assert.equal(scriptSrc, "script-src 'nonce-abc123'");
    assert.doesNotMatch(csp, /unsafe-inline|unsafe-eval/);
  });

  it("loads styles, images and fonts from the extension only", () => {
    assert.match(csp, /style-src https:\/\/file\+/);
    assert.match(csp, /img-src https:\/\/file\+\S* data:/);
    assert.match(csp, /font-src https:\/\/file\+/);
  });
});

describe("webview html", () => {
  const html = renderWebviewHtml(options);

  it("loads the entry script as a module carrying the nonce", () => {
    assert.match(html, /<script type="module" nonce="abc123" src="https:\/\/res\/assets\/index\.js"><\/script>/);
  });

  it("links every stylesheet of the entry", () => {
    assert.match(html, /<link rel="stylesheet" href="https:\/\/res\/assets\/index\.css" \/>/);
  });

  it("passes the IDE language through <html lang>", () => {
    assert.match(html, /<html lang="fr">/);
  });

  it("escapes every interpolated value", () => {
    const attack = '"><script>alert(1)</script>';
    const hostile = renderWebviewHtml({
      cspSource: attack,
      nonce: attack,
      scriptUri: attack,
      styleUris: [attack],
      lang: attack,
      title: attack,
    });
    assert.ok(!hostile.includes(attack), "the raw value leaked into the page");
    const escaped = escapeHtml(attack);
    for (const fragment of [
      `<html lang="${escaped}">`,
      `<title>${escaped}</title>`,
      `nonce="${escaped}" src="${escaped}"`,
      `<link rel="stylesheet" href="${escaped}" />`,
      `img-src ${escaped} data:`,
    ]) {
      assert.ok(hostile.includes(fragment), `not escaped: ${fragment}`);
    }
    assert.equal(escapeHtml(`&<>"'`), "&amp;&lt;&gt;&quot;&#39;");
  });
});

describe("nonce", () => {
  it("is fresh and long enough", () => {
    const a = createNonce();
    assert.notEqual(a, createNonce());
    assert.ok(Buffer.from(a, "base64").length >= 16);
  });
});
