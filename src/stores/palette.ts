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
  prepareTitles,
  rankTitles,
  type TitleMatch,
} from "../palette/title-match";
import { useNewPagesStore } from "../shell/new-pages";
import { pagePresence, visiblePages } from "../shell/page-presence";
import { useIndexStatusStore } from "./index-status";
import { usePageTitlesStore } from "./page-titles";
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

function titlesOf(source: SourceTree) {
  return prepareTitles(
    visiblePages(source, useNewPagesStore.getState().pages, new Set()).map(
      (page) => ({
        id: page.id,
        title: displayTitle(
          usePageTitlesStore.getState().drafts.get(page.id)?.title ??
            page.title,
        ),
      }),
    ),
  );
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

function initialSelection(
  titles: readonly TitleMatch[],
  commands: readonly PageCommand[],
  content: ContentResult,
) {
  if (titles.length > 0) return 0;
  const safe = commands.findIndex((entry) => entry.command !== "trashPage");
  if (safe >= 0) return safe;
  return content.kind === "ready" && content.hits.length > 0
    ? commands.length
    : -1;
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
      active !== null &&
        pagePresence(active).kind === "listed" &&
        useShellStore.getState().trashRequest?.status !== "pending",
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
        selected: -1,
      });
      return;
    }
    set({
      query,
      titles,
      pageCommands,
      request,
      content: { kind: "idle" },
      selected: initialSelection(titles, pageCommands, { kind: "idle" }),
    });
    void commands
      .search(query, contentLimit, contentScope(titles))
      .then(contentFromReply, (error: unknown) => ({
        kind: "error" as const,
        message: error instanceof Error ? error.message : String(error),
      }))
      .then((content) => {
        const current = get();
        if (current.request !== request || !current.open) return;
        set({
          content,
          answered: request,
          selected: initialSelection(
            current.titles,
            current.pageCommands,
            content,
          ),
        });
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
    selected: -1,
    show: (returnTo) => {
      if (get().open) return;
      set({
        open: true,
        returnTo,
        query: "",
        titles: [],
        pageCommands: [],
        content: { kind: "idle" },
        selected: -1,
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
      const current = get().selected;
      set({
        selected:
          current < 0
            ? delta > 0
              ? 0
              : count - 1
            : (current + delta + count) % count,
      });
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
  const stopNewPages = useNewPagesStore.subscribe(rerun);
  const stopTitles = usePageTitlesStore.subscribe(rerun);
  return () => {
    stopNewPages();
    stopTitles();
    stopStatus();
    stopTree();
  };
}
