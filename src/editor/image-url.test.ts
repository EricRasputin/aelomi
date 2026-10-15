import { expect, test } from "vitest";
import { localImageUrl } from "./image-url";

test("only data and blob URLs load, everything else resolves to nothing", () => {
  const cases = [
    "data:image/png;base64,iVBORw0KGgo=",
    "blob:http://localhost:1420/0b6f2f4e-1c1a-4c5e-9d6b-2a0d7b1c9e11",
    "https://example.com/x.png",
    "http://localhost:1420/x.png",
    "file:///Users/someone/x.png",
    "asset://localhost/x.png",
    "//example.com/x.png",
    "x.png",
    " data:image/png;base64,iVBORw0KGgo=",
    "DATA:image/png;base64,iVBORw0KGgo=",
    "javascript:alert(1)",
    "",
  ];
  expect(cases.map(localImageUrl)).toEqual([
    "data:image/png;base64,iVBORw0KGgo=",
    "blob:http://localhost:1420/0b6f2f4e-1c1a-4c5e-9d6b-2a0d7b1c9e11",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
  ]);
});
