import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { resolve } from "node:path";
import { STAGES } from "./check-all-stages.ts";

const root = resolve(import.meta.dirname, "..");
// CI makes Vitest and Playwright reject a leftover test.only, as they do in CI.
const env = {
  ...process.env,
  CI: "true",
  E2E_TELEMETRY_DISABLED: "1",
  DO_NOT_TRACK: "1",
};

const git = (...args: string[]) =>
  spawnSync("git", args, { cwd: root, encoding: "utf8" }).stdout.trim();

const run = (command: string[]) => {
  console.log(`\n$ ${command.join(" ")}`);
  const [file = "", ...args] = command;
  return spawnSync(file, args, { cwd: root, env, stdio: "inherit" }).status;
};

// Spotlight indexes a fresh build, which can push the load past 10x the core
// count for minutes. CI waits the same way.
const waitForQuiet = () => {
  const limit = cpus().length / 2;
  const deadline = Date.now() + 2 * 60 * 1000;
  for (;;) {
    const load = loadavg()[0] ?? 0;
    console.log(`load ${load.toFixed(2)}, waiting for under ${limit}`);
    if (load < limit) return;
    if (Date.now() > deadline) {
      console.log("still busy after 2 minutes; measuring anyway");
      return;
    }
    spawnSync("sleep", ["15"]);
  }
};

const fail = (message: string): never => {
  console.error(`\ncheck:all failed: ${message}`);
  process.exit(1);
};

// The record vouches for a commit, so uncommitted or untracked files would make it lie.
if (git("status", "--porcelain") !== "")
  fail("commit or stash every change first");
const sha = git("rev-parse", "HEAD");

if (run(["pnpm", "install", "--frozen-lockfile"]) !== 0)
  fail("pnpm install --frozen-lockfile");
const stages: { job: string; seconds: number }[] = [];
for (const { job, commands, perf } of STAGES) {
  console.log(`\n== ${job}`);
  if (perf) waitForQuiet();
  const start = performance.now();
  for (const command of commands)
    if (run(command) !== 0) fail(`${job}: ${command.join(" ")}`);
  stages.push({ job, seconds: Math.round((performance.now() - start) / 1000) });
}

if (git("rev-parse", "HEAD") !== sha) fail("HEAD moved during the run");
if (git("status", "--porcelain") !== "")
  fail(`the run changed tracked files:\n${git("status", "--porcelain")}`);

const dir = resolve(root, ".local-gate");
mkdirSync(dir, { recursive: true });
const record = resolve(dir, `${sha}.checks.json`);
writeFileSync(
  record,
  `${JSON.stringify({ sha, passed: true, finishedAt: new Date().toISOString(), stages }, null, 2)}\n`,
);
console.log(`\ncheck:all passed on ${sha}. Record: ${record}`);
