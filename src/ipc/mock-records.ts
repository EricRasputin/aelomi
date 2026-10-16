import type * as Y from "yjs";
import { isPageId } from "../editor/body-reader";
import type { PageError, PageTree } from "./bindings";
import {
  type PageRecord,
  pagesRootProblem,
  trashPage,
  writeRecords,
} from "./mock-workspace";

type Reply<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: PageError };
type RecordCommands = {
  pageCreate: (parentId: string | null, pageId: string | null) => Reply<string>;
  pageRename: (pageId: string, title: string) => Reply<null>;
  pageTrash: (pageId: string) => Reply<string>;
};
type Context = {
  doc: () => Y.Doc;
  records: () => Map<string, PageRecord>;
  tree: () => PageTree;
  accept: (edit: () => void) => void;
  error: () => PageError | null;
  hasBody: (id: string) => boolean;
};
export function createRecordCommands(context: Context): RecordCommands {
  function recordFailure(ids: readonly string[]): PageError | null {
    const error = context.error();
    if (error) return error;
    if (ids.some((id) => !isPageId(id))) return "InvalidPageId";
    const problem = pagesRootProblem(context.doc());
    const all = context.records();
    if (
      problem !== null ||
      all.size !== context.doc().getMap("pages").size ||
      [...all].some(
        ([id, record]) =>
          !isPageId(id) ||
          (record.parentId !== null && !isPageId(record.parentId)) ||
          (record.trash?.originalParentId != null &&
            !isPageId(record.trash.originalParentId)),
      )
    )
      return { Malformed: { reason: problem ?? "malformed page record" } };
    return null;
  }

  return {
    pageCreate: (parentId, pageId = null) => {
      const error = recordFailure([
        ...(parentId === null ? [] : [parentId]),
        ...(pageId === null ? [] : [pageId]),
      ]);
      if (error) return { status: "error", error };
      if (pageId !== null) {
        const existing = context.records().get(pageId);
        if (existing)
          return existing.parentId === parentId &&
            context.tree().pages.some((page) => page.id === pageId) &&
            existing.title === ""
            ? { status: "ok", data: pageId }
            : { status: "error", error: "AlreadyExists" };
        if (context.hasBody(pageId))
          return { status: "error", error: "AlreadyExists" };
      }
      if (
        parentId !== null &&
        !context.tree().pages.some((p) => p.id === parentId)
      )
        return { status: "error", error: "NotActive" };
      const step = 2 ** 20;
      const siblings = new Set(
        context
          .tree()
          .pages.filter((page) => page.parent_id === parentId)
          .map((page) => page.id),
      );
      const order =
        [...context.records().entries()]
          .filter(([id]) => siblings.has(id))
          .reduce((max, [, record]) => Math.max(max, record.order), 0) + step;
      if (!Number.isSafeInteger(order))
        return { status: "error", error: "OrderExhausted" };
      const id = pageId ?? crypto.randomUUID().replaceAll("-", "");
      const now = Date.now();
      context.accept(() =>
        writeRecords(context.doc(), [
          {
            id,
            title: "",
            parentId,
            order,
            createdAt: now,
            updatedAt: now,
          },
        ]),
      );
      return { status: "ok", data: id };
    },
    pageRename: (pageId, title) => {
      const error = recordFailure([pageId]);
      if (error) return { status: "error", error };
      if (!context.tree().pages.some((p) => p.id === pageId))
        return { status: "error", error: "NotActive" };
      if (
        context
          .doc()
          .getMap<Y.Map<unknown>>("pages")
          .get(pageId)
          ?.get("title") === title
      )
        return { status: "ok", data: null };
      context.accept(() => {
        const map = context.doc().getMap<Y.Map<unknown>>("pages").get(pageId);
        const now = Date.now();
        if (map?.get("title") !== title) map?.set("title", title);
        if (map?.get("updatedAt") !== now) map?.set("updatedAt", now);
      });
      return { status: "ok", data: null };
    },
    pageTrash: (pageId) => {
      const error = recordFailure([pageId]);
      if (error) return { status: "error", error };
      const existing = context.records().get(pageId)?.trash;
      if (existing) return { status: "ok", data: existing.batchId };
      if (!context.tree().pages.some((p) => p.id === pageId))
        return { status: "error", error: "NotActive" };
      const batch = crypto.randomUUID().replaceAll("-", "");
      context.accept(() => trashPage(context.doc(), pageId, batch, Date.now()));
      return { status: "ok", data: batch };
    },
  };
}
