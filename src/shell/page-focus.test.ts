import { expect, test } from "vitest";
import { resolvePageFocus } from "./page-focus";

test("TREE-7 every deferred body target yields to a focused text field", () => {
  for (const request of [
    { pageId: "a", target: "body" },
    { pageId: "a", target: "bodyStart" },
    { pageId: "a", target: "block", blockId: "two" },
  ] as const) {
    expect(
      resolvePageFocus(request, {
        kind: "bodyFocus",
        deferred: true,
        textFieldFocused: true,
      }),
    ).toBeNull();
    expect(
      resolvePageFocus(request, {
        kind: "bodyFocus",
        deferred: true,
        textFieldFocused: false,
      }),
    ).toEqual(request);
  }
});

test("TREE-7 immediate Return enters a mounted body from its focused title", () => {
  expect(
    resolvePageFocus(
      { pageId: "a", target: "bodyStart" },
      {
        kind: "bodyFocus",
        deferred: false,
        textFieldFocused: true,
      },
    ),
  ).toEqual({ pageId: "a", target: "bodyStart" });
  expect(
    resolvePageFocus(
      { pageId: "a", target: "body" },
      {
        kind: "bodyFocus",
        deferred: false,
        textFieldFocused: true,
      },
    ),
  ).toBeNull();
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
  for (const request of [
    { pageId: "a", target: "body" },
    { pageId: "a", target: "bodyStart" },
    { pageId: "a", target: "block", blockId: "two" },
  ] as const) {
    expect(
      resolvePageFocus(request, {
        kind: "titleFocus",
      }),
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
