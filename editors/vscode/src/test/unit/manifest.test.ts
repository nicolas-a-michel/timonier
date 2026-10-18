import * as assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, it } from "node:test";

// Tests run from out/test/unit; the package root is three levels up.
const root = path.resolve(__dirname, "..", "..", "..");

interface Manifest {
  capabilities?: { untrustedWorkspaces?: { supported: boolean | "limited" } };
  contributes: {
    configuration?: { properties: Record<string, { scope?: string }> };
    viewsContainers: { activitybar: { id: string; title: string; icon: string }[] };
    views: Record<string, { id: string; name: string; icon?: string }[]>;
    viewsWelcome: { view: string; contents: string }[];
    commands: { command: string; icon?: string }[];
    menus: { "view/title": { command: string; when: string; group?: string }[] };
  };
}

const manifest = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8")) as Manifest;
const { contributes } = manifest;
const nls = (file: string) => JSON.parse(readFileSync(path.join(root, file), "utf8")) as Record<string, string>;

describe("activity bar entry", () => {
  const container = contributes.viewsContainers.activitybar.find((c) => c.id === "timonier");

  it("contributes a Timonier container to the activity bar", () => {
    assert.ok(container);
    assert.equal(container.title, "Timonier");
  });

  it("uses a 24×24, single-colour SVG icon, as VSCode asks", () => {
    assert.ok(container);
    const file = path.join(root, container.icon);
    assert.ok(existsSync(file), container.icon);
    const svg = readFileSync(file, "utf8");
    assert.match(svg, /viewBox="0 0 24 24"/);
    const colours = [...svg.matchAll(/(?:fill|stroke|stop-color)="([^"]+)"/g)].map((m) => m[1]);
    assert.ok(
      colours.every((c) => c === "currentColor" || c === "none"),
      colours.join(", "),
    );
  });

  it("holds a single overview view, with an icon in case it is moved", () => {
    const views = contributes.views.timonier ?? [];
    assert.deepEqual(
      views.map((v) => v.id),
      ["timonier.home"],
    );
    assert.ok(views[0]?.icon && existsSync(path.join(root, views[0].icon)));
  });

  it("offers « Open Map » as the overview's welcome button, in both languages", () => {
    const welcome = contributes.viewsWelcome.find((w) => w.view === "timonier.home");
    assert.ok(welcome);
    const key = welcome.contents.replace(/^%|%$/g, "");
    for (const file of ["package.nls.json", "package.nls.fr.json"]) {
      // A command link alone on its line is rendered as a button.
      assert.match(nls(file)[key] ?? "", /^\[[^\]]+\]\(command:timonier\.openMap\)$/m, file);
    }
  });

  it("puts the map button in the overview's title bar", () => {
    const action = contributes.menus["view/title"].find((m) => m.command === "timonier.openMap");
    assert.ok(action);
    assert.equal(action.when, "view == timonier.home");
    assert.equal(action.group, "navigation");
    const command = contributes.commands.find((c) => c.command === "timonier.openMap");
    assert.match(command?.icon ?? "", /^\$\([a-z-]+\)$/);
  });
});

describe("the program the extension runs", () => {
  it("is chosen in the user's settings only, never by the folder it serves", () => {
    // A folder's .vscode/settings.json could otherwise name a program of its own.
    assert.equal(contributes.configuration?.properties["timonier.daemon.path"]?.scope, "machine");
  });

  it("is not run in a folder that is not trusted", () => {
    assert.equal(manifest.capabilities?.untrustedWorkspaces?.supported, false);
  });
});
