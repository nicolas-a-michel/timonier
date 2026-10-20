import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The webview is loaded by front-ends (the VSCode extension first) from its build output.
// Front-ends find the hashed entry files through the manifest (dist/.vite/manifest.json).
// Two pages, two builds: the map (`dist/`), then the panels of the tracking (`--mode tracking`,
// `dist/tracking/`, ADR 0031). Each page is one script: under the webview's CSP, only the script
// carrying the nonce runs, and a chunk it imported would not.
export default defineConfig(({ mode }) => {
  const tracking = mode === "tracking";
  return {
    plugins: [react()],
    base: "./",
    build: {
      outDir: tracking ? "dist/tracking" : "dist",
      emptyOutDir: true,
      manifest: true,
      rolldownOptions: { input: tracking ? "tracking.html" : "index.html" },
      // Licences of the bundled dependencies (React…), shipped with every front-end.
      license: { fileName: "THIRD-PARTY-NOTICES.md" },
    },
    test: {
      environment: "node",
    },
  };
});
