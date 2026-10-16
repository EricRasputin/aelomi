import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vitest";
import { LOCAL_ENV, STAGES, type Stage } from "./check-all-stages.ts";

const ci = readFileSync(
  resolve(import.meta.dirname, "../.github/workflows/ci.yml"),
  "utf8",
);

const parser = "/usr/bin/ruby";

test("the workflow is valid YAML", { timeout: 20_000 }, () => {
  const parsed = spawnSync(parser, ["-ryaml", "-e", "YAML.parse(STDIN.read)"], {
    input: ci,
    encoding: "utf8",
  });
  if (parsed.error) {
    throw new Error(`Could not run the CI YAML parser ${parser}`, {
      cause: parsed.error,
    });
  }
  expect(
    parsed.status,
    `${parser} failed with ${parsed.signal ?? `exit ${parsed.status}`}: ${parsed.stderr}`,
  ).toBe(0);
});

// One-line `run:` steps per job. Multi-line `run: |` blocks are CI's quiet-machine wait,
// which check:all does natively, and a bundle-attribute printout.
const ciRuns = new Map(
  (ci.split(/^jobs:$/m)[1] ?? "")
    .split(/^ {2}(?=[\w-]+:$)/m)
    .filter((block) => block.trim() !== "")
    .map((block) => {
      const [header = "", ...lines] = block.split("\n");
      const runs = lines
        .map((line) => line.match(/^\s+(?:- )?run: (.+)$/)?.[1])
        .filter((run): run is string => run !== undefined && run !== "|");
      return [header.replace(/:$/, ""), runs];
    }),
);

// Steps only a hosted runner needs. check:all runs pnpm install once before every stage.
const CI_ONLY = new Set([
  "pnpm install --frozen-lockfile",
  "rustup show active-toolchain",
  `node -e 'console.log("version=" + require("@playwright/test/package.json").version)' >> "$GITHUB_OUTPUT"`,
  "ditto -c -k --keepParent target/release/bundle/macos/Aelomi.app Aelomi-app.zip",
  "sudo mdutil -a -i off",
  "ditto -x -k Aelomi-app.zip app",
]);

// CI launches the bundle it downloaded and unzipped; check:all launches the one it built.
const LOCAL_FOR_CI = new Map([
  [
    "node perf/cold-start.ts --ci --app app/Aelomi.app --out perf-results/cold-start",
    "node perf/cold-start.ts --ci",
  ],
]);

test("check:all runs a stage for every CI job and no other", () => {
  expect(ciRuns.size).toBeGreaterThan(0);
  expect(STAGES.map((stage) => stage.job).sort()).toEqual(
    [...ciRuns.keys()].sort(),
  );
});

test("each stage runs its CI job's commands, reusing what earlier stages ran", () => {
  const ran = new Set<string>();
  for (const { job, commands } of STAGES) {
    const local = commands.map((command) => command.join(" "));
    const wanted = (ciRuns.get(job) ?? [])
      .filter((run) => !CI_ONLY.has(run))
      .map((run) => LOCAL_FOR_CI.get(run) ?? run);
    for (const command of local) ran.add(command);
    expect({
      job,
      extra: local.filter((command) => !wanted.includes(command)),
      missing: wanted.filter((run) => !ran.has(run)),
    }).toEqual({ job, extra: [], missing: [] });
  }
});

type Env = Record<string, string>;
type Step = { run?: string; env?: Env };
type Workflow = {
  env: Env;
  jobs: Map<string, { env: Env; steps: Step[] }>;
};

function isEnv(value: unknown): value is Env {
  return (
    typeof value === "object" &&
    value !== null &&
    Object.values(value).every((entry) => typeof entry === "string")
  );
}

function envOf(value: unknown): Env {
  if (value === undefined || value === null) return {};
  if (!isEnv(value))
    throw new Error(`env is not a string map: ${String(value)}`);
  return value;
}

function parseWorkflow(text: string): Workflow {
  const json = spawnSync(
    parser,
    [
      "-ryaml",
      "-rjson",
      "-e",
      "puts JSON.generate(YAML.safe_load(STDIN.read))",
    ],
    { input: text, encoding: "utf8" },
  );
  const parsed: unknown = JSON.parse(json.stdout);
  if (typeof parsed !== "object" || parsed === null || !("jobs" in parsed))
    throw new Error("the workflow has no jobs");
  const { jobs } = parsed;
  if (typeof jobs !== "object" || jobs === null)
    throw new Error("jobs is not a map");
  return {
    env: envOf("env" in parsed ? parsed.env : undefined),
    jobs: new Map(
      Object.entries(jobs).map(([name, job]: [string, unknown]) => {
        if (typeof job !== "object" || job === null || !("steps" in job))
          throw new Error(`job ${name} has no steps`);
        const steps: unknown = job.steps;
        if (!Array.isArray(steps)) throw new Error(`${name} steps`);
        return [
          name,
          {
            env: envOf("env" in job ? job.env : undefined),
            steps: steps.map((step: unknown): Step => {
              if (typeof step !== "object" || step === null) return {};
              const run =
                "run" in step && typeof step.run === "string"
                  ? step.run
                  : undefined;
              const env = envOf("env" in step ? step.env : undefined);
              return run === undefined ? { env } : { run, env };
            }),
          },
        ];
      }),
    ),
  };
}

// GitHub sets CI=true in every hosted job:
// https://docs.github.com/en/actions/reference/workflows-and-actions/variables#default-environment-variables
const HOSTED_DEFAULTS: Env = { CI: "true" };

/** Each job whose hosted environment differs from its local stage's, for the commands both run. */
function environmentDrift(stages: readonly Stage[], workflow: Workflow) {
  return stages.flatMap(({ job, commands, env }) => {
    const local = commands.map((command) => command.join(" "));
    const hostedJob = workflow.jobs.get(job);
    const steps = (hostedJob?.steps ?? []).filter(
      (step) =>
        step.run !== undefined &&
        local.includes(LOCAL_FOR_CI.get(step.run) ?? step.run),
    );
    const hosted: Env = {
      ...HOSTED_DEFAULTS,
      ...workflow.env,
      ...hostedJob?.env,
      ...Object.assign({}, ...steps.map((step) => step.env)),
    };
    const ours: Env = { ...LOCAL_ENV, ...env };
    const sorted = (vars: Env) =>
      Object.fromEntries(Object.entries(vars).sort());
    return JSON.stringify(sorted(hosted)) === JSON.stringify(sorted(ours))
      ? []
      : [{ job, hosted: sorted(hosted), local: sorted(ours) }];
  });
}

test("each stage runs with its CI job's environment", () => {
  expect(environmentDrift(STAGES, parseWorkflow(ci))).toEqual([]);
});

test("losing the Vitest heap limit on either side is drift", () => {
  const vitestEnv = {
    CI: "true",
    DO_NOT_TRACK: "1",
    E2E_TELEMETRY_DISABLED: "1",
    NODE_OPTIONS: "--max-old-space-size=2048",
  };
  const { NODE_OPTIONS: _, ...withoutHeap } = vitestEnv;
  const hostedWithout = ci.replace(
    /\n\s+env:\n\s+NODE_OPTIONS: "--max-old-space-size=2048"/,
    "",
  );
  expect(hostedWithout).not.toBe(ci);
  expect(environmentDrift(STAGES, parseWorkflow(hostedWithout))).toEqual([
    { job: "vitest", hosted: withoutHeap, local: vitestEnv },
  ]);
  const localWithout = STAGES.map((stage) =>
    stage.job === "vitest"
      ? { job: stage.job, commands: stage.commands }
      : stage,
  );
  expect(environmentDrift(localWithout, parseWorkflow(ci))).toEqual([
    { job: "vitest", hosted: vitestEnv, local: withoutHeap },
  ]);
});

test("every pinned action names its version", () => {
  const pinned = [...ci.matchAll(/^\s+(?:- )?uses: (\S+@[0-9a-f]{40})(.*)$/gm)];
  expect(pinned.length).toBeGreaterThan(0);
  expect(
    pinned
      .filter(([, , comment]) => !/^ # v\d+(\.\d+)*$/.test(comment ?? ""))
      .map(([, action]) => action),
  ).toEqual([]);
});
