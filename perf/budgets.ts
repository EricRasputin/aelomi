import { readFileSync } from "node:fs";

export type Budget<T = number> = { value: T; why: string };
export type KeystrokeSurface = "empty" | "page-5000-blocks";
export type Budgets = {
  coldStart: {
    specMs: Budget;
    hostedCiMs: Budget;
    hostedNavToPaintMs: Budget;
    warmupRuns: Budget;
    timedRuns: Budget;
  };
  keystroke: {
    p95Ms: Budget;
    hostedAppOwnedP95Ms: Budget;
    gate: Budget<KeystrokeSurface>;
    pageLoads: Budget;
    samplesPerLoad: Budget;
    discardedPerLoad: Budget;
    gapMs: Budget;
  };
  noise: {
    maxLoadPerCore: Budget;
  };
};

export const keystrokeSurfaces: readonly KeystrokeSurface[] = [
  "empty",
  "page-5000-blocks",
];

function section(raw: unknown, name: string): Record<string, unknown> {
  if (typeof raw !== "object" || raw === null)
    throw new Error(`budgets.json: ${name} must be an object`);
  return raw as Record<string, unknown>;
}

function budget<T>(
  raw: Record<string, unknown>,
  path: string,
  isValue: (value: unknown) => value is T,
): Budget<T> {
  const entry = section(raw[path.split(".").at(-1) ?? path], path);
  if (!isValue(entry.value) || typeof entry.why !== "string")
    throw new Error(`budgets.json: ${path} needs a value and a why`);
  return { value: entry.value, why: entry.why };
}

const isNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
const isSurface = (value: unknown): value is KeystrokeSurface =>
  keystrokeSurfaces.some((surface) => surface === value);

export function readBudgets(): Budgets {
  const raw = section(
    JSON.parse(
      readFileSync(new URL("./budgets.json", import.meta.url), "utf8"),
    ),
    "root",
  );
  const coldStart = section(raw.coldStart, "coldStart");
  const keystroke = section(raw.keystroke, "keystroke");
  const noise = section(raw.noise, "noise");
  return {
    coldStart: {
      specMs: budget(coldStart, "coldStart.specMs", isNumber),
      hostedCiMs: budget(coldStart, "coldStart.hostedCiMs", isNumber),
      hostedNavToPaintMs: budget(
        coldStart,
        "coldStart.hostedNavToPaintMs",
        isNumber,
      ),
      warmupRuns: budget(coldStart, "coldStart.warmupRuns", isNumber),
      timedRuns: budget(coldStart, "coldStart.timedRuns", isNumber),
    },
    keystroke: {
      p95Ms: budget(keystroke, "keystroke.p95Ms", isNumber),
      hostedAppOwnedP95Ms: budget(
        keystroke,
        "keystroke.hostedAppOwnedP95Ms",
        isNumber,
      ),
      gate: budget(keystroke, "keystroke.gate", isSurface),
      pageLoads: budget(keystroke, "keystroke.pageLoads", isNumber),
      samplesPerLoad: budget(keystroke, "keystroke.samplesPerLoad", isNumber),
      discardedPerLoad: budget(
        keystroke,
        "keystroke.discardedPerLoad",
        isNumber,
      ),
      gapMs: budget(keystroke, "keystroke.gapMs", isNumber),
    },
    noise: {
      maxLoadPerCore: budget(noise, "noise.maxLoadPerCore", isNumber),
    },
  };
}
