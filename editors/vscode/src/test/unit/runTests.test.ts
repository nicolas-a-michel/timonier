import * as assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { after, describe, it } from "node:test";

import { recordedCommands, TEST_ARGS } from "../../runTests";

describe("the Performances panel's « Run the tests »", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "timonier-run-tests-"));
  after(() => rmSync(dir, { recursive: true, force: true }));

  it("shows the commands the project records, or says it is not set up", () => {
    assert.equal(recordedCommands(path.join(dir, "nothing")), null);
    mkdirSync(path.join(dir, ".timonier"));
    writeFileSync(
      path.join(dir, ".timonier", "tests.json"),
      JSON.stringify({
        version: 1,
        runners: [
          { language: "rust", runner: "cargo-nextest", command: "cargo nextest run --profile timonier" },
          { language: "typescript", runner: "jest", command: null },
        ],
      }),
    );
    assert.deepEqual(recordedCommands(dir), ["cargo nextest run --profile timonier"]);
    writeFileSync(path.join(dir, ".timonier", "tests.json"), "{ not json");
    assert.equal(recordedCommands(dir), null);
  });

  it("approves what it showed and installs what is missing", () => {
    assert.deepEqual(TEST_ARGS, ["test", "--approve", "--install"]);
  });
});
