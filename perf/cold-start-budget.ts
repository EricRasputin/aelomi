import type { Budget, Budgets } from "./budgets.ts";

export function hostedNavBudget(
  budgets: Budgets["coldStart"],
  variant: "index" | "page",
): Budget {
  return variant === "page"
    ? budgets.hostedPageNavToPaintMs
    : budgets.hostedNavToPaintMs;
}

export function hostedEndToEndBudget(
  budgets: Budgets["coldStart"],
  variant: "index" | "page",
): Budget {
  return variant === "page" ? budgets.hostedPageCiMs : budgets.hostedCiMs;
}
