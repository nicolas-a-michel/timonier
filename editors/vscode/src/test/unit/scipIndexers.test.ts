import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { missingIndexers, SCIP_INSTALL_ARGS } from "../../scipIndexers";

describe("« Install the SCIP indexers »", () => {
  it("shows the commands of the missing indexers once, and says what is left by hand", () => {
    const json = JSON.stringify([
      {
        language: "rust",
        indexer: "rust-analyzer",
        found: false,
        install: "rustup component add rust-analyzer",
        note: null,
      },
      {
        language: "typescript",
        indexer: "scip-typescript",
        found: false,
        install: "npm i -g scip-typescript",
        note: null,
      },
      {
        language: "javascript",
        indexer: "scip-typescript",
        found: false,
        install: "npm i -g scip-typescript",
        note: null,
      },
      { language: "java", indexer: "scip-java", found: false, install: null, note: "install coursier" },
      { language: "go", indexer: "scip-go", found: true, install: null, note: null },
    ]);
    assert.deepEqual(missingIndexers(json), {
      commands: ["rustup component add rust-analyzer", "npm i -g scip-typescript"],
      byHand: ["java, scip-java: install coursier"],
    });
    assert.deepEqual(missingIndexers("[]"), { commands: [], byHand: [] });
    assert.equal(missingIndexers("not json"), null);
  });

  it("installs what it showed, approved, then indexes", () => {
    assert.deepEqual(SCIP_INSTALL_ARGS, ["scip", "--install", "--approve"]);
  });
});
