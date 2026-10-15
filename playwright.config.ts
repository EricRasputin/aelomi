import { defineConfig } from "@playwright/test";

const port = Number(process.env.AELOMI_TEST_PORT ?? 4173);
if (!Number.isInteger(port) || port < 1 || port > 65535)
  throw new Error("AELOMI_TEST_PORT must be an integer from 1 to 65535");
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./tests/playwright",
  testMatch: "**/*.spec.ts",
  forbidOnly: Boolean(process.env.CI),
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL,
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
  ],
  webServer: {
    command: `pnpm preview:mock --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
