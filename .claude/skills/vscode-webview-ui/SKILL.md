---
name: vscode-webview-ui
description: Building Timonier's VSCode extension and React webview — the stack (React, React Flow/xyflow, elkjs, uPlot; no tldraw), the design system (tokens on VSCode CSS variables, light/dark/high-contrast), the postMessage protocol between webview and extension, webview security (CSP), accessibility (WCAG AA, keyboard, reduced motion) and the five-second test. Use for any work in editors/vscode/ or webview/, for any new view, panel or visual element, and for any UX decision.
---

# VSCode extension and webview UI

## Design principles (spec, « Expérience et design »). They apply to every view, from the MVP on.

**Calm by default**

- Desaturated palette on neutral backgrounds.
- Vivid colour is reserved for meaning and appears only when there is something to say: red
  for a violation, orange for attention, green for conformity.
- Native look: the IDE theme through its CSS variables, in light and in dark.
- Animations are short and soft, and only explain a change (a component appearing, an edge
  thickening). Never decorative.
- Discreet notifications: badges and summaries. Only blocking alerts interrupt.

**Clear before complete**

- The home view, « le pont », answers three questions in a few seconds: where the plan
  stands, what happened since my last review, where I should look.
- Progressive disclosure: overview first, detail on hover, depth on click.
- Semantic zoom on the map: zooming changes the level of detail (components → files →
  symbols) instead of drawing everything smaller.
- Stable layout: the map keeps element positions across updates (incremental, constrained
  layout).
- One visual language for all views: the same shape, colour and icon always mean the same
  thing (component, agent, violation).

**Accessible and fluid**

- WCAG AA contrast at least, a colour-blind-safe palette, and colour never carries
  information alone: always add an icon or a pattern.
- Full keyboard navigation, and a command-palette command for every action.
- Level-of-detail rendering, with automatic aggregation beyond a node threshold.
- Adjustable density (comfortable or compact).
- Teaching empty states that say what to do next.

## Stack (decided in the spec; versions verified on 2026-10-14)

| Use                         | Package           | Version, licence and caveat                                                        |
| --------------------------- | ----------------- | ---------------------------------------------------------------------------------- |
| UI                          | `react`           | 19.3.0, MIT                                                                        |
| Architecture map            | `@xyflow/react`   | 12.12.0, MIT. The old `reactflow` package is frozen: do not use it.                |
| Hierarchical layout         | `elkjs`           | 0.12.0, EPL-2.0 OR GPL-3.0-or-later. Use it under EPL-2.0.                         |
| Whiteboard                  | to be chosen      | **Not tldraw**: the human ruled it out (`PLAN.md` Q3, proprietary licence with a key). The replacement is chosen by ADR at the start of Phase 7; the leading option is React Flow. |
| Timelines and perf curves   | `uplot`           | 1.6.32, MIT                                                                        |
| Data-flow animation (v2)    | `pixi.js`         | 8.22.0, MIT                                                                        |
| Native-looking widgets      | `@vscode-elements/elements` | 2.5.1, MIT. `@vscode/webview-ui-toolkit` is deprecated: do not use it.  |
| Build                       | `vite` / `esbuild` | Vite 8.3.4 (MIT) builds the webview, with a manifest and third-party notices; esbuild 0.28.2 (MIT) bundles the extension (ADR 0005). |

TypeScript: `typescript-eslint` 8.x does not support TypeScript 7, so pin TypeScript 6.0.x
until it does. Check again before upgrading.

## Architecture

- The webview **never** talks to the daemon. The path is:
  daemon ⇄ JSON-RPC 2.0 over a local socket ⇄ extension ⇄ `postMessage` ⇄ webview.
- Subscriptions are per view and carry versioned deltas. If the webview sees a version gap,
  it asks for a full snapshot. Every message is typed and carries a protocol version. The
  message types come from a single source; the generation mechanism is fixed by ADR.
- The graph truth lives in the daemon. The webview keeps only view state, persisted with
  `vscode.getState()` / `setState()` (`acquireVsCodeApi()` is called once).
- Security:
  - CSP meta tag with `default-src 'none'` and `script-src 'nonce-…'` only (no origin, no
    `unsafe-inline`). An integration test proves the bundle runs under it: the webview posts
    `{ type: "ready" }` after its first render;
  - so each page is **one** script: a chunk it imported would not carry the nonce. A new page
    gets its own Vite build (`vite build --mode <page>`, its own `dist/<page>/` and manifest),
    as the panels of the tracking do (`tracking.html`, ADR 0031), and its files are listed in
    `editors/vscode/.vscodeignore`;
  - no remote resources;
  - `localResourceRoots` restricted to the extension's build output;
  - `enableScripts` only where needed.

## Interface language

English base with a French translation (ADR 0006, accepted; more languages will come later):
`package.nls*.json` for the manifest, `vscode.l10n.t` with `l10n/bundle.l10n.fr.json` in the
extension, `webview/src/i18n.ts` in the webview (language read from `<html lang>`). A unit test
fails when a string has no French translation.

## Design tokens

- Exactly one file defines the tokens as CSS custom properties, prefixed `--tim-`:
  `webview/src/tokens.css`.
- Components use **only** tokens: no raw hex colour, no ad-hoc spacing. A vitest test
  (`App.test.tsx`, « design tokens ») rejects raw colours in every other stylesheet.
- Tokens map to VSCode theme variables, documented as "`editor.foreground` →
  `var(--vscode-editor-foreground)`". Examples:
  - surfaces and text: `--vscode-editor-background`, `--vscode-foreground`,
    `--vscode-descriptionForeground`;
  - lines and focus: `--vscode-panel-border`, `--vscode-focusBorder`;
  - semantic colours: `--vscode-charts-red`, `--vscode-charts-orange`, `--vscode-charts-green`,
    `--vscode-charts-blue`, `--vscode-charts-yellow`, `--vscode-charts-purple`,
    `--vscode-charts-foreground`, `--vscode-charts-lines`.
- Typography: `--vscode-font-family` and `--vscode-font-size` for the UI;
  `--vscode-editor-font-family` for code.
- Theme kinds: `body.vscode-light`, `body.vscode-dark`, `body.vscode-high-contrast`. Test every
  screen in all three, including a high-contrast light theme.
- Motion: one set of duration tokens. Under `@media (prefers-reduced-motion: reduce)`,
  animations are off and an equivalent static rendering is shown.
- Precision (spec): an edge known only from tree-sitter **looks different** from an edge
  confirmed by LSP or SCIP. The exact encoding is decided in the design system, and it must
  not rely on colour alone.

## Where the design system lives (ADR 0020)

- Tokens: `webview/src/tokens.css`. Components: `webview/src/design/` (component box, edge,
  badge, empty state, tooltip, toolbar), styled in `design.css` with tokens only.
- Every component appears, in every state, on the demonstration page `webview/demo.html`
  (`npm run dev -w webview`, then `/demo.html?theme=<name>&locale=fr`).
- `npm run test:a11y` runs axe-core and measures 3:1 for lines, icons and focus on that page,
  in the VSCode themes captured in `webview/test/themes/`. Recapture them with
  `editors/vscode/scripts/capture-themes.mjs` when VSCode changes its themes.
- A new graphic that carries meaning gets `data-contrast` (`outline`, `stroke` or `icon`) so
  that the test measures it.

## Accessibility checklist (for every screen)

- [ ] Contrast at least 4.5:1 for text and 3:1 for UI and graphics, in light, dark and high
      contrast.
- [ ] Every action is reachable by keyboard and exists as a command (`contributes.commands`).
- [ ] Focus is visible everywhere. Map nodes and edges have accessible names.
- [ ] Information is never carried by colour alone.
- [ ] Reduced motion is respected.
- [ ] An automated check (axe-core) runs in the tests.

## The five-second test

1. Show the screen for five seconds to someone who has not seen it, then hide it.
2. Ask what the screen shows and what they would do next.
3. The test passes if they answer the screen's main question correctly.
4. Record the screen, date, tester, result and any fix in `PLAN.md`.

## Tests

- Webview: unit tests with vitest (Node environment, `react-dom/server`) for state, reducers,
  rendering and message handling.
- Extension: unit tests with `node:test` on the `tsc` output (`src/test/unit/`) for pure modules
  (HTML/CSP, manifest, translations).
- Component tests for the design-system components.
- Extension integration tests (`@vscode/test-cli` / `@vscode/test-electron`, `src/test/integration/`) for activation,
  commands and the daemon relay.
