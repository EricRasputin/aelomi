import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
  resolve: {
    alias: {
      "@ipc-commands": fileURLToPath(
        new URL(
          mode === "mock" ? "./src/ipc/mock.ts" : "./src/ipc/bindings.ts",
          import.meta.url,
        ),
      ),
    },
  },
  build: { outDir: mode === "mock" ? "dist-mock" : "dist" },
  plugins: [react(), tailwindcss()],
  server: { host: "localhost", port: 1420, strictPort: true },
  clearScreen: false,
}));
