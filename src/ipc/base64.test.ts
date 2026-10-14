import { Buffer } from "node:buffer";
import { expect, test } from "vitest";
import { fromBase64, toBase64 } from "./base64";

test("matches the standard base64 alphabet and padding", () => {
  expect(toBase64(new TextEncoder().encode("foob"))).toBe("Zm9vYg==");
  expect(toBase64(Uint8Array.of(0, 255, 62, 63))).toBe("AP8+Pw==");
  expect(fromBase64("Zm9vYmE=")).toEqual(new TextEncoder().encode("fooba"));
  expect(toBase64(new Uint8Array())).toBe("");
});

test("round-trips random bytes across chunk boundaries", () => {
  for (const length of [1, 2, 3, 0x7fff, 0x8000, 0x8001, 1_000_000]) {
    const bytes = new Uint8Array(length);
    for (let i = 0; i < length; i += 65_536)
      crypto.getRandomValues(bytes.subarray(i, i + 65_536));
    expect(Buffer.compare(fromBase64(toBase64(bytes)), bytes)).toBe(0);
  }
});

test("rejects text that is not base64", () => {
  expect(() => fromBase64("not base64!")).toThrow();
});
