import * as assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import path from "node:path";

import * as vscode from "vscode";

import type { TimonierApi } from "../../extension";
import { METHODS, type StatusResult, type ViewSnapshot } from "../../protocol/generated";
import type { Rendered } from "../../viewRelay";

const EXTENSION_ID = "nicolas-a-michel.timonier";

async function api(): Promise<TimonierApi> {
  const extension = vscode.extensions.getExtension<TimonierApi>(EXTENSION_ID);
  assert.ok(extension, `${EXTENSION_ID} is not installed`);
  return extension.activate();
}

async function waitFor(what: string, condition: () => boolean | Promise<boolean>, timeoutMs = 20_000): Promise<void> {
  const start = Date.now();
  while (!(await condition())) {
    if (Date.now() - start > timeoutMs) throw new Error(`${what}: not within ${timeoutMs} ms`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/** The project of `.vscode-test.mjs`, which the daemon indexes. */
function fixture(): string {
  const project = process.env.TIMONIER_FIXTURE;
  assert.ok(project, "TIMONIER_FIXTURE is not set: run the tests through .vscode-test.mjs");
  return project;
}

describe("Timonier daemon (ADR 0019)", () => {
  let timonier: TimonierApi;

  before(async () => {
    timonier = await api();
  });

  after(async () => {
    // Leave no daemon behind.
    await timonier.stopDaemon();
  });

  it("launches the project's daemon and connects to it", async () => {
    await waitFor("connected", () => timonier.daemonStatus().state === "connected");
    assert.ok(timonier.daemonPid(), "the daemon's process is not known");
  });

  it("shows the daemon's view in the map", async () => {
    await vscode.commands.executeCommand("timonier.openMap");
    await waitFor("the map shows boxes", () => (timonier.mapRendered()?.nodes ?? 0) > 0);
  });

  it("follows a change of the project through deltas", async () => {
    await waitFor("the map shows boxes", () => (timonier.mapRendered()?.nodes ?? 0) > 0);
    const before = timonier.mapRendered();
    assert.ok(before);
    // A new file: the package at the top of the map has one more child.
    writeFileSync(path.join(fixture(), "src", "b.ts"), "export function g(): void {}\n");
    // A delta on what the map shows. A page reloaded meanwhile (a busy machine) subscribes
    // again: then a delta of its new subscription.
    await waitFor("a delta applied", () => {
      const now = timonier.mapRendered();
      return !!now && now.revision > (now.subscription === before.subscription ? before.revision : 0);
    });
  });

  it("launches a killed daemon again, and the map gets the whole view", async () => {
    const pid = timonier.daemonPid();
    assert.ok(pid);
    const renders: Rendered[] = [];
    const counting = timonier.onMapRender((rendered) => renders.push(rendered));
    try {
      process.kill(pid, "SIGKILL");
      await waitFor("lost", () => timonier.daemonStatus().state === "restarting", 10_000);
      await waitFor(
        "a new daemon",
        () => timonier.daemonStatus().state === "connected" && timonier.daemonPid() !== pid,
      );
      await waitFor("the map shows boxes again", () => renders.some((r) => r.nodes > 0));
      // First a new subscription's snapshot, not a delta of the old one.
      assert.equal(renders[0]?.revision, 0);
    } finally {
      counting.dispose();
    }
  });

  it("relays the IDE's call hierarchy to the daemon (PLAN.md 4.5)", async function () {
    this.timeout(120_000);
    const main = vscode.Uri.file(path.join(fixture(), "src", "main.ts"));
    const document = await vscode.workspace.openTextDocument(main);
    // `run`, in « export function run(): void { »: VSCode's TypeScript server answers once its
    // project is loaded.
    const run = new vscode.Position(2, 16);
    await waitFor(
      "the TypeScript server",
      async () =>
        (
          (await vscode.commands.executeCommand<vscode.CallHierarchyItem[] | undefined>(
            "vscode.prepareCallHierarchy",
            main,
            run,
          )) ?? []
        ).length > 0,
      60_000,
    );
    // Shown in an editor: `editor/opened`, then the daemon asks for its calls.
    await vscode.window.showTextDocument(document);
    const expanded = ["pkg:npm/fixture", "file:src/main.ts", "file:src/util.ts"];
    await waitFor(
      "a call verified by the IDE",
      async () => {
        const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, { view: { expanded } });
        await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
        return view.edges.some(
          (edge) =>
            edge.best === "lsp" && !edge.approximateOnly && edge.src.endsWith("#run") && edge.dst.endsWith("#helper"),
        );
      },
      60_000,
    );
  });

  it("verifies a type usage, and names the users of a changed signature (PLAN.md 4.5, ADR 0022)", async function () {
    this.timeout(120_000);
    // Shown in an editor, area.ts is refined: its use of the type `Shape` is asked about.
    const area = vscode.Uri.file(path.join(fixture(), "src", "area.ts"));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(area));
    const expanded = ["pkg:npm/fixture"];
    await waitFor(
      "a type usage verified by the IDE",
      async () => {
        const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, { view: { expanded } });
        await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
        return view.edges.some(
          (edge) =>
            edge.kind === "references" &&
            edge.best === "lsp" &&
            edge.src === "file:src/area.ts" &&
            edge.dst === "file:src/shape.ts",
        );
      },
      60_000,
    );
    const before = (await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {})).lspAnswers;
    assert.ok(before.definitions > 0, "no definitions answered");
    // `helper` changes signature: VSCode names its users, main.ts among them, verified above
    // (a lookup « named », not one by name), and main.ts is verified again: its call hierarchy
    // is asked once more.
    const lookups = (await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {})).lookups;
    writeFileSync(path.join(fixture(), "src", "util.ts"), "export function helper(times = 1): void {}\n");
    await waitFor(
      "the users of helper named by the IDE, and main.ts verified again",
      async () => {
        const status = await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
        return (
          status.lookups.named > lookups.named &&
          status.lspAnswers.references > before.references &&
          status.lspAnswers.outgoingCalls > before.outgoingCalls
        );
      },
      60_000,
    );
    assert.equal((await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {})).lookups.byName, lookups.byName);
  });

  it("answers nothing about unsaved changes, and is asked again once they are reverted (ADR 0022)", async function () {
    this.timeout(120_000);
    const main = vscode.Uri.file(path.join(fixture(), "src", "main.ts"));
    const document = await vscode.workspace.openTextDocument(main);
    await vscode.window.showTextDocument(document);
    const calls = async () => {
      const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
        view: { expanded: ["pkg:npm/fixture", "file:src/main.ts", "file:src/util.ts", "file:src/area.ts"] },
      });
      await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
      return view.edges.filter((edge) => edge.kind === "calls" && edge.best === "lsp" && edge.src.endsWith("#run"));
    };
    const toHelper = (edges: Awaited<ReturnType<typeof calls>>) =>
      edges.length === 1 && edges[0]?.dst.endsWith("#helper") === true;
    await waitFor("run → helper verified", async () => toHelper(await calls()), 60_000);
    // Unsaved, `run` calls `area` instead: the review n° 5 probe (I-α).
    const edit = new vscode.WorkspaceEdit();
    edit.replace(
      main,
      new vscode.Range(0, 0, 3, 11),
      'import { area } from "./area";\n\nexport function run(): void {\n  area(1);',
    );
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(document.isDirty);
    // `helper` changes signature on disk: the daemon looks up its users, main.ts among them. The
    // extension refuses while main.ts is unsaved, and the daemon holds the lookup.
    const before = await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    writeFileSync(path.join(fixture(), "src", "util.ts"), "export function helper(times = 2): void {}\n");
    await waitFor(
      "the lookup held",
      async () => (await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {})).refinementsHeld > 0,
      60_000,
    );
    const held = await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    assert.equal(held.lspAnswers.references, before.lspAnswers.references, "answered about an unsaved buffer");
    assert.ok(toHelper(await calls()), "the unsaved call was recorded");
    // Reverted: `editor/saved`, and what was held is asked again, about the disk's content.
    await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand("workbench.action.files.revert");
    assert.ok(!document.isDirty);
    await waitFor(
      "the held work asked again and answered",
      async () => {
        const status = await timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
        return (
          status.refinementsHeld === 0 &&
          status.lookups.named > before.lookups.named &&
          status.lspAnswers.outgoingCalls > before.lspAnswers.outgoingCalls
        );
      },
      60_000,
    );
    assert.ok(toHelper(await calls()), JSON.stringify(await calls()));
  });

  it("lets the held work go once an unsaved document is closed without saving (review n° 8, X3)", async function () {
    this.timeout(120_000);
    const status = () => timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    const main = vscode.Uri.file(path.join(fixture(), "src", "main.ts"));
    const document = await vscode.workspace.openTextDocument(main);
    await vscode.window.showTextDocument(document);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(main, new vscode.Position(0, 0), "// Unsaved.\n");
    assert.ok(await vscode.workspace.applyEdit(edit));
    const before = await status();
    // `helper` changes signature: its lookup is held while main.ts is unsaved.
    writeFileSync(path.join(fixture(), "src", "util.ts"), "export function helper(count = 2): void {}\n");
    await waitFor("the lookup held", async () => (await status()).refinementsHeld > 0, 60_000);
    await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    await waitFor(
      "the held lookup answered",
      async () => {
        const now = await status();
        return now.refinementsHeld === 0 && now.lookups.named > before.lookups.named;
      },
      60_000,
    );
  });

  it("verifies a file whose calls go into a dependency under the root (review n° 6, I6-1)", async function () {
    this.timeout(120_000);
    const status = () => timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    const before = await status();
    // `usePkg` calls `shout`, declared in node_modules/lib, which the daemon does not index.
    const usePkg = vscode.Uri.file(path.join(fixture(), "src", "usePkg.ts"));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(usePkg));
    await waitFor(
      "usePkg.ts asked about",
      async () => {
        const now = await status();
        return now.lspAnswers.outgoingCalls > before.lspAnswers.outgoingCalls && now.refinementsWaiting === 0;
      },
      60_000,
    );
    await new Promise((resolve) => setTimeout(resolve, 500));
    const after = await status();
    assert.equal(after.refinementsHeld, 0, "held for a file the daemon does not index");
    // Verified: its call to `helper`, in the repository, comes from VSCode.
    await waitFor(
      "usePkg → helper verified",
      async () => {
        const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
          view: { expanded: ["pkg:npm/fixture", "file:src/usePkg.ts", "file:src/util.ts"] },
        });
        await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
        return view.edges.some(
          (edge) => edge.best === "lsp" && edge.src.endsWith("#usePkg") && edge.dst.endsWith("#helper"),
        );
      },
      30_000,
    );
  });

  it("verifies a file VSCode reads in Latin-1 (review n° 7, E8)", async function () {
    this.timeout(120_000);
    const latin = vscode.Uri.file(path.join(fixture(), "src", "latin.tsx"));
    const document = await vscode.workspace.openTextDocument(latin);
    assert.equal(document.encoding, "iso88591");
    await vscode.window.showTextDocument(document);
    await waitFor(
      "latin → helper verified",
      async () => {
        const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
          view: { expanded: ["pkg:npm/fixture", "file:src/latin.tsx", "file:src/util.ts"] },
        });
        await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
        return view.edges.some(
          (edge) => edge.best === "lsp" && edge.src.endsWith("#latin") && edge.dst.endsWith("#helper"),
        );
      },
      60_000,
    );
  });

  it("converts the columns of names after accents, each way (review n° 8, X6; n° 9, X6b)", async function () {
    this.timeout(120_000);
    // callsWide.ts calls `wide`, whose name the answer places after the accents: UTF-16 → UTF-8.
    // wide.ts asks about `wide` itself, at a position after the accents: UTF-8 → UTF-16.
    for (const file of ["callsWide.ts", "wide.ts"]) {
      const uri = vscode.Uri.file(path.join(fixture(), "src", file));
      await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(uri), {
        viewColumn: vscode.ViewColumn.Beside,
      });
    }
    const verified = (src: string, dst: string) => async () => {
      const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
        view: { expanded: ["pkg:npm/fixture", "file:src/callsWide.ts", "file:src/wide.ts", "file:src/util.ts"] },
      });
      await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
      return view.edges.some((edge) => edge.best === "lsp" && edge.src.endsWith(src) && edge.dst.endsWith(dst));
    };
    await waitFor("callsWide → wide verified", verified("#callsWide", "#wide"), 60_000);
    await waitFor("wide → helper verified", verified("#wide", "#helper"), 60_000);
  });

  it("keeps tree-sitter's edges where no language server answers (review n° 7, E11, E16)", async function () {
    this.timeout(120_000);
    const status = () => timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    const before = await status();
    // No rust-analyzer here: no call hierarchy for lib.rs, no definition for wrap.rs's `Unit`.
    for (const file of ["lib.rs", "wrap.rs"]) {
      const uri = vscode.Uri.file(path.join(fixture(), "tools", "src", file));
      await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(uri), {
        viewColumn: vscode.ViewColumn.Beside,
      });
    }
    await waitFor(
      "both questions failed",
      async () => (await status()).lspAnswers.failed >= before.lspAnswers.failed + 2,
      60_000,
    );
    const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
      view: { expanded: ["pkg:cargo/tools"] },
    });
    await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
    const edge = (kind: string, src: string) =>
      view.edges.find((e) => e.kind === kind && e.src === src && e.dst === "file:tools/src/shapes.rs");
    assert.equal(edge("calls", "file:tools/src/lib.rs")?.best, "tree_sitter", JSON.stringify(view.edges));
    assert.equal(edge("references", "file:tools/src/wrap.rs")?.best, "tree_sitter", JSON.stringify(view.edges));
  });

  it("goes on while a note is being written (review n° 6, m6-2)", async function () {
    this.timeout(120_000);
    const status = () => timonier.daemonRequest<StatusResult>(METHODS.STATUS, {});
    const notes = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(fixture(), "NOTES.md")));
    await vscode.window.showTextDocument(notes);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(notes.uri, new vscode.Position(1, 0), "Unsaved.\n");
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(notes.isDirty);
    try {
      const before = await status();
      // A new file shown in an editor: asked about, and verified, though the note is unsaved.
      const later = path.join(fixture(), "src", "later.ts");
      writeFileSync(later, "export function later(): void {}\n");
      await waitFor("later.ts indexed", async () => (await status()).files > before.files, 30_000);
      await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(vscode.Uri.file(later)), {
        viewColumn: vscode.ViewColumn.Beside,
      });
      await waitFor(
        "later.ts answered",
        async () => (await status()).lspAnswers.outgoingCalls > before.lspAnswers.outgoingCalls,
        60_000,
      );
      assert.equal((await status()).refinementsHeld, 0, "held for an unsaved note");
    } finally {
      await vscode.window.showTextDocument(notes);
      await vscode.commands.executeCommand("workbench.action.files.revert");
    }
  });

  it("opens the code from the map, at the span of a symbol (PLAN.md 4.9)", async function () {
    this.timeout(60_000);
    // The map in view again: the editors of the previous test hid it.
    await vscode.commands.executeCommand("timonier.openMap");
    await waitFor("the map shows boxes", () => (timonier.mapRendered()?.nodes ?? 0) > 0);
    const before = timonier.mapRendered()?.nodes ?? 0;
    // As the palette does: the package, then its file, expanded by the daemon.
    assert.equal(await timonier.mapCommand({ name: "toggle", target: "pkg:npm/fixture" }), true);
    await waitFor("the files", () => (timonier.mapRendered()?.nodes ?? 0) > before);
    const files = timonier.mapRendered()?.nodes ?? 0;
    await timonier.mapCommand({ name: "toggle", target: "file:src/main.ts" });
    await waitFor("the symbols", () => (timonier.mapRendered()?.nodes ?? 0) > files);
    const view = await timonier.daemonRequest<ViewSnapshot>(METHODS.SUBSCRIBE, {
      view: { expanded: ["pkg:npm/fixture", "file:src/main.ts"] },
    });
    await timonier.daemonRequest(METHODS.UNSUBSCRIBE, { subscription: view.subscription });
    const run = view.nodes.find((node) => node.name === "run");
    assert.ok(run?.span, "no span for run");
    await timonier.mapCommand({ name: "openCode", target: run.id });
    const main = path.join(fixture(), "src", "main.ts");
    // The editor of `main.ts` may be open already (the previous test showed it): wait for the
    // cursor, which only the command moves. Among the visible editors: without the focus of
    // its window (a locked session), VSCode does not change the active one (review n° 9, m9-3).
    const atSymbol = () =>
      vscode.window.visibleTextEditors.find(
        (editor) => editor.document.uri.fsPath === main && editor.selection.active.line === run.span?.start.line,
      );
    await waitFor("the editor at the symbol", () => atSymbol() !== undefined);
    const editor = atSymbol();
    assert.ok(editor);
    // « export function run(): void { » is the third line; the cursor at the symbol's start.
    assert.equal(editor.selection.active.line, run.span.start.line);
    assert.equal(editor.selection.active.line, 2);
    // Beside the map, which stays open.
    assert.notEqual(editor.viewColumn, vscode.ViewColumn.One);
  });
});
