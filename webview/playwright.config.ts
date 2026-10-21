import { defineConfig } from "@playwright/test";

// The design system's accessibility tests (ADR 0020): the demonstration page, served by Vite, in
// every captured VSCode theme; and the whiteboard as built for production, under the webview's
// CSP (test/production, PLAN.md 7.1). `npm run test:a11y`; Chromium comes from
// `npx playwright install`.
const port = 5199;

export default defineConfig({
  // `BENCH=1`: the map's measurements (test/perf), apart from the accessibility tests.
  testDir: process.env.BENCH ? "test/perf" : "test",
  testMatch: process.env.BENCH ? "**/*.perf.ts" : ["a11y/**/*.a11y.ts", "production/**/*.prod.ts"],
  // The whiteboard's tests and bench need its production build.
  globalSetup: "./test/production/build.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    browserName: "chromium",
    viewport: { width: 1100, height: 900 },
    // A stuck action names itself, instead of the whole test timing out.
    actionTimeout: 5_000,
  },
  webServer: {
    command: `npx vite --port ${port} --strictPort --host 127.0.0.1`,
    url: `http://127.0.0.1:${port}/demo.html`,
    reuseExistingServer: !process.env.CI,
  },
});
