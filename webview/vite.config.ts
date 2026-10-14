import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The webview is loaded by front-ends (the VSCode extension first) from its build output.
// Front-ends find the hashed entry files through the manifest (dist/.vite/manifest.json).
export default defineConfig({
  plugins: [react()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    manifest: true,
    // Licences of the bundled dependencies (React…), shipped with every front-end.
    license: { fileName: "THIRD-PARTY-NOTICES.md" },
  },
  test: {
    environment: "node",
  },
});
