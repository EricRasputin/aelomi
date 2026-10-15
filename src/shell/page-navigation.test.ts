import { expect, test } from "vitest";
import type { TreePage } from "../ipc";
import { flattenPages, pageAfterTrash } from "./page-navigation";

const page = (id: string, parent_id: string | null = null): TreePage => ({
  id,
  parent_id,
  title: id,
  created_at: 1,
  updated_at: 1,
});
const before = [
  page("projects"),
  page("a", "projects"),
  page("nested", "a"),
  page("b", "projects"),
  page("c", "projects"),
  page("other"),
];
const pick = (
  pageId: string,
  ids: string[],
  recents: [string, number][] = [],
) =>
  pageAfterTrash({
    pageId,
    before,
    active: before.filter((page) => ids.includes(page.id)),
    openedAt: new Map(recents),
  });

test("TREE-6 visible rows retain source order and omit collapsed descendants", () => {
  expect(
    flattenPages(before, new Set(["projects"])).map(
      ({ page, depth, hasChildren }) => [page.id, depth, hasChildren],
    ),
  ).toEqual([
    ["projects", 0, true],
    ["a", 1, true],
    ["b", 1, false],
    ["c", 1, false],
    ["other", 0, false],
  ]);
  expect(
    flattenPages(before, new Set(["projects", "a"])).map((row) => [
      row.page.id,
      row.depth,
    ]),
  ).toEqual([
    ["projects", 0],
    ["a", 1],
    ["nested", 2],
    ["b", 1],
    ["c", 1],
    ["other", 0],
  ]);
  expect(flattenPages(before, new Set()).map((row) => row.page.id)).toEqual([
    "projects",
    "other",
  ]);
});

test("TREE-6 flattening uses an explicit stack for deep source trees", () => {
  const pages = Array.from({ length: 10000 }, (_, i) =>
    page(`page-${i}`, i ? `page-${i - 1}` : null),
  );
  const rows = flattenPages(pages, new Set(pages.map((page) => page.id)));
  expect([rows.length, rows.at(-1)?.page.id, rows.at(-1)?.depth]).toEqual([
    10000,
    "page-9999",
    9999,
  ]);
});

test("TREE-9 trash opens the next active sibling before ancestor or recent page", () => {
  expect(pick("a", ["projects", "b", "other"], [["other", 20]])).toBe("b");
  expect(pick("a", ["projects", "c", "other"], [["other", 20]])).toBe("c");
});

test("TREE-9 trash opens the nearest active ancestor when there is no next sibling", () => {
  expect(pick("nested", ["projects", "a", "other"], [["other", 20]])).toBe("a");
  expect(pick("nested", ["projects", "other"], [["other", 20]])).toBe(
    "projects",
  );
});

test("TREE-9 trash opens the most recently opened active page after siblings and ancestors", () => {
  expect(
    pick(
      "c",
      ["a", "other"],
      [
        ["c", 100],
        ["a", 5],
        ["other", 20],
      ],
    ),
  ).toBe("other");
});

test("TREE-9 trash shows the empty state when no active fallback was opened", () => {
  expect(pick("c", [], [["c", 100]])).toBe(null);
  expect(pick("c", ["other"], [["other", 20]])).toBe("other");
  expect(pick("c", ["a"])).toBe(null);
});
