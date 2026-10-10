import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
  build: { outDir: mode === "mock" ? "dist-mock" : "dist" },
  plugins: [react(), tailwindcss()],
  server: { host: "localhost", port: 1420, strictPort: true },
  clearScreen: false,
}));
