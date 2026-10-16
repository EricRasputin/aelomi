import { create } from "zustand";

type FocusRequest = { pageId: string; target: "title" | "body" | "bodyStart" };

export type CreateIntent = {
  originPageId: string | null;
  existingPageIds: ReadonlySet<string>;
  editorFocus: number;
  title: string;
  source: "button" | "keyboard" | "menu" | "palette";
  stage:
    | { kind: "pending" }
    | { kind: "opening"; pageId: string }
    | { kind: "failed" };
};

type ShellStore = {
  sidebarVisible: boolean;
  activePageId: string | null;
  focusRequest: FocusRequest | null;
  message: string | null;
  createIntent: CreateIntent | null;
  editorFocus: number;
  revealedPageId: string | null;
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
  createIntent: null,
  editorFocus: 0,
  revealedPageId: null,
  trashRequest: null,
  toggleSidebar: () =>
    set((state) => ({ sidebarVisible: !state.sidebarVisible })),
  requestFocus: (focusRequest) => set({ focusRequest }),
}));
