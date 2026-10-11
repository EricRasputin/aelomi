import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: [
      "src/**/*.test.ts",
      "tests/*.test.ts",
      "fixtures/**/*.test.ts",
      "perf/**/*.test.ts",
    ],
    exclude: [
      ".claude/**",
      ".agents/**",
      "node_modules/**",
      "dist/**",
      "dist-mock/**",
      "e2e/**",
      "tests/playwright/**",
      "target/**",
    ],
  },
});
