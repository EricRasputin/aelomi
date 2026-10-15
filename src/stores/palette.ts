import { create } from "zustand";
import { commands } from "../ipc";
import {
  type ContentResult,
  contentFromReply,
  contentLimit,
  contentScope,
  titleGroupMax,
} from "../palette/content";
import {
  matchingPageCommands,
  type PageCommand,
} from "../palette/page-commands";
import {
  type PreparedTitles,
  prepareTitles,
  rankTitles,
  type TitleMatch,
} from "../palette/title-match";
import { useIndexStatusStore } from "./index-status";
import { displayTitle, type SourceTree, usePageTreeStore } from "./page-tree";
import { useRecentPagesStore } from "./recent-pages";
import { useShellStore } from "./shell";

type PaletteStore = {
  open: boolean;
  returnTo: Element | null;
  query: string;
  titles: TitleMatch[];
  pageCommands: PageCommand[];
  content: ContentResult;
  /** The newest search sent. A reply for an older one is dropped. */
  request: number;
  /** The search whose reply the content group shows. */
  answered: number;
  selected: number;
  show: (returnTo: Element | null) => void;
  hide: () => void;
  setQuery: (query: string, source: SourceTree) => void;
  rerun: (source: SourceTree) => void;
  move: (delta: number) => void;
  select: (index: number) => void;
};

let prepared: { source: SourceTree; titles: PreparedTitles } | null = null;

function titlesOf(source: SourceTree): PreparedTitles {
  if (source.kind !== "ready") return [];
  if (prepared?.source !== source)
    prepared = {
      source,
      titles: prepareTitles(
        source.tree.pages.map((page) => ({
          id: page.id,
          title: displayTitle(page.title),
        })),
      ),
    };
  return prepared.titles;
}

export function resultCount(
  state: Pick<PaletteStore, "titles" | "content" | "pageCommands">,
) {
  return (
    state.titles.length +
    state.pageCommands.length +
    (state.content.kind === "ready" ? state.content.hits.length : 0)
  );
}

export const usePaletteStore = create<PaletteStore>((set, get) => {
  const run = (query: string, source: SourceTree) => {
    const titles = rankTitles(
      titlesOf(source),
      query,
      useRecentPagesStore.getState().openedAt,
      titleGroupMax,
    );
    const active = useShellStore.getState().activePageId;
    const pageCommands = matchingPageCommands(
      query,
      source.kind === "ready" && active !== null && source.byId.has(active),
    );
    const request = get().request + 1;
    if (query.trim() === "") {
      set({
        query,
        titles,
        pageCommands,
        content: { kind: "idle" },
        request,
        answered: request,
        selected: 0,
      });
      return;
    }
    set({ query, titles, pageCommands, request, selected: 0 });
    void commands
      .search(query, contentLimit, contentScope(titles))
      .then(contentFromReply, (error: unknown) => ({
        kind: "error" as const,
        message: error instanceof Error ? error.message : String(error),
      }))
      .then((content) => {
        const current = get();
        if (current.request !== request || !current.open) return;
        set({ content, answered: request, selected: 0 });
      });
  };
  return {
    open: false,
    returnTo: null,
    query: "",
    titles: [],
    pageCommands: [],
    content: { kind: "idle" },
    request: 0,
    answered: 0,
    selected: 0,
    show: (returnTo) => {
      if (get().open) return;
      set({
        open: true,
        returnTo,
        query: "",
        titles: [],
        pageCommands: [],
        content: { kind: "idle" },
        selected: 0,
        request: get().request + 1,
        answered: get().request + 1,
      });
    },
    hide: () =>
      set({ open: false, returnTo: null, request: get().request + 1 }),
    setQuery: run,
    rerun: (source) => {
      const { open, query } = get();
      if (open) run(query, source);
    },
    move: (delta) => {
      const count = resultCount(get());
      if (count === 0) return;
      set({ selected: (get().selected + delta + count) % count });
    },
    select: (index) => set({ selected: index }),
  };
});

/**
 * Reruns the open query on every index status or change event, and when the
 * source tree answers, so the title group and its exclusions stay current.
 */
export function connectPalette(): () => void {
  const rerun = () =>
    usePaletteStore.getState().rerun(usePageTreeStore.getState().source);
  const stopStatus = useIndexStatusStore.subscribe((state, previous) => {
    if (state.revision !== previous.revision) rerun();
  });
  const stopTree = usePageTreeStore.subscribe((state, previous) => {
    if (state.source !== previous.source) rerun();
  });
  return () => {
    stopStatus();
    stopTree();
  };
}
