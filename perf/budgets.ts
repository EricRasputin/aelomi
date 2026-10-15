import { readFileSync } from "node:fs";

export type Budget<T = number> = { value: T; why: string };
export type Budgets = {
  coldStart: {
    specMs: Budget;
    hostedCiMs: Budget;
    hostedNavToPaintMs: Budget;
    hostedPageNavToPaintMs: Budget;
    warmupRuns: Budget;
    timedRuns: Budget;
  };
  keystroke: {
    p95Ms: Budget;
    hostedAppOwnedP95Ms: Budget;
    pageLoads: Budget;
    samplesPerLoad: Budget;
    discardedPerLoad: Budget;
    gapMs: Budget;
  };
  noise: {
    maxLoadPerCore: Budget;
  };
  search: {
    p95Ms: Budget;
    hostedP95Ms: Budget;
    firstSamples: Budget;
  };
  palette: {
    p95Ms: Budget;
    hostedAppOwnedP50Ms: Budget;
    hostedAppOwnedP95Ms: Budget;
  };
  scroll: {
    p95FrameMs: Budget;
    hostedWorkP95Ms: Budget;
    maxFrameMs: Budget;
    hostedMaxFrameMs: Budget;
  };
};

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
  const search = section(raw.search, "search");
  const palette = section(raw.palette, "palette");
  const scroll = section(raw.scroll, "scroll");
  return {
    coldStart: {
      specMs: budget(coldStart, "coldStart.specMs", isNumber),
      hostedCiMs: budget(coldStart, "coldStart.hostedCiMs", isNumber),
      hostedNavToPaintMs: budget(
        coldStart,
        "coldStart.hostedNavToPaintMs",
        isNumber,
      ),
      hostedPageNavToPaintMs: budget(
        coldStart,
        "coldStart.hostedPageNavToPaintMs",
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
    search: {
      p95Ms: budget(search, "search.p95Ms", isNumber),
      hostedP95Ms: budget(search, "search.hostedP95Ms", isNumber),
      firstSamples: budget(search, "search.firstSamples", isNumber),
    },
    palette: {
      p95Ms: budget(palette, "palette.p95Ms", isNumber),
      hostedAppOwnedP50Ms: budget(
        palette,
        "palette.hostedAppOwnedP50Ms",
        isNumber,
      ),
      hostedAppOwnedP95Ms: budget(
        palette,
        "palette.hostedAppOwnedP95Ms",
        isNumber,
      ),
    },
    scroll: {
      p95FrameMs: budget(scroll, "scroll.p95FrameMs", isNumber),
      hostedWorkP95Ms: budget(scroll, "scroll.hostedWorkP95Ms", isNumber),
      maxFrameMs: budget(scroll, "scroll.maxFrameMs", isNumber),
      hostedMaxFrameMs: budget(scroll, "scroll.hostedMaxFrameMs", isNumber),
    },
  };
}
