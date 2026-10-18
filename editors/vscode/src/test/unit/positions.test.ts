import * as assert from "node:assert/strict";
import path from "node:path";
import { describe, it } from "node:test";

import { relativeInside, resolveInside, utf16ToUtf8, utf8ToUtf16 } from "../../positions";

describe("columns", () => {
  // "é" is 2 bytes and 1 unit; "😀" is 4 bytes and 2 units.
  const line = "let é = f😀(x);";

  it("are the same in ASCII", () => {
    assert.equal(utf8ToUtf16("fn main() {}", 3), 3);
    assert.equal(utf16ToUtf8("fn main() {}", 3), 3);
  });

  it("convert across multi-byte characters, both ways", () => {
    // `=`: byte 7, unit 6.
    assert.equal(utf8ToUtf16(line, 7), 6);
    assert.equal(utf16ToUtf8(line, 6), 7);
    // `(`: after the emoji (units 9 and 10), byte 14, unit 11.
    assert.equal(line.indexOf("("), 11);
    assert.equal(utf8ToUtf16(line, 14), 11);
    assert.equal(utf16ToUtf8(line, 11), 14);
    // Every character boundary goes there and back.
    for (const units of [0, 4, 6, 9, 11, 14]) {
      assert.equal(utf8ToUtf16(line, utf16ToUtf8(line, units)), units, `unit ${units}`);
    }
  });

  it("stop at the end of the line", () => {
    assert.equal(utf8ToUtf16("abc", 10), 3);
    assert.equal(utf16ToUtf8("abé", 10), 4);
  });
});

describe("paths", () => {
  const root = path.join(path.sep, "work", "project");

  it("are relative to the root, with slashes", () => {
    assert.equal(relativeInside(root, path.join(root, "src", "a.ts")), "src/a.ts");
  });

  it("outside the root are null", () => {
    assert.equal(relativeInside(root, path.join(path.sep, "work", "other", "a.ts")), null);
    assert.equal(relativeInside(root, path.join(path.sep, "usr", "lib", "node.d.ts")), null);
    assert.equal(relativeInside(root, root), null);
  });
});

describe("a file named by the webview", () => {
  const root = path.join(path.sep, "work", "project");

  it("resolves inside the root", () => {
    assert.equal(resolveInside(root, "src/a.ts"), path.join(root, "src", "a.ts"));
    assert.equal(resolveInside(root, "..notes/a.md"), path.join(root, "..notes", "a.md"));
  });

  it("never outside it", () => {
    for (const file of ["../other/a.ts", "src/../../a.ts", "/etc/passwd", "", "."]) {
      assert.equal(resolveInside(root, file), null, file);
    }
  });
});
