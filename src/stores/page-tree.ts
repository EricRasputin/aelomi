import { create } from "zustand";
import { commands, events, type PageTree, type TreePage } from "../ipc";
import { onRecordCommand } from "../ipc/record-commands";

export type SourceTree =
  | { kind: "loading" }
  | { kind: "ready"; tree: PageTree; byId: ReadonlyMap<string, TreePage> }
  | { kind: "error"; message: string };

type PageTreeStore = {
  source: SourceTree;
  refresh: () => Promise<SourceTree>;
};

let inFlight: Promise<SourceTree> | null = null;
let dirty = false;

export const usePageTreeStore = create<PageTreeStore>((set, get) => ({
  source: { kind: "loading" },
  refresh: () => {
    if (inFlight) {
      dirty = true;
      return inFlight;
    }
    const read = async () => {
      do {
        dirty = false;
        let next: SourceTree;
        try {
          const reply = await commands.pageTree();
          next =
            reply.status === "ok"
              ? {
                  kind: "ready",
                  tree: reply.data,
                  byId: new Map(
                    reply.data.pages.map((page) => [page.id, page]),
                  ),
                }
              : { kind: "error", message: queryErrorText(reply.error) };
        } catch (error: unknown) {
          next = {
            kind: "error",
            message: error instanceof Error ? error.message : String(error),
          };
        }
        set({ source: next });
      } while (dirty);
      return get().source;
    };
    inFlight = read().finally(() => {
      inFlight = null;
      if (dirty) void get().refresh();
    });
    return inFlight;
  },
}));

export function connectPageTreeEvents() {
  let disposed = false;
  const refresh = () => {
    if (!disposed) void usePageTreeStore.getState().refresh();
  };
  const stops = [onRecordCommand(refresh)];
  const keep = (stop: () => void) => {
    if (disposed) stop();
    else stops.push(stop);
  };
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const subscribe = async (
    register: () => Promise<() => void>,
    delay = 100,
  ): Promise<void> => {
    try {
      keep(await register());
    } catch (error: unknown) {
      if (disposed) return;
      console.error("Page tree event connection failed; retrying", error);
      const timer = setTimeout(() => {
        timers.delete(timer);
        if (!disposed)
          void subscribe(register, Math.min(delay * 2, 5000)).then(refresh);
      }, delay);
      timers.add(timer);
    }
  };
  const ready = (async () => {
    await Promise.all([
      subscribe(() => events.pageRecordsChanged.listen(refresh)),
      subscribe(() =>
        events.indexChanged.listen(({ payload }) => {
          if (payload.workspace) refresh();
        }),
      ),
    ]);
    if (!disposed) await usePageTreeStore.getState().refresh();
  })();
  return {
    ready,
    dispose: () => {
      disposed = true;
      for (const timer of timers) clearTimeout(timer);
      timers.clear();
      for (const stop of stops.splice(0)) stop();
    },
  };
}

export const untitled = "Untitled";
export const deletedPage = "Deleted page";

export function displayTitle(title: string): string {
  return title.trim() === "" ? untitled : title;
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
