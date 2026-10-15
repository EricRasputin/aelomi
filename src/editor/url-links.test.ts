import { expect, test } from "vitest";
import { allowedUrl } from "./url-links";

test.each([
  "http://example.com",
  "https://example.com",
  "mailto:hello@example.com",
  "HTTPS://example.com/path?q=test#part",
])("allows a browser URL %s", (url) => {
  expect(allowedUrl(url)).toBe(true);
});
test.each([
  "javascript:alert(1)",
  "file:///tmp/secret",
  "data:text/html,test",
  "ftp://example.com",
  "/page/page-a",
  "not a URL",
  "tauri://localhost",
])("blocks a non-browser URL %s", (url) => {
  expect(allowedUrl(url)).toBe(false);
});
