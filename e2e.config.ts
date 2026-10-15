import type { E2EConfig } from "e2e";
import { acceptanceEngine } from "./e2e/browser";

export default {
  tests: ["e2e/**/*.e2e.ts"],
  reporters: ["list", "junit"],
  retries: 0,
  targets: [
    {
      name: "webkit",
      engine: acceptanceEngine,
      app: {
        url: "http://127.0.0.1:0",
        command: {
          executable: "pnpm",
          args: ["preview:mock", "--port", "{port}", "--strictPort"],
          log: ".e2e/logs/app.log",
        },
      },
    },
  ],
} satisfies E2EConfig;
