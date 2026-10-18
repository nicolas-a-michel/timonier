import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { backoff, DAEMON_PATH_VARIABLE, daemonProgram, parseAnnounce } from "../../launch";
import { isWebviewMessage } from "../../messages";

describe("daemon launch", () => {
  it("waits twice as long after each failure, from 1 s up to 30 s", () => {
    assert.deepEqual(
      [1, 2, 3, 4, 5, 6, 7, 20].map(backoff),
      [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000],
    );
    assert.equal(backoff(0), 1_000);
  });

  it("finds the program in the environment, then the setting, then the PATH", () => {
    assert.equal(daemonProgram({ [DAEMON_PATH_VARIABLE]: "/env/timonier" }, "/setting/timonier"), "/env/timonier");
    assert.equal(daemonProgram({}, " /setting/timonier "), "/setting/timonier");
    assert.equal(daemonProgram({}, "  "), "timonier");
    assert.equal(daemonProgram({}, undefined), "timonier");
  });

  it("reads the line of `timonier daemon run`", () => {
    assert.deepEqual(parseAnnounce('{"socket":"/run/t/a.sock","pid":42}'), { socket: "/run/t/a.sock", pid: 42 });
    assert.deepEqual(parseAnnounce('{"socket":"/run/t/a.sock","pid":null}'), { socket: "/run/t/a.sock", pid: null });
    for (const line of ["", "not json", "[]", '{"pid":1}', '{"socket":3,"pid":1}', '{"socket":"s","pid":"1"}']) {
      assert.equal(parseAnnounce(line), undefined, line);
    }
  });
});

describe("webview messages", () => {
  it("are recognised by their type and their fields", () => {
    const valid: unknown[] = [
      { type: "ready", protocol: 1 },
      { type: "subscribe", view: { expanded: ["pkg:a"] } },
      { type: "resync" },
      { type: "displayed", paths: ["src/a.ts"] },
      { type: "openCode", file: "src/a.ts", span: null },
      { type: "openCode", file: "src/a.ts", span: { start: { line: 1, column: 0 }, end: { line: 2, column: 4 } } },
      { type: "rendered", subscription: 1, revision: 0, nodes: 6 },
      { type: "pins", pins: {} },
      { type: "pins", pins: { "@role/utility": { parent: "@root", x: -40, y: 128 } } },
    ];
    for (const message of valid) assert.equal(isWebviewMessage(message), true, JSON.stringify(message));
    const invalid: unknown[] = [
      { type: "snapshot" },
      {},
      null,
      "ready",
      3,
      { type: "ready" },
      { type: "subscribe", view: { expanded: [1] } },
      { type: "displayed", paths: "src/a.ts" },
      { type: "openCode", file: 3, span: null },
      { type: "openCode", file: "src/a.ts", span: { start: { line: -1, column: 0 }, end: { line: 0, column: 0 } } },
      { type: "rendered", subscription: 1, revision: 0.5, nodes: 6 },
      { type: "pins", pins: [] },
      { type: "pins", pins: { a: { parent: 1, x: 0, y: 0 } } },
      { type: "pins", pins: { a: { parent: "@root", x: 0.5, y: 0 } } },
      {
        type: "pins",
        pins: Object.fromEntries([...Array(5001).keys()].map((i) => [`n${i}`, { parent: "@root", x: 0, y: 0 }])),
      },
    ];
    for (const message of invalid) assert.equal(isWebviewMessage(message), false, JSON.stringify(message));
  });
});
