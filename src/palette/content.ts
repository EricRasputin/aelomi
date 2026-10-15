import {
  type IndexStatus,
  MAX_EXCLUDED_PAGES,
  type PageIssue,
  type QueryError,
  type SearchHit,
  type SearchReply,
  type SearchScope,
} from "../ipc";
import type { TitleMatch } from "./title-match";

/** Content search excludes the whole title group, so the core's exclusion limit caps it. */
export const titleGroupMax = MAX_EXCLUDED_PAGES;
export const contentLimit = 20;

export type ContentResult =
  | { kind: "idle" }
  | { kind: "indexing" }
  | { kind: "ready"; hits: SearchHit[]; issues: PageIssue[] }
  | { kind: "failed"; reason: string }
  | { kind: "error"; message: string };

export function contentScope(titles: readonly TitleMatch[]): SearchScope {
  return { Content: { exclude: titles.map((title) => title.id) } };
}

export function contentFromReply(
  reply:
    | { status: "ok"; data: SearchReply }
    | { status: "error"; error: QueryError },
): ContentResult {
  if (reply.status === "error") {
    const { error } = reply;
    if (typeof error === "string") return { kind: "error", message: error };
    if ("Failed" in error && error.Failed)
      return { kind: "failed", reason: error.Failed.reason };
    if ("InvalidQuery" in error && error.InvalidQuery)
      return {
        kind: "error",
        message: `too many title results to exclude (at most ${error.InvalidQuery.TooManyExcluded.max})`,
      };
    return { kind: "error", message: error.Io ?? "unknown" };
  }
  if (reply.data === "Indexing") return { kind: "indexing" };
  return {
    kind: "ready",
    hits: reply.data.Ready.hits,
    issues: reply.data.Ready.issues,
  };
}

export function shownContent(
  status: IndexStatus | null,
  content: ContentResult,
): ContentResult {
  if (content.kind === "idle") return content;
  if (status !== null && typeof status === "object")
    return { kind: "failed", reason: status.Failed.reason };
  if (status === "Rebuilding") return { kind: "indexing" };
  return content;
}
