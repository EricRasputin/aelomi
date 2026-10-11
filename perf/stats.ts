export function median(values: readonly number[]): number {
  if (values.length === 0) throw new Error("median of no values");
  const sorted = [...values].sort((a, b) => a - b);
  const upper = sorted.length >> 1;
  const lower = (sorted.length - 1) >> 1;
  return ((sorted[lower] ?? 0) + (sorted[upper] ?? 0)) / 2;
}

export function percentile95(values: readonly number[]): number {
  if (values.length === 0) throw new Error("p95 of no values");
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.ceil(0.95 * sorted.length) - 1] ?? 0;
}

export function range(values: readonly number[]): string {
  return `${Math.min(...values).toFixed(1)} to ${Math.max(...values).toFixed(1)}`;
}
