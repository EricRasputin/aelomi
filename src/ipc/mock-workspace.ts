import * as Y from "yjs";
import { rootContent } from "../editor/yjs-roots";
import type { PageTree, TrashedPage, TreePage } from "./bindings";

export type PageRecord = {
  title: string;
  parentId: string | null;
  order: number;
  favorite: { order: number } | null;
  trash: {
    originalParentId: string | null;
    originalOrder: number;
    batchId: string;
    trashedAt: number;
  } | null;
  createdAt: number;
  updatedAt: number;
};

export type SeedPage = { id: string; title: string } & Partial<
  Omit<PageRecord, "title">
>;

export const workspaceDocId = "workspace";
const pagesRoot = "pages";

export function writeRecords(doc: Y.Doc, seeds: readonly SeedPage[]) {
  const pages = doc.getMap<Y.Map<unknown>>(pagesRoot);
  doc.transact(() => {
    seeds.forEach((seed, index) => {
      const record: PageRecord = {
        title: seed.title,
        parentId: seed.parentId ?? null,
        order: seed.order ?? index,
        favorite: seed.favorite ?? null,
        trash: seed.trash ?? null,
        createdAt: seed.createdAt ?? 1_700_000_000_000 + index * 1000,
        updatedAt: seed.updatedAt ?? 1_700_000_000_000 + index * 1000,
      };
      let map = pages.get(seed.id);
      if (!(map instanceof Y.Map)) {
        map = new Y.Map<unknown>();
        pages.set(seed.id, map);
      }
      map.set("title", record.title);
      map.set("placement", { parentId: record.parentId, order: record.order });
      map.set("favorite", record.favorite);
      map.set("trash", record.trash);
      map.set("createdAt", record.createdAt);
      map.set("updatedAt", record.updatedAt);
    });
  });
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function integer(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : null;
}

function optionalPage(value: unknown): string | null | undefined {
  if (value === null) return null;
  return typeof value === "string" && value !== "" ? value : undefined;
}

function readRecord(map: unknown): PageRecord | null {
  if (!(map instanceof Y.Map)) return null;
  const title = map.get("title");
  const placement = map.get("placement");
  const favorite = map.get("favorite") ?? null;
  const trash = map.get("trash") ?? null;
  const createdAt = integer(map.get("createdAt"));
  const updatedAt = integer(map.get("updatedAt"));
  if (typeof title !== "string" || !isObject(placement)) return null;
  const parentId = optionalPage(placement.parentId);
  const order = integer(placement.order);
  if (parentId === undefined || order === null) return null;
  if (createdAt === null || updatedAt === null) return null;
  let favoriteRecord: PageRecord["favorite"] = null;
  if (favorite !== null) {
    const favoriteOrder = isObject(favorite) ? integer(favorite.order) : null;
    if (favoriteOrder === null) return null;
    favoriteRecord = { order: favoriteOrder };
  }
  let trashRecord: PageRecord["trash"] = null;
  if (trash !== null) {
    if (!isObject(trash)) return null;
    const originalParentId = optionalPage(trash.originalParentId);
    const originalOrder = integer(trash.originalOrder);
    const trashedAt = integer(trash.trashedAt);
    const batchId = trash.batchId;
    if (
      originalParentId === undefined ||
      originalOrder === null ||
      trashedAt === null ||
      typeof batchId !== "string" ||
      batchId === ""
    )
      return null;
    trashRecord = { originalParentId, originalOrder, batchId, trashedAt };
  }
  return {
    title,
    parentId,
    order,
    favorite: favoriteRecord,
    trash: trashRecord,
    createdAt,
    updatedAt,
  };
}

/**
 * Mirrors `pages_root` in `crates/core/src/workspace/records.rs`: a pages
 * root that holds sequence content, or a populated root of another shared
 * type, has no records this build can read.
 */
export function pagesRootProblem(doc: Y.Doc): string | null {
  const root = doc.share.get(pagesRoot);
  if (!root) return null;
  const { sequence, entries } = rootContent(root);
  if (sequence.length > 0)
    return "the pages root is not a Y.Map, found a sequence";
  const mapLike = root instanceof Y.Map || root.constructor === Y.AbstractType;
  if (!mapLike && entries.length > 0)
    return "the pages root is not a Y.Map, found another shared type";
  return null;
}

export function readRecords(doc: Y.Doc): Map<string, PageRecord> {
  const records = new Map<string, PageRecord>();
  if (pagesRootProblem(doc) !== null) return records;
  for (const [id, map] of doc.getMap(pagesRoot)) {
    const record = readRecord(map);
    if (record) records.set(id, record);
  }
  return records;
}

const byId = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/**
 * The tree `crates/core/src/workspace/tree.rs` projects, written
 * independently: a missing parent makes a page top-level, a parent cycle is
 * cut at its smallest page ID, and a page is active when neither it nor any
 * ancestor is trashed.
 */
export function projectTree(records: ReadonlyMap<string, PageRecord>): {
  tree: PageTree;
  parentOf: ReadonlyMap<string, string | null>;
} {
  const ids = [...records.keys()].sort(byId);
  const parentOf = new Map<string, string | null>();
  for (const id of ids) {
    const parent = records.get(id)?.parentId ?? null;
    parentOf.set(id, parent !== null && records.has(parent) ? parent : null);
  }
  const settled = new Set<string>();
  for (const start of ids) {
    const path: string[] = [];
    let page: string | null = start;
    while (page !== null && !settled.has(page)) {
      const at = path.indexOf(page);
      if (at >= 0) {
        const cut = path.slice(at).sort(byId)[0];
        if (cut !== undefined) parentOf.set(cut, null);
        break;
      }
      path.push(page);
      page = parentOf.get(page) ?? null;
    }
    for (const visited of path) settled.add(visited);
  }
  const activeMemo = new Map<string, boolean>();
  const isActive = (id: string): boolean => {
    const path: string[] = [];
    let page: string | null = id;
    let active = true;
    while (page !== null) {
      const known = activeMemo.get(page);
      if (known !== undefined) {
        active = known;
        break;
      }
      path.push(page);
      if (records.get(page)?.trash !== null) {
        active = false;
        break;
      }
      page = parentOf.get(page) ?? null;
    }
    for (const visited of path) activeMemo.set(visited, active);
    return active;
  };
  const children = new Map<string | null, string[]>();
  for (const id of ids) {
    if (!isActive(id)) continue;
    const parent = parentOf.get(id) ?? null;
    children.set(parent, [...(children.get(parent) ?? []), id]);
  }
  const order = (id: string) => records.get(id)?.order ?? 0;
  for (const siblings of children.values())
    siblings.sort((a, b) => order(a) - order(b) || byId(a, b));
  const pages: TreePage[] = [];
  const stack = [...(children.get(null) ?? [])].reverse();
  while (stack.length > 0) {
    const id = stack.pop();
    const record = id === undefined ? undefined : records.get(id);
    if (id === undefined || !record) continue;
    pages.push({
      id,
      title: record.title,
      parent_id: parentOf.get(id) ?? null,
      created_at: record.createdAt,
      updated_at: record.updatedAt,
    });
    stack.push(...[...(children.get(id) ?? [])].reverse());
  }
  const favorites = ids
    .filter((id) => isActive(id) && records.get(id)?.favorite)
    .sort(
      (a, b) =>
        (records.get(a)?.favorite?.order ?? 0) -
          (records.get(b)?.favorite?.order ?? 0) || byId(a, b),
    );
  const trash: TrashedPage[] = [];
  for (const id of ids) {
    const record = records.get(id);
    if (!record?.trash) continue;
    const parent = parentOf.get(id) ?? null;
    const parentBatch =
      parent === null ? undefined : records.get(parent)?.trash?.batchId;
    if (parentBatch === record.trash.batchId) continue;
    trash.push({
      id,
      title: record.title,
      batch_id: record.trash.batchId,
      trashed_at: record.trash.trashedAt,
    });
  }
  trash.sort(
    (a, b) => (b.trashed_at ?? 0) - (a.trashed_at ?? 0) || byId(a.id, b.id),
  );
  return { tree: { pages, favorites, trash }, parentOf };
}

/** Trashes `pageId` and its active descendants as one batch (TREE-9). */
export function trashPage(
  doc: Y.Doc,
  pageId: string,
  batchId: string,
  trashedAt: number,
) {
  const records = readRecords(doc);
  const { tree } = projectTree(records);
  const batch = new Set([pageId]);
  for (const page of tree.pages) {
    if (page.parent_id !== null && batch.has(page.parent_id))
      batch.add(page.id);
  }
  const pages = doc.getMap<Y.Map<unknown>>(pagesRoot);
  doc.transact(() => {
    for (const id of batch) {
      const record = records.get(id);
      const map = pages.get(id);
      if (!record || !(map instanceof Y.Map)) continue;
      map.set("trash", {
        originalParentId: record.parentId,
        originalOrder: record.order,
        batchId,
        trashedAt,
      });
    }
  });
}
