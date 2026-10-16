import { create } from "zustand";
import { commands, type Theme } from "../ipc";
import type { AppliedTheme } from "../theme";
import { updateSettings } from "./settings";
import { useStorageStartupStore } from "./storage-startup";

type ThemeChoice = Exclude<Theme, object>;
type ThemeStore = {
  choice: Theme;
  choose: (choice: ThemeChoice) => void;
};

let revision = 0;
export const useThemeStore = create<ThemeStore>((set) => ({
  choice: window.__aelomiInitialTheme ?? "System",
  choose: (choice) => {
    revision += 1;
    set({ choice });
    updateSettings({ theme: choice });
  },
}));

export function connectTheme() {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  let appearance: AppliedTheme = media.matches ? "dark" : "light";
  let disposed = false;
  const apply = () =>
    window.aelomiTheme.apply(
      window.aelomiTheme.resolve(useThemeStore.getState().choice, appearance),
    );
  const changed = () => {
    appearance = media.matches ? "dark" : "light";
    apply();
  };
  let stopMock = () => {};
  media.addEventListener("change", changed);
  if (import.meta.env.MODE === "mock") {
    const mockChanged = () => {
      appearance = window.__aelomiMock?.appearance.current() ?? appearance;
      apply();
    };
    mockChanged();
    window.addEventListener("aelomi-test-appearance", mockChanged);
    stopMock = () =>
      window.removeEventListener("aelomi-test-appearance", mockChanged);
  }
  const stop = useThemeStore.subscribe(apply);
  apply();
  const load = () => {
    const loadingRevision = revision;
    void commands.settingsGet().then(
      (result) => {
        if (!disposed && revision === loadingRevision && result.status === "ok")
          useThemeStore.setState({ choice: result.data.theme });
      },
      (error: unknown) => console.error("Theme settings read failed", error),
    );
  };
  const stopStorage = useStorageStartupStore.subscribe(
    ({ startup }, previous) => {
      if (startup.kind === "ready" && previous.startup.kind !== "ready") load();
    },
  );
  load();
  return () => {
    disposed = true;
    stop();
    stopStorage();
    media.removeEventListener("change", changed);
    stopMock();
  };
}
