import { create } from "zustand";

type FocusRequest = { pageId: string; target: "title" | "body" | "bodyStart" };

type ShellStore = {
  sidebarVisible: boolean;
  activePageId: string | null;
  focusRequest: FocusRequest | null;
  message: string | null;
  creating: boolean;
  toggleSidebar: () => void;
  requestFocus: (request: FocusRequest) => void;
};

export const useShellStore = create<ShellStore>((set) => ({
  sidebarVisible: true,
  activePageId: null,
  focusRequest: null,
  message: null,
  creating: false,
  toggleSidebar: () =>
    set((state) => ({ sidebarVisible: !state.sidebarVisible })),
  requestFocus: (focusRequest) => set({ focusRequest }),
}));
