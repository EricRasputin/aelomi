import { afterEach, beforeEach, expect, test, vi } from "vitest";

const reports: { variant: string; commit: number; raf2: number }[] = [];

vi.mock("../ipc", () => ({
  commands: {
    perfReport: async (report: {
      variant: string;
      commit: number;
      raf2: number;
    }) => {
      reports.push(report);
    },
  },
}));

let frames: FrameRequestCallback[] = [];

async function nextFrame() {
  const due = frames.splice(0);
  for (const frame of due) frame(0);
  await new Promise((done) => setTimeout(done, 5));
}

async function load() {
  vi.resetModules();
  return import("./cold-start");
}

beforeEach(() => {
  reports.length = 0;
  frames = [];
  vi.stubGlobal("window", { __AELOMI_PERF__: true });
  vi.stubGlobal("requestAnimationFrame", (frame: FrameRequestCallback) => {
    frames.push(frame);
    return frames.length;
  });
});
afterEach(() => vi.unstubAllGlobals());

test("the entry's variant reports once, after its second frame and task", async () => {
  const coldStart = await load();
  coldStart.markIndexCommit();
  coldStart.resolveEntry("page");
  await nextFrame();
  await nextFrame();
  expect(reports).toEqual([]);
  coldStart.markPageContent();
  await nextFrame();
  expect(reports).toEqual([]);
  await nextFrame();
  expect(reports.map((report) => report.variant)).toEqual(["page"]);
  coldStart.markPageContent();
  coldStart.resolveEntry("index");
  await nextFrame();
  await nextFrame();
  expect(reports).toHaveLength(1);
});

test("marks that finish before the entry resolves wait for it", async () => {
  const coldStart = await load();
  coldStart.markPageContent();
  await nextFrame();
  await nextFrame();
  expect(reports).toEqual([]);
  coldStart.resolveEntry("page");
  expect(reports.map((report) => report.variant)).toEqual(["page"]);
  const [report] = reports;
  expect(report && report.commit <= report.raf2).toBe(true);
});

test("nothing is measured unless the perf report is armed", async () => {
  vi.stubGlobal("window", {});
  const coldStart = await load();
  coldStart.markPageContent();
  coldStart.resolveEntry("page");
  await nextFrame();
  await nextFrame();
  expect(frames).toEqual([]);
  expect(reports).toEqual([]);
});
