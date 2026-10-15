// Every stage runs with this on top of the caller's environment. CI makes Vitest
// and Playwright reject a leftover test.only, as GitHub's runners set it too.
export const LOCAL_ENV: Readonly<Record<string, string>> = {
  CI: "true",
  E2E_TELEMETRY_DISABLED: "1",
  DO_NOT_TRACK: "1",
};

export type Stage = {
  job: string;
  commands: string[][];
  env?: Record<string, string>;
  perf?: true;
};

// One stage per job in .github/workflows/ci.yml, named after it, running that job's
// commands with that job's environment. check-all.test.ts fails when the jobs, their
// commands or their environment drift. e2e builds dist-mock/ once for playwright and
// keystroke, and build-app builds the bundle cold-start launches, so the order matters.
// Perf stages gate CI's runner-calibrated budgets, not the spec numbers, and wait for a
// quiet machine first. pnpm perf:cold and pnpm perf:keystroke on their own check the
// spec numbers.
export const STAGES: readonly Stage[] = [
  { job: "biome", commands: [["pnpm", "lint"]] },
  { job: "tsc", commands: [["pnpm", "typecheck"]] },
  {
    job: "vitest",
    commands: [["pnpm", "test"]],
    env: { NODE_OPTIONS: "--max-old-space-size=2048" },
  },
  {
    job: "pnpm-audit",
    commands: [
      ["pnpm", "audit", "--audit-level", "moderate"],
      ["pnpm", "check-licenses"],
    ],
  },
  {
    job: "nextest",
    commands: [
      ["cargo", "nextest", "run", "--workspace", "--locked"],
      [
        "cargo",
        "nextest",
        "run",
        "-p",
        "aelomi-core",
        "--cargo-profile",
        "release",
        "--locked",
        "--run-ignored",
        "only",
        "--test-threads",
        "1",
        "index::tests::latency",
      ],
      ["node", "scripts/check-search.ts", "--ci"],
    ],
  },
  { job: "cargo-deny", commands: [["cargo", "deny", "--locked", "check"]] },
  {
    job: "e2e",
    commands: [
      ["pnpm", "exec", "e2e-web", "install", "webkit"],
      ["pnpm", "build:mock"],
      ["pnpm", "test:e2e"],
    ],
  },
  {
    job: "playwright",
    commands: [
      ["pnpm", "exec", "playwright", "install", "webkit"],
      ["pnpm", "test:playwright"],
    ],
  },
  {
    job: "keystroke",
    commands: [["pnpm", "perf:keystroke"]],
    env: { AELOMI_PERF_CI: "1" },
    perf: true,
  },
  {
    job: "build-app",
    commands: [
      ["pnpm", "tauri", "build", "--bundles", "app", "--", "--locked"],
      ["pnpm", "check:no-mock"],
    ],
  },
  {
    job: "cold-start",
    commands: [
      ["node", "scripts/wait-for-quiet.ts"],
      [
        "node",
        "perf/cold-start.ts",
        "--ci",
        "--expected-variant",
        "index",
        "--out",
        "perf-results/cold-start-index",
      ],
      ["node", "scripts/wait-for-quiet.ts"],
      [
        "node",
        "perf/cold-start.ts",
        "--ci",
        "--expected-variant",
        "page",
        "--out",
        "perf-results/cold-start-page",
      ],
    ],
  },
];
