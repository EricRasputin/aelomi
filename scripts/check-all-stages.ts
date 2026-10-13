// One stage per job in .github/workflows/ci.yml, named after it. check-all.test.ts fails
// when the two lists drift. e2e builds dist-mock/ once for playwright and keystroke, and
// build-app builds the bundle cold-start launches, so the order matters. Perf stages gate
// CI's runner-calibrated budgets, not the spec numbers, and wait for a quiet machine first.
// pnpm perf:cold and pnpm perf:keystroke on their own check the spec numbers.
export const STAGES: readonly {
  job: string;
  commands: string[][];
  perf?: true;
}[] = [
  { job: "biome", commands: [["pnpm", "lint"]] },
  { job: "tsc", commands: [["pnpm", "typecheck"]] },
  { job: "vitest", commands: [["pnpm", "test"]] },
  {
    job: "pnpm-audit",
    commands: [
      ["pnpm", "audit", "--audit-level", "moderate"],
      ["pnpm", "check-licenses"],
    ],
  },
  {
    job: "nextest",
    commands: [["cargo", "nextest", "run", "--workspace", "--locked"]],
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
    commands: [["env", "AELOMI_PERF_CI=1", "pnpm", "perf:keystroke"]],
    perf: true,
  },
  {
    job: "build-app",
    commands: [
      ["pnpm", "tauri", "build", "--bundles", "app", "--", "--locked"],
      ["pnpm", "check:no-mock"],
    ],
  },
  { job: "cold-start", commands: [["pnpm", "perf:cold", "--ci"]], perf: true },
];
