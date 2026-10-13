import { expect, test } from "vitest";
import { contrastRatio, type Rgb } from "./contrast";

const white: Rgb = [255, 255, 255];

test.each<[Rgb, number]>([
  [[0, 0, 0], 21],
  [[0x76, 0x76, 0x76], 4.54],
  [[0x77, 0x77, 0x77], 4.48],
  [white, 1],
])("contrastRatio(%j, white) is %d", (color, expected) => {
  expect(contrastRatio(color, white)).toBeCloseTo(expected, 2);
  expect(contrastRatio(white, color)).toBeCloseTo(expected, 2);
});
