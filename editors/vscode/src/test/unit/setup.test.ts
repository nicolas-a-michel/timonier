import * as assert from "node:assert/strict";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { after, describe, it } from "node:test";

import { detectAgents, setUp } from "../../setup";

/** A `timonier` that answers `init --detect`, and records the arguments of `init --yes`. */
function fake(dir: string, initExit: number): string {
  const program = path.join(dir, "timonier");
  writeFileSync(
    program,
    `#!/bin/sh
if [ "$2" = "--detect" ]; then
  echo '{"agents":[{"key":"claude","label":"Claude Code","found":true,"why":"~/.claude exists"},{"key":"cursor","label":"Cursor","found":false,"why":null}]}'
  exit 0
fi
echo "$@" > "${dir}/args"
echo "  created AGENTS.md"
if [ ${initExit} -ne 0 ]; then echo "timonier: cannot write AGENTS.md" >&2; fi
exit ${initExit}
`,
  );
  chmodSync(program, 0o755);
  return program;
}

describe("the bridge's setup, without a terminal (T-118)", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "timonier-setup-"));
  after(() => rmSync(dir, { recursive: true, force: true }));
  const lines: string[] = [];
  const log = { info: (m: string) => lines.push(m), error: (m: string) => lines.push(`error: ${m}`) };

  it("says the agents init configures, those found and the others", async () => {
    const agents = await detectAgents(fake(dir, 0), dir);
    assert.deepEqual(
      agents.map((a) => [a.key, a.found]),
      [
        ["claude", true],
        ["cursor", false],
      ],
    );
    // A program that cannot run: no agent, not an error.
    assert.deepEqual(await detectAgents(path.join(dir, "missing"), dir), []);
  });

  it("runs init for the agents chosen, its output in the log", async () => {
    const done = await setUp(fake(dir, 0), dir, ["claude", "copilot"], log);
    assert.deepEqual(done, { ok: true, detail: null });
    assert.equal(
      readFileSync(path.join(dir, "args"), "utf8").trim(),
      `init --yes --root ${dir} --agents claude,copilot`,
    );
    assert.ok(lines.includes("init:   created AGENTS.md"), lines.join("\n"));
  });

  it("says why init failed, in its last words", async () => {
    const done = await setUp(fake(dir, 1), dir, ["claude"], log);
    assert.deepEqual(done, { ok: false, detail: "cannot write AGENTS.md" });
  });
});
