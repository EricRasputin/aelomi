import { expect, test } from "vitest";
import { keystrokeMode, splitResidual, splitSample } from "./keystroke.ts";

const stamps = {
  down: 100,
  input: 101,
  raf: 111,
  rafMc: 114,
  doubleRaf: 128,
  shown: true,
};

test("a sample splits into the app's work and the tick wait", () => {
  expect(splitSample(stamps)).toEqual({
    keyToInput: 1,
    tickWait: 10,
    render: 3,
    appOwned: 4,
    full: 14,
    doubleRaf: 28,
  });
});

test("app-owned plus tick wait is the full key-to-paint", () => {
  expect(splitResidual(splitSample(stamps))).toBe(0);
  expect(
    splitResidual({
      keyToInput: 1,
      tickWait: 10,
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
