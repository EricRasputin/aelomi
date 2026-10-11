#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import {
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { expect, webkit } from "@playwright/test";
import { isLocalUrl } from "../../../../tests/is-local-url.ts";

const USAGE = `Usage: mock-drive.mjs [--port N] [--flow FILE] [--out DIR] [--rebuild] [--headed]
  --port N     serve dist-mock on 127.0.0.1:N (default: a free port; 4173 is refused)
  --flow FILE  flow module to run (default: flows/ping.mjs next to this script)
  --out DIR    evidence directory (default: /tmp/aelomi-verify/<timestamp>-mock)
  --rebuild    run pnpm build:mock even if dist-mock is newer than the sources
  --headed     show the WebKit window`;

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../../..");
const reservedPort = 4173;

const { values: opts } = parseArgs({
  options: {
    port: { type: "string" },
    flow: { type: "string", default: path.join(here, "flows/ping.mjs") },
    out: { type: "string" },
    rebuild: { type: "boolean", default: false },
    headed: { type: "boolean", default: false },
    help: { type: "boolean", default: false },
  },
});
if (opts.help) {
  console.log(USAGE);
  process.exit(0);
}

const stamp = new Date()
  .toISOString()
  .replace(/[-:]/g, "")
  .replace("T", "-")
  .slice(0, 15);
const out = path.resolve(
  opts.out ??
    path.join(
      process.env.AELOMI_VERIFY_ROOT ?? "/tmp/aelomi-verify",
      `${stamp}-mock`,
    ),
);
mkdirSync(out, { recursive: true });

function newestMtime(entry) {
  const full = path.join(repo, entry);
  if (!existsSync(full)) return 0;
  const stat = statSync(full);
  if (!stat.isDirectory()) return stat.mtimeMs;
  return Math.max(
    0,
    ...readdirSync(full, { recursive: true }).map(
      (file) => statSync(path.join(full, String(file))).mtimeMs,
    ),
  );
}

function ensureBuild() {
  const built = path.join(repo, "dist-mock/index.html");
  const sources = Math.max(
    ...["src", "index.html", "vite.config.ts", "package.json"].map(
      newestMtime,
    ),
  );
  if (
    !opts.rebuild &&
    existsSync(built) &&
    statSync(built).mtimeMs >= sources
  ) {
    return "reused";
  }
  const build = spawnSync("pnpm", ["build:mock"], {
    cwd: repo,
    stdio: "inherit",
  });
  if (build.status !== 0) throw new Error("pnpm build:mock failed");
  return "built";
}

function portIsFree(port) {
  return new Promise((resolve) => {
    const server = createServer();
    server.once("error", () => resolve(false));
    server.listen(port, "127.0.0.1", () => server.close(() => resolve(true)));
  });
}

function freePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

async function choosePort() {
  if (opts.port === undefined) return freePort();
  const port = Number(opts.port);
  if (!Number.isInteger(port) || port <= 0) throw new Error(USAGE);
  if (port === reservedPort) {
    throw new Error(
      `Port ${reservedPort} belongs to the Playwright suite. Pick another port or omit --port.`,
    );
  }
  if (!(await portIsFree(port))) {
    throw new Error(
      `Port ${port} is in use. Never drive a server you did not start.`,
    );
  }
  return port;
}

// Runs the preview:mock script's vite command directly, so killing one process group stops the server.
function startPreview(port) {
  const pkg = JSON.parse(readFileSync(path.join(repo, "package.json"), "utf8"));
  const [bin, ...args] = pkg.scripts["preview:mock"].split(" ");
  const log = createWriteStream(path.join(out, "preview.log"));
  const child = spawn(
    path.join(repo, "node_modules/.bin", bin),
    [...args, "--port", String(port), "--strictPort"],
    { cwd: repo, detached: true, stdio: ["ignore", "pipe", "pipe"] },
  );
  child.stdout.pipe(log);
  child.stderr.pipe(log);
  return child;
}

async function waitForServer(url, child) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(
        `Preview server exited with code ${child.exitCode}. See ${out}/preview.log`,
      );
    }
    try {
      if ((await fetch(url)).ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Preview server did not answer at ${url} within 30 s`);
}

async function stopPreview(child) {
  if (!child || child.exitCode !== null) return;
  const exited = new Promise((resolve) => child.once("exit", resolve));
  process.kill(-child.pid, "SIGTERM");
  await Promise.race([
    exited,
    new Promise((resolve) => setTimeout(resolve, 5_000)),
  ]);
  if (child.exitCode === null) process.kill(-child.pid, "SIGKILL");
}

// Mirrors the nonLocalRequests fixture in tests/playwright/fixtures.ts (SHELL-13).
async function guardNetwork(context) {
  const requests = [];
  context.on("request", (request) => {
    if (!isLocalUrl(request.url())) {
      requests.push(`${request.method()} ${request.url()}`);
    }
  });
  await context.route("**/*", (route) =>
    isLocalUrl(route.request().url()) ? route.fallback() : route.abort(),
  );
  await context.routeWebSocket(/.*/, (socket) => {
    if (isLocalUrl(socket.url())) {
      socket.connectToServer();
    } else {
      requests.push(`WS ${socket.url()}`);
      void socket.close();
    }
  });
  return requests;
}

const summary = { flow: path.resolve(opts.flow), out, ok: false };
let server;
let browser;

async function shutdown() {
  await browser?.close().catch(() => {});
  await stopPreview(server);
  if (summary.port !== undefined) {
    summary.portFreeAfterCleanup = await portIsFree(summary.port);
  }
}

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.once(signal, async () => {
    summary.error = `interrupted by ${signal}`;
    await shutdown();
    writeFileSync(
      path.join(out, "summary.json"),
      `${JSON.stringify(summary, null, 2)}\n`,
    );
    process.exit(130);
  });
}
process.on("exit", () => {
  if (server?.exitCode === null) process.kill(-server.pid, "SIGKILL");
});

let page;
try {
  summary.port = await choosePort();
  summary.bundle = ensureBuild();
  summary.url = `http://127.0.0.1:${summary.port}`;
  server = startPreview(summary.port);
  await waitForServer(summary.url, server);

  browser = await webkit.launch({
    headless: !opts.headed,
    handleSIGINT: false,
    handleSIGTERM: false,
    handleSIGHUP: false,
  });
  const context = await browser.newContext({
    baseURL: summary.url,
    serviceWorkers: "block",
  });
  const nonLocalRequests = await guardNetwork(context);
  page = await context.newPage();

  const shots = [];
  const shot = async (name) => {
    await page.screenshot({ path: path.join(out, `${name}.png`) });
    writeFileSync(
      path.join(out, `${name}.aria.yml`),
      await page.locator("body").ariaSnapshot(),
    );
    shots.push(name);
  };

  const { default: flow } = await import(pathToFileURL(summary.flow).href);
  await flow({ page, expect, shot, out });

  summary.mockCalls = await page.evaluate(
    () => window.__aelomiMock?.calls ?? null,
  );
  summary.nonLocalRequests = nonLocalRequests;
  summary.shots = shots;
  expect(nonLocalRequests, "SHELL-13 non-local requests").toEqual([]);
  summary.ok = true;
} catch (error) {
  summary.error = error instanceof Error ? error.message : String(error);
  await page
    ?.screenshot({ path: path.join(out, "failure.png") })
    .catch(() => {});
} finally {
  await shutdown();
  writeFileSync(
    path.join(out, "summary.json"),
    `${JSON.stringify(summary, null, 2)}\n`,
  );
}

console.log(
  `${summary.ok ? "PASS" : "FAIL"} ${path.basename(summary.flow)} on ${summary.url ?? "no server"}`,
);
if (summary.error) console.log(summary.error);
console.log(`Evidence: ${out}`);
process.exit(summary.ok ? 0 : 1);
