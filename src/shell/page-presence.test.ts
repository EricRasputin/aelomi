import { expect, test } from "vitest";
import type { TreePage } from "../ipc";
import type { SourceTree } from "../stores/page-tree";
import { bodyReady, presenceOf, visiblePages } from "./page-presence";

const pages: TreePage[] = [
  { id: "a", title: "A", parent_id: null, created_at: null, updated_at: null },
  { id: "b", title: "B", parent_id: "a", created_at: null, updated_at: null },
];
const source: SourceTree = {
  kind: "ready",
  tree: { pages, favorites: [], trash: [] },
  byId: new Map(pages.map((p) => [p.id, p])),
};
test("TREE-1 presence checks trash and new pages before the listed tree", () => {
  expect(presenceOf("a", source, new Map(), new Set(["a"]))).toEqual({
    kind: "trashed",
  });
  expect(
    presenceOf("a", source, new Map([["a", "creating"]]), new Set()),
  ).toEqual({ kind: "new", stage: "creating" });
  expect(presenceOf("b", source, new Map(), new Set())).toEqual({
    kind: "listed",
    fresh: true,
  });
});
test("TREE-1 only a successful create unlocks its body", () => {
  expect([
    bodyReady({ kind: "new", stage: "creating" }),
    bodyReady({ kind: "new", stage: "failed" }),
    bodyReady({ kind: "trashed" }),
    bodyReady({ kind: "new", stage: "created" }),
    bodyReady({ kind: "listed", fresh: false }),
  ]).toEqual([false, false, false, true, true]);
});
test("TREE-9 visible pages omit session trash and append new IDs in activation order", () => {
  expect(
    visiblePages(source, new Map([["n", "creating"]]), new Set(["b"])).map(
      (p) => p.id,
    ),
  ).toEqual(["a", "n"]);
});

test("SHELL-9 opening admission closes during Draining and reopens with Open", async () => {
  const { useQuitStore } = await import("../stores/quit");
  const { canOpenPage } = await import("./page-presence");
  useQuitStore.setState({ decision: { kind: "draining", revision: 1 } });
  expect(canOpenPage("a")).toBe(false);
  useQuitStore.setState({ decision: { kind: "open", revision: 2 } });
  expect(canOpenPage("a")).toBe(true);
});
