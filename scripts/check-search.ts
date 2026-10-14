import { spawnSync } from "node:child_process";
import { parseArgs } from "node:util";
import { readBudgets } from "../perf/budgets.ts";

const { values } = parseArgs({ options: { ci: { type: "boolean" } } });
const { search } = readBudgets();
const budget = values.ci ? search.hostedP95Ms : search.p95Ms;
if (budget.value <= 0)
  throw new Error("The search p95 budget must be positive");
if (
  !Number.isSafeInteger(search.firstSamples.value) ||
  search.firstSamples.value < 30
) {
  throw new Error("The search gate requires at least 30 first-query samples");
}
console.log(
  `Search gate ${values.ci ? "CI" : "product"} p95 < ${budget.value} ms. ${budget.why}`,
);
const result = spawnSync(
  "cargo",
  [
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
    "--no-capture",
    "index::tests::search_gate",
  ],
  {
    stdio: "inherit",
    env: {
      ...process.env,
      AELOMI_SEARCH_GATE_BUDGET_MS: String(budget.value),
      AELOMI_SEARCH_GATE_FIRST_SAMPLES: String(search.firstSamples.value),
    },
  },
);
if (result.error)
  throw new Error("Could not run the release search gate", {
    cause: result.error,
  });
process.exitCode = result.status ?? 1;
