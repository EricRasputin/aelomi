import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vitest";
import { STAGES } from "./check-all-stages.ts";

const ci = readFileSync(
  resolve(import.meta.dirname, "../.github/workflows/ci.yml"),
  "utf8",
);

test("the workflow is valid YAML", () => {
  const parsed = spawnSync("ruby", ["-ryaml", "-e", "YAML.parse(STDIN.read)"], {
    input: ci,
    encoding: "utf8",
  });
  expect({ status: parsed.status, error: parsed.stderr }).toEqual({
    status: 0,
    error: "",
  });
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
