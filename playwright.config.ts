import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/playwright",
  testMatch: "**/*.spec.ts",
  forbidOnly: Boolean(process.env.CI),
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://127.0.0.1:4173",
    serviceWorkers: "block",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "webkit",
      use: { browserName: "webkit" },
      testIgnore: "**/perf/**",
    },
    {
      name: "perf",
      use: { browserName: "webkit" },
      testMatch: "**/perf/**/*.spec.ts",
      retries: 0,
      timeout: 20 * 60 * 1000,
    },
    {
      name: "perf-headed",
      use: { browserName: "webkit", headless: false },
      testMatch: "**/perf/**/*.spec.ts",
      retries: 0,
      timeout: 20 * 60 * 1000,
    },
  ],
  webServer: {
    command: "pnpm preview:mock --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
  },
});
