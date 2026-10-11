import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { readBudgets } from "./budgets.ts";
import { machineNoise, withNoise } from "./noise.ts";
import { median, range } from "./stats.ts";

const HARNESS_FAILURE = 2;
const BUDGET_MISS = 1;
const TIMEOUT_MS = 10_000;
const GAP_MS = 500;
const SLOW_SPAWN_MS = 25;
const VARIANT = "index";

const { values: args } = parseArgs({
  options: {
    ci: { type: "boolean", default: false },
    app: { type: "string", default: "target/release/bundle/macos/Aelomi.app" },
    out: { type: "string", default: "perf-results/cold-start" },
  },
});
const binary = resolve(args.app, "Contents/MacOS/aelomi");
const outDir = resolve(args.out);
mkdirSync(outDir, { recursive: true });
const logPath = join(outDir, "harness.log");
writeFileSync(logPath, "");
const log = (line: string) => {
  console.log(line);
  writeFileSync(logPath, `${line}\n`, { flag: "a" });
};
const fail = (code: number, line: string): never => {
  log(line);
  process.exit(code);
};

type JsMarks = {
  variant: string;
  time_origin: number;
  module_eval: number;
  commit: number;
  raf1: number;
  mc_after_raf1: number;
  raf2: number;
  mc_after_raf2: number;
  fcp: number | null;
};
type RustStamps = {
  process_start_us: number;
  main_us: number;
  setup_us: number;
  page_start_us: number;
  page_done_us: number;
  received_us: number;
};
type Report = { js: JsMarks; rust: RustStamps };
type Run = {
  tag: string;
  t0: number;
  exit: string;
  ms: Record<string, number>;
  endToEnd: number;
  navToPaint: number;
};

const epochMs = () => Date.now();
const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));
const fmt = (value: number | undefined) =>
  value === undefined ? "-" : value.toFixed(1);
const load = () =>
  loadavg()
    .map((x) => x.toFixed(2))
    .join(" ");

function isReport(raw: unknown): raw is Report {
  if (typeof raw !== "object" || raw === null) return false;
  const { js, rust } = raw as { js?: unknown; rust?: unknown };
  return (
    typeof js === "object" &&
    js !== null &&
    typeof rust === "object" &&
    rust !== null
  );
}

async function launch(tag: string): Promise<Run> {
  const reportPath = join(outDir, `report-${tag}.json`);
  rmSync(reportPath, { force: true });
  let stderr = "";
  const t0 = epochMs();
  const child = spawn(binary, [], {
    env: { ...process.env, AELOMI_PERF_REPORT: reportPath },
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderr.on("data", (chunk: Buffer) => {
    stderr += chunk.toString();
  });
  const exit = await new Promise<string>((done) => {
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, TIMEOUT_MS);
    child.on("error", (error) => {
      clearTimeout(timer);
      done(`spawn error: ${error.message}`);
    });
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      done(timedOut ? "timeout" : `exit ${code ?? signal}`);
    });
  });
  const stderrTail = stderr.slice(-2000).replace(/^/gm, "    | ");
  if (!existsSync(reportPath)) {
    const reason =
      exit === "timeout"
        ? `no paint marker in ${TIMEOUT_MS / 1000} s (window not visible?)`
        : `process ${exit} without a paint marker`;
    return fail(HARNESS_FAILURE, `${tag}: FAIL ${reason}\n${stderrTail}`);
  }
  const raw: unknown = JSON.parse(readFileSync(reportPath, "utf8"));
  if (!isReport(raw))
    return fail(HARNESS_FAILURE, `${tag}: FAIL report is not {js, rust}`);
  const { js, rust } = raw;
  const problems: string[] = [];
  if (js.variant !== VARIANT)
    problems.push(`variant ${js.variant} is not ${VARIANT}`);
  if (exit !== "exit 0") problems.push(`exit was ${exit}`);
  const ms: Record<string, number> = {
    processStart: rust.process_start_us / 1000 - t0,
    main: rust.main_us / 1000 - t0,
    setup: rust.setup_us / 1000 - t0,
    pageStart: rust.page_start_us / 1000 - t0,
    navStart: js.time_origin - t0,
    pageDone: rust.page_done_us / 1000 - t0,
    moduleEval: js.module_eval - t0,
    commit: js.commit - t0,
    raf1: js.raf1 - t0,
    rafMc: js.mc_after_raf1 - t0,
    raf2: js.raf2 - t0,
    rafMc2: js.mc_after_raf2 - t0,
    received: rust.received_us / 1000 - t0,
  };
  if (js.fcp !== null) ms.fcp = js.fcp - t0;
  for (const [name, value] of Object.entries(ms)) {
    if (!(value > 0 && value < TIMEOUT_MS))
      problems.push(`${name} ${fmt(value)} ms is not inside the run`);
  }
  if (problems.length > 0)
    return fail(
      HARNESS_FAILURE,
      `${tag}: FAIL ${problems.join("; ")}\n${stderrTail}`,
    );
  const firstContentFrame = Math.min(ms.rafMc ?? 0, ms.raf2 ?? 0);
  const run: Run = {
    tag,
    t0,
    exit,
    ms,
    endToEnd: firstContentFrame,
    navToPaint: firstContentFrame - (ms.navStart ?? 0),
  };
  const flags = [
    (ms.processStart ?? 0) > SLOW_SPAWN_MS ? " SLOW-SPAWN" : "",
    (ms.raf2 ?? 0) < (ms.rafMc ?? 0) ? " MC-AFTER-RAF2" : "",
  ].join("");
  const marks = Object.entries(ms)
    .map(([name, value]) => `${name} ${fmt(value)}`)
    .join(" ");
  log(
    `${tag}: end-to-end ${fmt(run.endToEnd)} ms | nav-to-paint ${fmt(run.navToPaint)} ms | ${marks}${flags}`,
  );
  return run;
}

const budgets = readBudgets().coldStart;
const warmupRuns = budgets.warmupRuns.value;
const timedRuns = budgets.timedRuns.value;
if (!existsSync(binary))
  fail(
    HARNESS_FAILURE,
    `no app binary at ${binary}; run pnpm tauri build --bundles app`,
  );
log(
  `cold-start harness ${args.ci ? "(--ci, gates hosted budgets)" : "(local, gates specMs)"}. node ${process.version}, ${cpus().length} cpus (${cpus()[0]?.model}), load ${load()}, ${new Date().toISOString()}`,
);
log(`app ${binary}`);
log(
  `plan: ${warmupRuns} discarded warm-up, then ${timedRuns} timed runs. T0 is the wall clock (Date.now, 1 ms resolution) read just before spawn, the same clock as the kernel's process start stamp. Marks are ms after T0. First content frame = min(rAF+MessageChannel, double rAF). ${TIMEOUT_MS / 1000} s timeout per run.`,
);

const startNoise = machineNoise();
if (startNoise) log(startNoise);

for (let i = 1; i <= warmupRuns; i++) await launch(`warmup-${i}`);
await sleep(GAP_MS);
const runs: Run[] = [];
for (let i = 1; i <= timedRuns; i++) {
  runs.push(await launch(`run-${i}`));
  await sleep(GAP_MS);
}

const endToEnd = runs.map((run) => run.endToEnd);
const navToPaint = runs.map((run) => run.navToPaint);
const endToEndMedian = median(endToEnd);
const navToPaintMedian = median(navToPaint);
const verdict = (value: number, budget: number) =>
  `${fmt(value)} ms ${value <= budget ? "<=" : ">"} ${budget} ms ${value <= budget ? "PASS" : "FAIL"}`;
const hostedPass =
  endToEndMedian <= budgets.hostedCiMs.value &&
  navToPaintMedian <= budgets.hostedNavToPaintMs.value;
const specPass = endToEndMedian <= budgets.specMs.value;

log("");
log(
  `median end-to-end ${fmt(endToEndMedian)} ms (${timedRuns} runs, ${range(endToEnd)})`,
);
log(
  `median nav-to-paint ${fmt(navToPaintMedian)} ms (${timedRuns} runs, ${range(navToPaint)})`,
);
log(
  `hosted verdict${args.ci ? " (gate)" : " (information)"}: end-to-end ${verdict(endToEndMedian, budgets.hostedCiMs.value)}; nav-to-paint ${verdict(navToPaintMedian, budgets.hostedNavToPaintMs.value)}`,
);
log(
  `spec verdict${args.ci ? " (information)" : " (gate)"}: end-to-end ${verdict(endToEndMedian, budgets.specMs.value)}`,
);
log(`load at end ${load()}`);
const endNoise = machineNoise();
if (endNoise) log(endNoise);
writeFileSync(
  join(outDir, "results.json"),
  JSON.stringify(
    {
      mode: args.ci ? "ci" : "local",
      node: process.version,
      cpus: cpus().length,
      budgets,
      runs,
      endToEndMedian,
      navToPaintMedian,
      hostedPass,
      specPass,
    },
    null,
    2,
  ),
);
const gated = args.ci ? hostedPass : specPass;
log(
  withNoise(
    gated ? "cold start: PASS" : "cold start: FAIL (budget miss)",
    startNoise ?? endNoise,
  ),
);
process.exit(gated ? 0 : BUDGET_MISS);
