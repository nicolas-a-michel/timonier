import * as assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { describe, it } from "node:test";

// Tests run from out/test/unit; the package root is three levels up.
const root = path.resolve(__dirname, "..", "..", "..");
const readJson = (file: string) => JSON.parse(readFileSync(path.join(root, file), "utf8")) as Record<string, unknown>;

/** Source files of the extension, tests excluded. */
function sourceFiles(dir = path.join(root, "src")): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) return name === "test" ? [] : sourceFiles(full);
    return name.endsWith(".ts") ? [full] : [];
  });
}

/** Messages passed literally to l10n.t, with any quote style. */
function runtimeMessages(): Set<string> {
  const messages = new Set<string>();
  const call = /l10n\.t\(\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\$]|\\.)*)`)/g;
  for (const file of sourceFiles()) {
    for (const match of readFileSync(file, "utf8").matchAll(call)) messages.add(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return messages;
}

describe("localisation", () => {
  it("translates every manifest string into French", () => {
    const manifest = readFileSync(path.join(root, "package.json"), "utf8");
    const keys = [...manifest.matchAll(/"%([^%"]+)%"/g)].map((m) => m[1] ?? "");
    const en = readJson("package.nls.json");
    const fr = readJson("package.nls.fr.json");
    assert.ok(keys.length > 0);
    for (const key of keys) {
      assert.equal(typeof en[key], "string", `package.nls.json lacks ${key}`);
      assert.equal(typeof fr[key], "string", `package.nls.fr.json lacks ${key}`);
    }
  });

  it("finds the messages of nested source files", () => {
    assert.ok(sourceFiles().some((file) => file.endsWith("mapPanel.ts")));
    assert.ok(runtimeMessages().has("Timonier map"));
  });

  it("translates every runtime message into French, and nothing else", () => {
    const fr = readJson("l10n/bundle.l10n.fr.json");
    const used = runtimeMessages();
    assert.ok(used.size > 0);
    assert.deepEqual([...used].sort(), Object.keys(fr).sort());
  });
});
