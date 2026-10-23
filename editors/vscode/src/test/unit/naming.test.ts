import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { askRename, ROLES, type Ask } from "../../naming";

const answers = (
  name: string | undefined,
  role: ReturnType<Ask["role"]> extends Thenable<infer R> ? R : never,
): Ask => ({
  name: () => Promise.resolve(name),
  role: () => Promise.resolve(role),
});

describe("renaming a box", () => {
  const box = { target: "pkg:cargo/store", name: "store", role: "utility" as const };

  it("gives the daemon the name and the role chosen", async () => {
    assert.deepEqual(await askRename(box, answers(" Storage ", "persistence")), {
      id: "pkg:cargo/store",
      name: "Storage",
      role: "persistence",
    });
  });

  it("gives the box back with an empty name and the computed role", async () => {
    assert.deepEqual(await askRename(box, answers("", null)), { id: "pkg:cargo/store", name: null, role: null });
  });

  it("does nothing when a dialog is cancelled", async () => {
    assert.equal(await askRename(box, answers(undefined, "persistence")), undefined);
    assert.equal(await askRename(box, answers("Storage", undefined)), undefined);
  });

  it("offers every role of the map", () => {
    assert.equal(ROLES.length, 8);
  });
});
