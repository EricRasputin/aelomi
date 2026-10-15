import { create } from "zustand";

type RecentPagesStore = {
  /** When each page was last opened in this session, as an increasing count. */
  openedAt: ReadonlyMap<string, number>;
  opened: (pageId: string) => void;
};

let clock = 0;

export const useRecentPagesStore = create<RecentPagesStore>((set, get) => ({
  openedAt: new Map(),
  opened: (pageId) => {
    clock += 1;
    set({ openedAt: new Map(get().openedAt).set(pageId, clock) });
  },
}));
