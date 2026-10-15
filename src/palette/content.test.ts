import { expect, test } from "vitest";
import { contentFromReply, contentScope, shownContent } from "./content";

const hit = {
  page_id: "page-a",
  title: [{ text: "A", highlighted: false }],
  block: { block_id: "b", text: [{ text: "x", highlighted: true }] },
};

test("maps search replies to what the content group shows", () => {
  expect(contentFromReply({ status: "ok", data: "Indexing" })).toEqual({
    kind: "indexing",
  });
  expect(
    contentFromReply({
      status: "ok",
      data: { Ready: { hits: [hit], issues: [] } },
    }),
  ).toEqual({ kind: "ready", hits: [hit], issues: [] });
  expect(
    contentFromReply({
      status: "error",
      error: { Failed: { reason: "read-only index folder" } },
    }),
  ).toEqual({ kind: "failed", reason: "read-only index folder" });
  expect(
    contentFromReply({
      status: "error",
      error: { InvalidQuery: { TooManyExcluded: { max: 200 } } },
    }),
  ).toEqual({
    kind: "error",
    message: "too many title results to exclude (at most 200)",
  });
  expect(contentFromReply({ status: "error", error: "Unavailable" })).toEqual({
    kind: "error",
    message: "Unavailable",
  });
});

test("a rebuilding index shows Indexing over any reply, and only Failed is an error", () => {
  const ready = { kind: "ready" as const, hits: [hit], issues: [] };
  expect(shownContent("Rebuilding", ready)).toEqual({ kind: "indexing" });
  expect(shownContent("Ready", { kind: "indexing" })).toEqual({
    kind: "indexing",
  });
  expect(shownContent({ Failed: { reason: "disk" } }, ready)).toEqual({
    kind: "failed",
    reason: "disk",
  });
  expect(shownContent("Rebuilding", { kind: "idle" })).toEqual({
    kind: "idle",
  });
  expect(shownContent("Ready", ready)).toBe(ready);
});

test("the content search excludes every page in the title group", () => {
  expect(
    contentScope([
      { id: "page-a", tier: "exact", parts: [] },
      { id: "page-b", tier: "prefix", parts: [] },
    ]),
  ).toEqual({ Content: { exclude: ["page-a", "page-b"] } });
});
