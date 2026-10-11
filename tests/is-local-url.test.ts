import { expect, test } from "vitest";
import { isLocalUrl } from "./is-local-url";

test.each([
  ["http://127.0.0.1:4173/", true],
  ["http://localhost:4173/assets/index.js", true],
  ["http://[::1]:4173/", true],
  ["data:text/plain,hi", true],
  ["blob:http://127.0.0.1:4173/8c2b", true],
  ["about:blank", true],
  ["https://example.com/", false],
  ["ws://example.com/socket", false],
  ["http://127.0.0.1.example.com/", false],
  ["http://localhost.example.com/", false],
  ["http://10.0.0.1/", false],
])("isLocalUrl(%s) is %s", (url, expected) => {
  expect(isLocalUrl(url)).toBe(expected);
});
