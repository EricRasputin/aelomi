import { create } from "zustand";

type SettingsDialog =
  | { kind: "closed" }
  | { kind: "open"; returnTo: Element | null };

export const useSettingsDialogStore = create<{
  dialog: SettingsDialog;
  show: (returnTo: Element | null) => void;
  hide: () => void;
}>((set, get) => ({
  dialog: { kind: "closed" },
  show: (returnTo) => {
    if (get().dialog.kind === "closed")
      set({ dialog: { kind: "open", returnTo } });
  },
  hide: () => set({ dialog: { kind: "closed" } }),
}));
