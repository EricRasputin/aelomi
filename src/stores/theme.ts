import { create } from "zustand";
import { commands, type Theme } from "../ipc";
import { trackCommand } from "../ipc/pending-commands";
import { storageRetry } from "../storage-retry";
import type { AppliedTheme } from "../theme";

type ThemeChoice = Exclude<Theme, object>;
type ThemeStore = {
  choice: Theme;
  transportFailure: string | null;
  choose: (choice: ThemeChoice) => void;
};

let revision = 0;
let writes = Promise.resolve();

async function persistTheme(choice: ThemeChoice) {
  let delay = storageRetry.firstMs;
  for (;;) {
    try {
      const result = await commands.settingsUpdate({ theme: choice });
      if (
        result.status === "ok" ||
        (typeof result.error === "object" && "Write" in result.error)
      ) {
        useThemeStore.setState({ transportFailure: null });
        return;
      }
      useThemeStore.setState({
        transportFailure: "Settings couldn't reach storage.",
      });
    } catch (error: unknown) {
      useThemeStore.setState({
        transportFailure: `Settings couldn't reach storage. ${error instanceof Error ? error.message : String(error)}`,
      });
    }
    await new Promise<void>((resolve) => setTimeout(resolve, delay));
    delay = Math.min(delay * 2, storageRetry.maxMs);
  }
}
export const useThemeStore = create<ThemeStore>((set) => ({
  choice: window.__aelomiInitialTheme ?? "System",
  transportFailure: null,
  choose: (choice) => {
    revision += 1;
    set({ choice });
    writes = trackCommand(writes.then(() => persistTheme(choice)));
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
  const loadingRevision = revision;
  void commands.settingsGet().then(
    (result) => {
      if (!disposed && revision === loadingRevision && result.status === "ok")
        useThemeStore.setState({ choice: result.data.theme });
    },
    (error: unknown) => console.error("Theme settings read failed", error),
  );
  return () => {
    disposed = true;
    stop();
    media.removeEventListener("change", changed);
    stopMock();
  };
}
