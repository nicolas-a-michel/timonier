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
    viewsContainers: {
      activitybar: { id: string; title: string; icon: string }[];
      secondarySidebar?: { id: string; title: string; icon: string }[];
    };
    views: Record<string, { id: string; name: string; icon?: string; type?: string }[]>;
    viewsWelcome?: { view: string; contents: string }[];
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

  it("holds the overview on the left, the tracking alone on the right (ADR 0031)", () => {
    const left = contributes.views.timonier ?? [];
    const right = contributes.views["timonier-journal"] ?? [];
    assert.deepEqual(
      left.map((v) => v.id),
      ["timonier.home"],
    );
    assert.deepEqual(
      right.map((v) => v.id),
      ["timonier.tracking"],
    );
    assert.equal(right[0]?.type, "webview");
    assert.deepEqual(
      contributes.viewsContainers.secondarySidebar?.map((c) => c.id),
      ["timonier-journal"],
    );
    for (const view of [...left, ...right]) assert.ok(view.icon && existsSync(path.join(root, view.icon)), view.id);
    // The view's name and the container's title, in both languages.
    const keys = [right[0]?.name ?? "", contributes.viewsContainers.secondarySidebar?.[0]?.title ?? ""];
    for (const file of ["package.nls.json", "package.nls.fr.json"]) {
      for (const key of keys) assert.ok(nls(file)[key.replace(/^%|%$/g, "")], `${key} in ${file}`);
    }
  });

  it("packages the three pages of the webview: the map, the panels and the whiteboard (ADR 0031, 0037)", () => {
    const kept = readFileSync(path.join(root, ".vscodeignore"), "utf8").split("\n");
    for (const page of ["dist/webview", "dist/webview/tracking", "dist/webview/whiteboard"]) {
      for (const file of ["assets/**", ".vite/manifest.json", "THIRD-PARTY-NOTICES.md"]) {
        assert.ok(kept.includes(`!${page}/${file}`), `${page}/${file} is left out of the .vsix`);
      }
    }
  });

  it("makes the overview the bridge, a page of the webview (ADR 0032)", () => {
    const home = contributes.views.timonier?.find((v) => v.id === "timonier.home");
    assert.equal(home?.type, "webview");
    // A page has no welcome content: its own « Open Map » button replaces it.
    assert.equal(contributes.viewsWelcome, undefined);
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
