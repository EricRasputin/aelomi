import { expect, test } from "vitest";
import { noiseLabel, withNoise } from "./noise.ts";

test("a quiet machine gets no label, and the same load against a stricter limit does", () => {
  expect(noiseLabel(1.2, 8, 0.8)).toBeUndefined();
  expect(noiseLabel(1.2, 8, 0.1)).toBe(
    "NOISY: load 1.20 on 8 cores, rerun on a quiet Mac before trusting this number",
  );
});

test("load exactly at the per-core limit is quiet and one hundredth above is noisy", () => {
  expect(noiseLabel(6.4, 8, 0.8)).toBeUndefined();
  expect(noiseLabel(6.41, 8, 0.8)).toBe(
    "NOISY: load 6.41 on 8 cores, rerun on a quiet Mac before trusting this number",
  );
});

test("a busy machine gets the one-line label", () => {
  expect(noiseLabel(12.3, 8, 0.8)).toBe(
    "NOISY: load 12.30 on 8 cores, rerun on a quiet Mac before trusting this number",
  );
});

test("the label rides on the verdict line only when there is one", () => {
  expect(withNoise("cold start: PASS", "NOISY: load 12.30 on 8 cores")).toBe(
    "cold start: PASS | NOISY: load 12.30 on 8 cores",
  );
  expect(withNoise("cold start: PASS", undefined)).toBe("cold start: PASS");
});
