import { cpus, loadavg } from "node:os";
import { readBudgets } from "./budgets.ts";

export function noiseLabel(
  load1: number,
  cores: number,
  maxLoadPerCore: number,
): string | undefined {
  if (load1 / cores <= maxLoadPerCore) return undefined;
  return `NOISY: load ${load1.toFixed(2)} on ${cores} cores, rerun on a quiet Mac before trusting this number`;
}

export function machineNoise(): string | undefined {
  return noiseLabel(
    loadavg()[0] ?? 0,
    cpus().length,
    readBudgets().noise.maxLoadPerCore.value,
  );
}

export function withNoise(line: string, noise: string | undefined): string {
  return noise ? `${line} | ${noise}` : line;
}
