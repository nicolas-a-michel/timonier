import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { ResponseError } from "vscode-jsonrpc/node";

import { isSource, onSavedContent, sameText, UnsavedSources, type DocumentState } from "../../content";
import { ERRORS } from "../../protocol/generated";

describe("a document against the file on disk", () => {
  it("is the same text, whatever its line ends", () => {
    assert.ok(sameText("a\nb\n", "a\nb\n"));
    assert.ok(sameText("a\r\nb\r\n", "a\r\nb\r\n"));
    // VSCode gives a document of mixed line ends the most frequent one.
    assert.ok(sameText("a\r\nb\nc\r\n", "a\r\nb\r\nc\r\n"));
  });

  it("differs when an edit is not saved, or a change not read again", () => {
    assert.ok(!sameText("helper();\n", "area(1);\n"));
    assert.ok(!sameText("a\n", "a\n\n"));
    // VSCode ends a line at a lone `\r`, the daemon does not: lines would not match.
    assert.ok(!sameText("a\rb\n", "a\nb\n"));
  });
});

describe("source files", () => {
  it("are those the daemon's packs read, once known; before, every file", () => {
    const packs = new Set(["rs", "ts", "tsx"]);
    assert.ok(isSource("src/main.ts", packs));
    assert.ok(isSource("types/index.d.ts", packs));
    assert.ok(isSource("crates/a/src/lib.rs", packs));
    assert.ok(!isSource("PLAN.md", packs));
    assert.ok(!isSource("docs/adr/0022.md", packs));
    assert.ok(!isSource("Makefile", packs));
    assert.ok(!isSource("src/.ts", packs));
    assert.ok(isSource("PLAN.md", undefined));
  });
});

describe("the unsaved source files", () => {
  const packs = ["rs", "ts"];

  it("count every file until `initialize` says which are sources, then only those", () => {
    const open = [
      { file: "NOTES.md", dirty: true },
      { file: "src/main.ts", dirty: false },
    ];
    const sources = new UnsavedSources(open);
    assert.equal(sources.state().unsaved, "NOTES.md");
    // The note, unsaved since before the connection, no longer counts (review n° 7, E7).
    sources.setSources(packs, open);
    assert.equal(sources.state().unsaved, undefined);
    assert.ok(sources.settled);
    assert.ok(!sources.changed({ file: "NOTES.md", dirty: true }, true), "a note changes no answer");
    assert.equal(sources.state().changes, 0);
    assert.ok(sources.changed({ file: "src/main.ts", dirty: true }, true));
    assert.deepEqual(sources.state(), { unsaved: "src/main.ts", changes: 1 });
    assert.ok(!sources.settled);
  });

  it("keep counting every file when the daemon does not say (review n° 7, m7-3)", () => {
    const sources = new UnsavedSources([]);
    sources.setSources(undefined, [{ file: "NOTES.md", dirty: true }]);
    assert.equal(sources.state().unsaved, "NOTES.md");
    sources.setSources(["rs", 3], [{ file: "NOTES.md", dirty: true }]);
    assert.equal(sources.state().unsaved, "NOTES.md", "not a list of extensions");
  });

  it("let go of a file saved, reverted, or closed without saving (review n° 7, E14)", () => {
    const sources = new UnsavedSources([]);
    sources.setSources(packs, []);
    sources.changed({ file: "src/a.ts", dirty: true }, true);
    sources.changed({ file: "src/b.rs", dirty: true }, true);
    // Saved: its dirty state only.
    assert.ok(sources.changed({ file: "src/a.ts", dirty: false }, false));
    assert.equal(sources.state().unsaved, "src/b.rs");
    assert.ok(sources.closed("src/b.rs"));
    assert.ok(sources.settled);
    assert.equal(sources.state().changes, 2);
    assert.ok(!sources.closed("NOTES.md"));
  });
});

/** A workspace whose state the test drives, and the questions asked. */
function workspace(initial: Partial<DocumentState> = {}) {
  const state = { unsaved: undefined as string | undefined, changes: 0, ...initial };
  return {
    state,
    get: (): DocumentState => ({ ...state }),
  };
}

const onDisk = (document: string) => Promise.resolve(document !== "stale");

function refused(error: unknown): boolean {
  return error instanceof ResponseError && error.code === ERRORS.CONTENT_MODIFIED;
}

describe("the relay's answers, on saved content only (ADR 0022)", () => {
  it("answers when nothing differs from the disk", async () => {
    const { get } = workspace();
    const answer = await onSavedContent(
      get,
      (documents) => {
        documents.set("src/a.ts", Promise.resolve("fresh"));
        return Promise.resolve(42);
      },
      onDisk,
    );
    assert.equal(answer, 42);
  });

  it("refuses at once while a source file is unsaved, without asking the servers", async () => {
    const { get } = workspace({ unsaved: "src/main.ts" });
    let asked = false;
    await assert.rejects(
      onSavedContent(
        get,
        () => {
          asked = true;
          return Promise.resolve(0);
        },
        onDisk,
      ),
      refused,
    );
    assert.ok(!asked);
  });

  it("refuses when a source file changed during the question, even saved again", async () => {
    const { state, get } = workspace();
    await assert.rejects(
      onSavedContent(
        get,
        () => {
          // Typed, then undone: clean again, but the servers answered in between.
          state.changes += 2;
          return Promise.resolve(0);
        },
        onDisk,
      ),
      refused,
    );
  });

  it("refuses when a source file has unsaved changes at the end", async () => {
    const { state, get } = workspace();
    await assert.rejects(
      onSavedContent(
        get,
        () => {
          state.unsaved = "src/util.ts";
          return Promise.resolve(0);
        },
        onDisk,
      ),
      refused,
    );
  });

  it("refuses when a document it read is not the file on disk", async () => {
    const { get } = workspace();
    await assert.rejects(
      onSavedContent(
        get,
        (documents) => {
          documents.set("src/a.ts", Promise.resolve("fresh"));
          documents.set("src/b.ts", Promise.resolve("stale"));
          return Promise.resolve(0);
        },
        onDisk,
      ),
      (error: unknown) => refused(error) && (error as Error).message.includes("src/b.ts"),
    );
  });
});
