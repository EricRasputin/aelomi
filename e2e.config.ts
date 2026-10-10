import { web } from "@e2e-dev/web";
import type { E2EConfig } from "e2e";

export default {
  tests: ["e2e/**/*.e2e.ts"],
  reporters: ["list", "junit"],
  retries: 0,
  targets: [
    {
      name: "webkit",
      // This engine blocks service workers only when headers are configured.
      engine: web({ browser: "webkit", headers: {} }),
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
