import { spawn } from "node:child_process";
import { once } from "node:events";
import { expect, test } from "vitest";
import { parseProcessSample, sampleNativeProcess } from "./native-process.ts";

test("RSS is recorded in KiB and process identity is retained", () => {
  expect(parseProcessSample("  81920 Wed Oct 14 14:30:10 2026\n")).toEqual({
    rssKiB: 81920,
    started: "Wed Oct 14 14:30:10 2026",
  });
  expect(() => parseProcessSample("unknown")).toThrow(
    "RSS and process start time",
  );
});

test("samples an owned process and stops when it exits", async () => {
  const child = spawn(process.execPath, [
    "-e",
    "process.stdout.write('ready'); setInterval(() => {}, 1000)",
  ]);
  try {
    await once(child.stdout, "data");
    if (!child.pid) throw new Error("The sample process did not start");
    const sampled = await sampleNativeProcess({
      pid: child.pid,
      durationMs: 120,
      intervalMs: 30,
    });
    expect(sampled.stopReason).toBe("duration");
    expect(sampled.samples.length).toBeGreaterThan(0);
    expect(sampled.samples.every((sample) => sample.rssKiB > 0)).toBe(true);
    expect(sampled.observedPeakKiB).toBe(
      Math.max(...sampled.samples.map((sample) => sample.rssKiB)),
    );
    const exited = once(child, "exit");
    child.kill("SIGTERM");
    await exited;
    const after = await sampleNativeProcess({
      pid: child.pid,
      durationMs: 120,
      intervalMs: 30,
    });
    expect(after.stopReason).toBe("exited");
    expect(after.samples).toEqual([]);
  } finally {
    if (child.exitCode === null && child.signalCode === null)
      child.kill("SIGTERM");
  }
});
