import { cpus, loadavg } from "node:os";
import { readBudgets } from "./budgets.ts";
import type { Mode } from "./keystroke.ts";

export function noiseLabel(
  load1: number,
  cores: number,
  maxLoadPerCore: number,
  mode: Mode,
): string | undefined {
  if (load1 / cores <= maxLoadPerCore) return undefined;
  const label = `NOISY: load ${load1.toFixed(2)} on ${cores} cores`;
  return mode === "local"
    ? `${label}, rerun on a quiet Mac before trusting this number`
    : label;
}

export function machineNoise(mode: Mode): string | undefined {
  return noiseLabel(
    loadavg()[0] ?? 0,
    cpus().length,
    readBudgets().noise.maxLoadPerCore.value,
    mode,
  );
}

export function withNoise(line: string, noise: string | undefined): string {
  return noise ? `${line} | ${noise}` : line;
}
