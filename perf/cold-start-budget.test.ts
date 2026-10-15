import { expect, test } from "vitest";
import { readBudgets } from "./budgets.ts";
import { hostedNavBudget } from "./cold-start-budget.ts";

test("page and empty-workspace variants use separate hosted navigation calibrations", () => {
  const budgets = {
    ...readBudgets().coldStart,
    hostedNavToPaintMs: { value: 321, why: "empty workspace series" },
    hostedPageNavToPaintMs: { value: 654, why: "seeded page series" },
  };
  expect(hostedNavBudget(budgets, "index")).toEqual({
    value: 321,
    why: "empty workspace series",
  });
  expect(hostedNavBudget(budgets, "page")).toEqual({
    value: 654,
    why: "seeded page series",
  });
});
