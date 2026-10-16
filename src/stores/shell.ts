import { create } from "zustand";
import { type FocusRequest, resolvePageFocus } from "../shell/page-focus";
import { bodyReady, pagePresence } from "../shell/page-presence";

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
  titleFocused: (pageId: string) => void;
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
  requestFocus: (request) =>
    set({
      focusRequest: resolvePageFocus(request, {
        kind: "request",
        bodyReady: bodyReady(pagePresence(request.pageId)),
      }),
    }),
  titleFocused: (pageId) =>
    set((state) => ({
      focusRequest:
        state.focusRequest?.pageId === pageId
          ? resolvePageFocus(state.focusRequest, { kind: "titleFocus" })
          : state.focusRequest,
    })),
}));
