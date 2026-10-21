import * as assert from "node:assert/strict";
import { chmodSync, existsSync, statSync, mkdirSync, mkdtempSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { after, describe, it } from "node:test";

import { bundledProgram, dataDir, installCommandLine, onPath, programName, refreshCopy } from "../../commandLine";

/** File modes and symbolic links: what Linux and macOS have (T-125). */
const posix = { skip: process.platform === "win32" };

describe("the shipped timonier, on the agents' PATH (ADR 0059)", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "timonier-cli-"));
  after(() => rmSync(dir, { recursive: true, force: true }));

  const extension = path.join(dir, "extension");
  mkdirSync(path.join(extension, "bin"), { recursive: true });
  const shipped = path.join(extension, "bin", "timonier");
  writeFileSync(shipped, "#!/bin/sh\necho 0.1.0\n");
  chmodSync(shipped, 0o755);

  it("finds the program a platform's package ships, and none in a package without", posix, () => {
    assert.equal(bundledProgram(extension, "linux"), shipped);
    assert.equal(bundledProgram(path.join(dir, "nowhere"), "linux"), undefined);
    // A mode lost in the unpacking comes back.
    chmodSync(shipped, 0o644);
    assert.equal(bundledProgram(extension, "linux"), shipped);
    assert.equal(statSync(shipped).mode & 0o111, 0o111);
    assert.equal(programName("win32", "tim"), "tim.exe");
  });

  it("keeps the data folder where the daemon looks for it", () => {
    assert.equal(dataDir({ TIMONIER_DATA_DIR: "/d" }, "/home/me", "linux"), "/d");
    assert.equal(dataDir({ XDG_DATA_HOME: "/x" }, "/home/me", "linux"), path.join("/x", "timonier"));
    assert.equal(dataDir({}, "/home/me", "darwin"), path.join("/home/me", ".local", "share", "timonier"));
    assert.equal(dataDir({ LOCALAPPDATA: "/l" }, "/home/me", "win32"), path.join("/l", "timonier"));
  });

  it("links ~/.local/bin to a copy, once, and leaves what is someone else's", posix, async () => {
    const home = path.join(dir, "home");
    mkdirSync(path.join(home, ".local", "bin"), { recursive: true });
    // Someone's own `tim`, which stays.
    writeFileSync(path.join(home, ".local", "bin", "tim"), "mine");
    const machine = {
      env: { PATH: `/usr/bin:${path.join(home, ".local", "bin")}` },
      home,
      platform: "linux" as const,
      addToUserPath: () => Promise.reject(new Error("not on Linux")),
    };
    const first = await installCommandLine(shipped, machine);
    const copy = path.join(home, ".local", "share", "timonier", "bin", "timonier");
    assert.deepEqual(first.linked, [path.join(home, ".local", "bin", "timonier")]);
    assert.deepEqual(first.left, [path.join(home, ".local", "bin", "tim")]);
    assert.equal(first.notOnPath, false);
    assert.equal(readlinkSync(path.join(home, ".local", "bin", "timonier")), copy);
    assert.equal(
      await onPath("timonier", path.join(home, ".local", "bin"), "linux"),
      path.join(home, ".local", "bin", "timonier"),
    );
    const again = await installCommandLine(shipped, { ...machine, env: { PATH: "/usr/bin" } });
    assert.deepEqual(again.linked, first.linked);
    assert.equal(again.notOnPath, true, "~/.local/bin is not on this PATH");
  });

  it("replaces the copy when the extension ships another program", posix, async () => {
    const data = path.join(dir, "data");
    const copy = await refreshCopy(shipped, data, "linux");
    writeFileSync(shipped, "#!/bin/sh\necho 0.2.0, a longer answer\n");
    await refreshCopy(shipped, data, "linux");
    assert.match((await import("node:fs")).readFileSync(copy, "utf8"), /0\.2\.0/);
  });

  it("puts the copies' folder in the user's PATH on Windows", async () => {
    const home = path.join(dir, "winhome");
    const added: string[] = [];
    const installed = await installCommandLine(shipped, {
      env: { LOCALAPPDATA: path.join(home, "AppData", "Local"), Path: "C:\\Windows" },
      home,
      platform: "win32",
      addToUserPath: (folder) => {
        added.push(folder);
        return Promise.resolve();
      },
    });
    const bin = path.join(home, "AppData", "Local", "timonier", "bin");
    assert.deepEqual(added, [bin]);
    assert.deepEqual(installed.linked, [path.join(bin, "timonier.exe"), path.join(bin, "tim.exe")]);
    assert.ok(existsSync(path.join(bin, "tim.exe")));
    assert.equal(installed.notOnPath, true);
  });
});
