import { spawn } from "node:child_process";
import { type AddressInfo, createServer } from "node:net";
import { resolve } from "node:path";

const HOST = "localhost";

const freePort = () =>
  new Promise<number>((done, fail) => {
    const server = createServer();
    server.once("error", fail);
    server.listen(0, HOST, () => {
      const { port } = server.address() as AddressInfo;
      server.close(() => done(port));
    });
  });

const requested = process.env.AELOMI_DEV_PORT;
const port = requested ? Number(requested) : await freePort();
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  console.error(`AELOMI_DEV_PORT must be a port number, got "${requested}"`);
  process.exit(2);
}
console.log(`AELOMI_DEV_PORT=${port}`);

const config = {
  build: {
    devUrl: `http://${HOST}:${port}`,
    beforeDevCommand: `pnpm dev --port ${port} --strictPort`,
  },
};
const root = resolve(import.meta.dirname, "..");
const tauri = spawn(
  resolve(root, "node_modules/.bin/tauri"),
  ["dev", "--config", JSON.stringify(config), ...process.argv.slice(2)],
  { cwd: root, stdio: "inherit" },
);
for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.on(signal, () => tauri.kill(signal));
}
tauri.on("exit", (code) => process.exit(code ?? 1));
