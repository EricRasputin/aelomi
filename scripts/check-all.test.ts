import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vitest";
import { STAGES } from "./check-all-stages.ts";

test("check:all runs a stage for every CI job and no other", () => {
  const ci = readFileSync(
    resolve(import.meta.dirname, "../.github/workflows/ci.yml"),
    "utf8",
  );
  const jobs = [
    ...(ci.split(/^jobs:$/m)[1] ?? "").matchAll(/^ {2}([\w-]+):$/gm),
  ].map((match) => match[1]);
  expect(jobs.length).toBeGreaterThan(0);
  expect(STAGES.map((stage) => stage.job).sort()).toEqual(jobs.sort());
});
