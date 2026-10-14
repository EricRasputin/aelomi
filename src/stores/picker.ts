import { SuggestionMenu } from "@blocknote/core";
import { create } from "zustand";
import { isPageId } from "../editor/body-reader";
import type { PickerKey } from "../editor/page-picker";
import { pickerTrigger } from "../editor/page-picker";
import type { PageEditor } from "../editor/page-session";
import type { TextPart, TreePage } from "../ipc";
import { prepareTitles, rankTitles } from "../palette/title-match";
import { displayTitle, type SourceTree, usePageTreeStore } from "./page-tree";
import { useRecentPagesStore } from "./recent-pages";

export type PickerOption = {
  id: string;
  parts: TextPart[];
  /** LINK-2: the parent path, and the updated date if paths match too. */
  detail: string | null;
};

type Anchor = { left: number; top: number; bottom: number };

type PickerStore = {
  open: boolean;
  /** The source tree was read since this picker opened. */
  fresh: boolean;
  query: string;
  anchor: Anchor | null;
  options: PickerOption[];
  selected: number;
  attach: (editor: PageEditor) => () => void;
  detach: () => void;
  key: (key: PickerKey) => boolean;
  choose: (index: number) => void;
};

const optionLimit = 50;

function parentPath(page: TreePage, byId: ReadonlyMap<string, TreePage>) {
  const names: string[] = [];
  let parent = page.parent_id;
  while (parent !== null) {
    const next = byId.get(parent);
    if (!next) break;
    names.unshift(displayTitle(next.title));
    parent = next.parent_id;
  }
  return names.join(" / ");
}

/** Candidates come only from the source tree, which holds active pages only. */
export function pickerOptions(
  source: SourceTree,
  query: string,
  openedAt: ReadonlyMap<string, number>,
): PickerOption[] {
  if (source.kind !== "ready") return [];
  const pages = source.tree.pages.filter((page) => isPageId(page.id));
  const ranked =
    query.trim() === ""
      ? pages.slice(0, optionLimit).map((page) => ({
          id: page.id,
          parts: [{ text: displayTitle(page.title), highlighted: false }],
        }))
      : rankTitles(
          prepareTitles(
            pages.map((page) => ({
              id: page.id,
              title: displayTitle(page.title),
            })),
          ),
          query,
          openedAt,
          optionLimit,
        );
  const titleOf = (id: string) =>
    displayTitle(source.byId.get(id)?.title ?? "");
  const counts = new Map<string, number>();
  for (const option of ranked)
    counts.set(titleOf(option.id), (counts.get(titleOf(option.id)) ?? 0) + 1);
  const paths = new Map<string, string>();
  for (const option of ranked) {
    const page = source.byId.get(option.id);
    if (page && (counts.get(titleOf(option.id)) ?? 0) > 1)
      paths.set(option.id, parentPath(page, source.byId) || "Top level");
  }
  return ranked.map((option) => {
    const path = paths.get(option.id);
    if (path === undefined) return { ...option, detail: null };
    const twins = ranked.filter(
      (other) =>
        other.id !== option.id &&
        titleOf(other.id) === titleOf(option.id) &&
        paths.get(other.id) === path,
    );
    const updated = source.byId.get(option.id)?.updated_at;
    return {
      ...option,
      detail:
        twins.length > 0 && updated != null
          ? `${path} · ${new Date(updated).toLocaleDateString()}`
          : path,
    };
  });
}

let editor: PageEditor | null = null;

export const usePickerStore = create<PickerStore>((set, get) => {
  const menu = () => editor?.getExtension(SuggestionMenu);
  const refresh = (query: string) =>
    set({
      query,
      options: pickerOptions(
        usePageTreeStore.getState().source,
        query,
        useRecentPagesStore.getState().openedAt,
      ),
      selected: 0,
    });
  return {
    open: false,
    fresh: false,
    query: "",
    anchor: null,
    options: [],
    selected: 0,
    attach: (attached) => {
      editor = attached;
      const store = attached.getExtension(SuggestionMenu)?.store;
      const stopMenu = store?.subscribe(() => {
        const state = store.state;
        if (!state || state.triggerCharacter !== pickerTrigger || !state.show) {
          if (get().open) set({ open: false, anchor: null });
          return;
        }
        const rect = state.referencePos;
        const anchor = { left: rect.left, top: rect.top, bottom: rect.bottom };
        if (!get().open) {
          set({
            open: true,
            anchor,
            query: state.query,
            options: [],
            fresh: false,
          });
          void usePageTreeStore
            .getState()
            .refresh()
            .then(() => {
              if (!get().open) return;
              set({ fresh: true });
              refresh(get().query);
            });
        } else {
          set({ anchor });
          if (state.query !== get().query) {
            if (get().fresh) refresh(state.query);
            else set({ query: state.query });
          }
        }
      });
      return () => {
        stopMenu?.();
        if (editor === attached) get().detach();
      };
    },
    detach: () => {
      editor = null;
      set({ open: false, fresh: false, anchor: null, options: [], query: "" });
    },
    key: (key) => {
      const { open, options, selected } = get();
      if (!open) return false;
      switch (key) {
        case "ArrowDown":
          if (options.length > 0)
            set({ selected: (selected + 1) % options.length });
          return true;
        case "ArrowUp":
          if (options.length > 0)
            set({ selected: (selected - 1 + options.length) % options.length });
          return true;
        case "Enter":
          get().choose(selected);
          return true;
        case "Escape":
          menu()?.closeMenu();
          return true;
      }
    },
    choose: (index) => {
      const option = get().options[index];
      const suggestions = menu();
      if (!editor || !suggestions) return;
      if (!option) {
        suggestions.closeMenu();
        return;
      }
      suggestions.clearQuery();
      editor.insertInlineContent([
        { type: "pageLink", props: { targetPageId: option.id } },
      ]);
      suggestions.closeMenu();
    },
  };
});
