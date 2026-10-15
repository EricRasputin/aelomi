import { create } from "zustand";
import { commands, type PageTree, type TreePage } from "../ipc";

export type SourceTree =
  | { kind: "loading" }
  | { kind: "ready"; tree: PageTree; byId: ReadonlyMap<string, TreePage> }
  | { kind: "error"; message: string };

type PageTreeStore = {
  source: SourceTree;
  refresh: () => Promise<SourceTree>;
};

let latest = 0;

export const usePageTreeStore = create<PageTreeStore>((set, get) => ({
  source: { kind: "loading" },
  refresh: async () => {
    latest += 1;
    const request = latest;
    let next: SourceTree;
    try {
      const reply = await commands.pageTree();
      next =
        reply.status === "ok"
          ? {
              kind: "ready",
              tree: reply.data,
              byId: new Map(reply.data.pages.map((page) => [page.id, page])),
            }
          : { kind: "error", message: queryErrorText(reply.error) };
    } catch (error: unknown) {
      next = {
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      };
    }
    if (request !== latest) return get().source;
    set({ source: next });
    return next;
  },
}));

export const untitled = "Untitled";
export const deletedPage = "Deleted page";

export function displayTitle(title: string): string {
  return title === "" ? untitled : title;
}

export type LinkLabel =
  | { kind: "loading" }
  | { kind: "active"; text: string }
  | { kind: "missing"; text: string };

export function linkLabel(source: SourceTree, pageId: string): LinkLabel {
  if (source.kind !== "ready") return { kind: "loading" };
  const page = source.byId.get(pageId);
  return page
    ? { kind: "active", text: displayTitle(page.title) }
    : { kind: "missing", text: deletedPage };
}

/** The text a page link shows: the target's title, or "Deleted page". */
export function linkText(source: SourceTree, pageId: string): string {
  const label = linkLabel(source, pageId);
  return label.kind === "loading" ? "" : label.text;
}

function queryErrorText(
  error: Extract<
    Awaited<ReturnType<typeof commands.pageTree>>,
    { status: "error" }
  >["error"],
): string {
  if (typeof error === "string") return error;
  if ("Failed" in error && error.Failed) return error.Failed.reason;
  if ("Io" in error && error.Io !== undefined) return error.Io;
  return "unknown";
}
