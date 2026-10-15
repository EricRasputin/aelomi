import { expect, test } from "vitest";
import * as Y from "yjs";
import {
  type PageRecord,
  projectTree,
  readRecords,
  trashPage,
  writeRecords,
} from "./mock-workspace";

function rec(parentId: string | null, order: number): PageRecord {
  return {
    title: "",
    parentId,
    order,
    favorite: null,
    trash: null,
    createdAt: 1,
    updatedAt: 2,
  };
}
function trashed(record: PageRecord, batchId: string, trashedAt: number) {
  return {
    ...record,
    trash: {
      originalParentId: record.parentId,
      originalOrder: record.order,
      batchId,
      trashedAt,
    },
  };
}
function fav(record: PageRecord, order: number) {
  return { ...record, favorite: { order } };
}
const tree = (records: Record<string, PageRecord>) =>
  projectTree(new Map(Object.entries(records))).tree;
const ids = (records: Record<string, PageRecord>) =>
  tree(records).pages.map((page) => [page.id, page.parent_id]);

test("pages come depth first in sibling order, as tree.rs projects them", () => {
  const records = {
    b: rec(null, 1),
    a: rec(null, 2),
    a1: rec("a", 0),
    b2: rec("b", 5),
    b1: rec("b", 5),
    b1x: rec("b1", 0),
  };
  expect(ids(records)).toEqual([
    ["b", null],
    ["b1", "b"],
    ["b1x", "b1"],
    ["b2", "b"],
    ["a", null],
    ["a1", "a"],
  ]);
  expect(tree(records).pages[0]).toEqual({
    id: "b",
    title: "",
    parent_id: null,
    created_at: 1,
    updated_at: 2,
  });
});

test("trash hides the page and every descendant and lists only batch roots", () => {
  const records = {
    keep: fav(rec(null, 0), 2),
    gone: trashed(rec(null, 1), "batch-1", 10),
    "gone-child": fav(trashed(rec("gone", 0), "batch-1", 10), 1),
    "left-active": rec("gone-child", 0),
    later: trashed(rec("keep", 0), "batch-2", 20),
    inner: trashed(rec("gone", 1), "batch-0", 5),
  };
  const projected = tree(records);
  expect(projected.pages.map((page) => page.id)).toEqual(["keep"]);
  expect(projected.favorites).toEqual(["keep"]);
  expect(projected.trash).toEqual([
    { id: "later", title: "", batch_id: "batch-2", trashed_at: 20 },
    { id: "gone", title: "", batch_id: "batch-1", trashed_at: 10 },
    { id: "inner", title: "", batch_id: "batch-0", trashed_at: 5 },
  ]);
});

test("favorites sort by favorite order then ID", () => {
  expect(
    tree({
      c: fav(rec(null, 0), 1),
      a: fav(rec(null, 1), 1),
      b: fav(rec("c", 0), 0),
    }).favorites,
  ).toEqual(["b", "a", "c"]);
});

test("a missing parent or a cycle, including a self-parent, never hides a page", () => {
  expect(
    ids({
      orphan: rec("erased", 0),
      x: rec("z", 0),
      y: rec("x", 0),
      z: rec("y", 0),
      self: rec("self", 1),
    }),
  ).toEqual([
    ["orphan", null],
    ["x", null],
    ["y", "x"],
    ["z", "y"],
    ["self", null],
  ]);
});

test("records round-trip through the workspace doc, and trashing a page trashes its subtree as one batch", () => {
  const doc = new Y.Doc();
  writeRecords(doc, [
    { id: "projects", title: "Projects" },
    { id: "aelomi", title: "Aelomi", parentId: "projects" },
    { id: "daily", title: "Daily" },
  ]);
  trashPage(doc, "projects", "batch-1", 50);
  const records = readRecords(doc);
  expect(records.get("aelomi")?.trash).toEqual({
    originalParentId: "projects",
    originalOrder: 1,
    batchId: "batch-1",
    trashedAt: 50,
  });
  const projected = projectTree(records).tree;
  expect(projected.pages.map((page) => page.title)).toEqual(["Daily"]);
  expect(projected.trash.map((page) => page.id)).toEqual(["projects"]);
});

test("a record that breaks the DATA-2 table is not read", () => {
  const doc = new Y.Doc();
  writeRecords(doc, [
    { id: "good", title: "Good" },
    { id: "bad", title: "Bad" },
  ]);
  doc.getMap<Y.Map<unknown>>("pages").get("bad")?.set("placement", "nowhere");
  expect([...readRecords(doc).keys()]).toEqual(["good"]);
});
