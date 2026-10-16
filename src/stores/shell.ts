import { create } from "zustand";

type FocusRequest = { pageId: string; target: "title" | "body" | "bodyStart" };

type ShellStore = {
  sidebarVisible: boolean;
  activePageId: string | null;
  focusRequest: FocusRequest | null;
  message: string | null;
  trashed: ReadonlySet<string>;
  trashRequest: {
    pageId: string;
    affected: ReadonlySet<string>;
    status: "pending" | "error";
  } | null;
  toggleSidebar: () => void;
  requestFocus: (request: FocusRequest) => void;
};

export const useShellStore = create<ShellStore>((set) => ({
  sidebarVisible: true,
  activePageId: null,
  focusRequest: null,
  message: null,
  trashed: new Set(),
  trashRequest: null,
  toggleSidebar: () =>
    set((state) => ({ sidebarVisible: !state.sidebarVisible })),
  requestFocus: (focusRequest) => set({ focusRequest }),
}));
