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

test("budgets.json holds every gate with a why", () => {
  const budgets = readBudgets();
  expect(budgets.coldStart.specMs.value).toBe(500);
  expect(budgets.keystroke.p95Ms.value).toBe(16);
  expect(budgets.coldStart.timedRuns.value).toBe(10);
  expect(budgets.coldStart.hostedCiMs.why).not.toBe("");
});
