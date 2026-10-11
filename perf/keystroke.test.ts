import { expect, test } from "vitest";
import { keystrokeMode, splitResidual, splitSample } from "./keystroke.ts";

const stamps = {
  down: 100,
  input: 101,
  frame: 111,
  raf: 111,
  rafMc: 114,
  doubleRaf: 128,
  shown: true,
};

test("a sample splits into the app's work and the tick wait", () => {
  expect(splitSample(stamps)).toEqual({
    keyToInput: 1,
    tickWait: 10,
    frameToRaf: 0,
    render: 3,
    appOwned: 4,
    full: 14,
    doubleRaf: 28,
  });
});

test("work an earlier rAF callback does in the same frame is app-owned", () => {
  expect(
    splitSample({
      ...stamps,
      frame: 111,
      raf: 141,
      rafMc: 144,
      doubleRaf: 158,
    }),
  ).toEqual({
    keyToInput: 1,
    tickWait: 10,
    frameToRaf: 30,
    render: 33,
    appOwned: 34,
    full: 44,
    doubleRaf: 58,
  });
});

test("app-owned plus tick wait is the full key-to-paint", () => {
  expect(splitResidual(splitSample(stamps))).toBe(0);
  expect(
    splitResidual({
      keyToInput: 1,
      tickWait: 10,
      frameToRaf: 0,
      render: 3,
      appOwned: 4,
      full: 15,
      doubleRaf: 28,
    }),
  ).toBe(1);
});

test("AELOMI_PERF_CI=1 selects the hosted gate", () => {
  expect(keystrokeMode({ AELOMI_PERF_CI: "1" })).toBe("ci");
  expect(keystrokeMode({})).toBe("local");
  expect(keystrokeMode({ CI: "true" })).toBe("local");
});
