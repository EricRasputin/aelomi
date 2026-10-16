import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import { readBudgets } from "./budgets.ts";
import { median, percentile95 } from "./stats.ts";

test("median of an even count is the mean of the two middle values", () => {
  expect(median([3, 1, 2])).toBe(2);
  expect(median([40, 10, 30, 20])).toBe(25);
  expect(median([10, 9, 8, 7, 6, 5, 4, 3, 2, 1])).toBe(5.5);
});

test("p95 uses the nearest rank", () => {
  const hundred = Array.from({ length: 100 }, (_, i) => i + 1);
  expect(percentile95(hundred)).toBe(95);
  const kept = Array.from({ length: 270 }, (_, i) => i + 1);
  expect(percentile95(kept)).toBe(257);
  expect(percentile95([5])).toBe(5);
});

test("budgets.json holds every gate with a value and a why", () => {
  const raw = JSON.parse(
    readFileSync(new URL("./budgets.json", import.meta.url), "utf8"),
  ) as Record<string, Record<string, { value?: unknown; why?: unknown }>>;
  const entries = Object.entries(raw).flatMap(([section, gates]) =>
    Object.entries(gates).map(([name, gate]) => ({
      key: `${section}.${name}`,
      ...gate,
    })),
  );
  expect(entries.map((entry) => entry.key)).toEqual([
    "coldStart.specMs",
    "coldStart.hostedCiMs",
    "coldStart.hostedNavToPaintMs",
    "coldStart.warmupRuns",
    "coldStart.timedRuns",
    "keystroke.p95Ms",
    "keystroke.hostedAppOwnedP95Ms",
    "keystroke.pageLoads",
    "keystroke.samplesPerLoad",
    "keystroke.discardedPerLoad",
    "keystroke.gapMs",
    "palette.p95Ms",
    "palette.hostedAppOwnedP50Ms",
    "palette.hostedAppOwnedP95Ms",
    "scroll.p95FrameMs",
    "scroll.hostedWorkP95Ms",
    "scroll.maxFrameMs",
    "scroll.hostedMaxFrameMs",
    "search.p95Ms",
    "search.hostedP95Ms",
    "search.firstSamples",
    "noise.maxLoadPerCore",
  ]);
  for (const entry of entries) {
    expect(entry.why, `${entry.key} why`).toMatch(/\S/);
    expect(typeof entry.value, `${entry.key} value`).toMatch(
      /^(number|string)$/,
    );
  }
  expect(readBudgets()).toEqual(raw);
});
