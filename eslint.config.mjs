import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig, globalIgnores } from "eslint/config";
import tseslint from "typescript-eslint";

// Globals of the Node.js scripts (hooks, build scripts, configs).
const nodeGlobals = {
  console: "readonly",
  process: "readonly",
  Buffer: "readonly",
  URL: "readonly",
  setTimeout: "readonly",
  clearTimeout: "readonly",
  fetch: "readonly",
  WebSocket: "readonly",
};

export default defineConfig([
  // The packs' instrumentation templates are written into other projects, with their own SDK.
  globalIgnores([
    "**/dist/",
    "**/out/",
    "**/node_modules/",
    "target/",
    "**/.vscode-test/",
    "**/coverage/",
    "crates/timonier-index/packs/*/instrumentation/",
  ]),
  {
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, tseslint.configs.recommendedTypeChecked],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      // node:test's describe/it return promises that the runner awaits itself.
      "@typescript-eslint/no-floating-promises": [
        "error",
        { allowForKnownSafeCalls: [{ from: "package", package: "node:test", name: ["describe", "it", "test"] }] },
      ],
    },
  },
  {
    files: ["webview/src/**/*.{ts,tsx}"],
    extends: [reactHooks.configs.flat.recommended],
  },
  {
    files: ["**/*.{js,mjs,cjs}"],
    extends: [js.configs.recommended],
    languageOptions: { globals: nodeGlobals },
  },
]);
