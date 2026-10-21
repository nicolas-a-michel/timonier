import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The webview is loaded by front-ends (the VSCode extension first) from its build output.
// Front-ends find the hashed entry files through the manifest (dist/.vite/manifest.json).
// Three pages, three builds: the map (`dist/`), the panels of the tracking (`--mode tracking`,
// `dist/tracking/`, ADR 0031) and the whiteboard (`--mode whiteboard`, `dist/whiteboard/`,
// ADR 0037), and the Performances panel (`--mode performance`, `dist/performance/`, ADR 0051). Each page is one script: under the webview's CSP, only the script carrying the
// nonce runs, and a chunk it imported would not.
const PAGES: Record<string, { outDir: string; input: string }> = {
  tracking: { outDir: "dist/tracking", input: "tracking.html" },
  whiteboard: { outDir: "dist/whiteboard", input: "whiteboard.html" },
  performance: { outDir: "dist/performance", input: "performance.html" },
};

export default defineConfig(({ mode }) => {
  const page = PAGES[mode] ?? { outDir: "dist", input: "index.html" };
  return {
    plugins: [react()],
    base: "./",
    build: {
      outDir: page.outDir,
      emptyOutDir: true,
      manifest: true,
      rolldownOptions: { input: page.input },
      // Licences of the bundled dependencies (React…), shipped with every front-end.
      license: { fileName: "THIRD-PARTY-NOTICES.md" },
    },
    test: {
      environment: "node",
    },
  };
});
