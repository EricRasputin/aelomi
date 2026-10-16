import { expect, test } from "vitest";
import { resolvePageFocus } from "./page-focus";

test("TREE-7 body focus yields to a focused text field, while Return can enter the body", () => {
  expect(
    resolvePageFocus(
      { pageId: "a", target: "body" },
      {
        kind: "bodyMount",
        textFieldFocused: true,
      },
    ),
  ).toBeNull();
  expect(
    resolvePageFocus(
      { pageId: "a", target: "body" },
      {
        kind: "bodyMount",
        textFieldFocused: false,
      },
    ),
  ).toEqual({ pageId: "a", target: "body" });
  expect(
    resolvePageFocus(
      { pageId: "a", target: "bodyStart" },
      {
        kind: "bodyMount",
        textFieldFocused: true,
      },
    ),
  ).toEqual({ pageId: "a", target: "bodyStart" });
});

test("TREE-1 gated Return is dropped and a title focus cancels pending body targets", () => {
  expect(
    resolvePageFocus(
      { pageId: "a", target: "bodyStart" },
      {
        kind: "request",
        bodyReady: false,
      },
    ),
  ).toBeNull();
  expect(
    resolvePageFocus(
      { pageId: "a", target: "body" },
      {
        kind: "request",
        bodyReady: false,
      },
    ),
  ).toEqual({ pageId: "a", target: "title" });
  for (const target of ["body", "bodyStart"] as const) {
    expect(
      resolvePageFocus(
        { pageId: "a", target },
        {
          kind: "titleFocus",
        },
      ),
    ).toBeNull();
  }
  expect(
    resolvePageFocus(
      { pageId: "a", target: "title" },
      {
        kind: "titleFocus",
      },
    ),
  ).toEqual({ pageId: "a", target: "title" });
});
