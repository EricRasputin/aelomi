import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { arch, cpus, loadavg, platform, release } from "node:os";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { setTimeout } from "node:timers/promises";
import { parseArgs } from "node:util";

type ProcessSample = { rssKiB: number; started: string };
type StopReason = "duration" | "exited" | "pid-reused";

export function parseProcessSample(output: string): ProcessSample {
  const match = output.trim().match(/^(\d+)\s+(.+)$/);
  if (!match) throw new Error("ps did not return RSS and process start time");
  const rssKiB = Number(match[1]);
  const started = match[2];
  if (!Number.isSafeInteger(rssKiB) || rssKiB < 0 || !started) {
    throw new Error("ps returned an invalid process sample");
  }
  return { rssKiB, started };
}

function readProcess(pid: number): ProcessSample | null {
  const sampled = spawnSync(
    "/bin/ps",
    ["-p", String(pid), "-o", "rss=", "-o", "lstart="],
    { encoding: "utf8", env: { ...process.env, LC_ALL: "C" } },
  );
  if (sampled.error) throw sampled.error;
  if (sampled.status === 1 && sampled.stdout.trim() === "") return null;
  if (sampled.status !== 0) {
    throw new Error(`ps failed with exit ${sampled.status}: ${sampled.stderr}`);
  }
  return parseProcessSample(sampled.stdout);
}

export async function sampleNativeProcess({
  pid,
  durationMs,
  intervalMs,
}: {
  pid: number;
  durationMs: number;
  intervalMs: number;
}) {
  for (const [name, value] of Object.entries({ pid, durationMs, intervalMs })) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new Error(`${name} must be a positive integer`);
    }
  }
  const context = {
    recordedAt: new Date().toISOString(),
    platform: platform(),
    osRelease: release(),
    architecture: arch(),
    cpuCount: cpus().length,
    loadBefore: loadavg(),
    nodeVersion: process.version,
  };
  const startedAt = performance.now();
  const samples: { elapsedMs: number; rssKiB: number }[] = [];
  let identity: string | undefined;
  let stopReason: StopReason = "duration";
  while (performance.now() - startedAt < durationMs) {
    const sample = readProcess(pid);
    if (!sample) {
      stopReason = "exited";
      break;
    }
    identity ??= sample.started;
    if (identity !== sample.started) {
      stopReason = "pid-reused";
      break;
    }
    samples.push({
      elapsedMs: performance.now() - startedAt,
      rssKiB: sample.rssKiB,
    });
    const nextAt = startedAt + samples.length * intervalMs;
    const remaining = startedAt + durationMs - performance.now();
    if (remaining > 0) {
      await setTimeout(
        Math.min(remaining, Math.max(1, nextAt - performance.now())),
      );
    }
  }
  return {
    context: { ...context, loadAfter: loadavg() },
    measurement:
      "Observed main-process RSS after sampler attachment; excludes WebKit helpers",
    intervalMs,
    durationMs,
    elapsedMs: performance.now() - startedAt,
    stopReason,
    observedPeakKiB: samples.reduce(
      (peak, sample) => Math.max(peak, sample.rssKiB),
      0,
    ),
    samples,
  };
}

async function main() {
  const { values } = parseArgs({
    options: {
      pid: { type: "string" },
      out: { type: "string" },
      "duration-ms": { type: "string", default: "30000" },
      "interval-ms": { type: "string", default: "100" },
    },
  });
  if (!values.pid || !values.out) {
    throw new Error(
      "Usage: node perf/native-process.ts --pid <owned app PID> --out <record.json>",
    );
  }
  const report = await sampleNativeProcess({
    pid: Number(values.pid),
    durationMs: Number(values["duration-ms"]),
    intervalMs: Number(values["interval-ms"]),
  });
  writeFileSync(values.out, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({
      samples: report.samples.length,
      observedPeakKiB: report.observedPeakKiB,
      stopReason: report.stopReason,
    }),
  );
  if (report.samples.length === 0 || report.stopReason === "pid-reused")
    process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === import.meta.filename)
  await main();
