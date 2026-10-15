import { create } from "zustand";
import { commands } from "../ipc";
import { usePageTreeStore } from "./page-tree";
import { canEdit, useQuitStore } from "./quit";

type Draft = { title: string; version: number; error: string | null };
type TitleStore = {
  drafts: ReadonlyMap<string, Draft>;
  rename: (pageId: string, title: string) => Promise<void>;
};
let version = 0;

export const usePageTitlesStore = create<TitleStore>((set, get) => ({
  drafts: new Map(),
  rename: async (pageId, title) => {
    if (!canEdit(useQuitStore.getState())) return;
    const current = ++version;
    set({
      drafts: new Map(get().drafts).set(pageId, {
        title,
        version: current,
        error: null,
      }),
    });
    try {
      const reply = await commands.pageRename(pageId, title);
      if (reply.status === "error")
        throw new Error(JSON.stringify(reply.error));
      const source = await usePageTreeStore.getState().refresh();
      if (
        get().drafts.get(pageId)?.version === current &&
        source.kind === "ready" &&
        source.byId.get(pageId)?.title === title
      ) {
        const drafts = new Map(get().drafts);
        drafts.delete(pageId);
        set({ drafts });
      }
    } catch {
      if (get().drafts.get(pageId)?.version !== current) return;
      set({
        drafts: new Map(get().drafts).set(pageId, {
          title,
          version: current,
          error: "This title couldn't be saved. Your text is kept here.",
        }),
      });
    }
  },
}));

export function usePageTitle(pageId: string) {
  const title = usePageTreeStore((state) =>
    state.source.kind === "ready"
      ? (state.source.byId.get(pageId)?.title ?? "")
      : "",
  );
  const draft = usePageTitlesStore((state) => state.drafts.get(pageId));
  return draft?.title ?? title;
}
