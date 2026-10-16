import { useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "./command-dispatcher";
import { historyCommand } from "./editor/history-commands";
import { commands, events } from "./ipc";
import { usePaletteStore } from "./stores/palette";
import { useSettingsDialogStore } from "./stores/settings-dialog";

export function useShortcutCommands() {
  useEffect(() => {
    const unregister = [
      commandDispatcher.register("closeWindow", {
        run: () => {
          const settings = useSettingsDialogStore.getState();
          const palette = usePaletteStore.getState();
          if (settings.dialog.kind === "open") flushSync(() => settings.hide());
          else if (palette.open) flushSync(() => palette.hide());
          else
            void commands
              .closeWindow()
              .catch((error: unknown) =>
                console.error("Window close failed", error),
              );
        },
      }),
      commandDispatcher.register("openPalette", {
        enabled: () =>
          useSettingsDialogStore.getState().dialog.kind === "closed",
        run: ({ target }) =>
          flushSync(() => usePaletteStore.getState().show(target)),
      }),
      commandDispatcher.register("openSettings", {
        run: ({ target }) => {
          const palette = usePaletteStore.getState();
          const returnTo = palette.open ? palette.returnTo : target;
          flushSync(() => palette.hide());
          flushSync(() => useSettingsDialogStore.getState().show(returnTo));
        },
      }),
      commandDispatcher.register("undo", historyCommand("undo")),
      commandDispatcher.register("redo", historyCommand("redo")),
    ];
    let composing = false;
    const compositionstart = () => {
      composing = true;
    };
    const compositionend = () => {
      composing = false;
    };
    const keydown = (event: KeyboardEvent) =>
      commandDispatcher.keydown(event, document.activeElement, composing);
    window.addEventListener("keydown", keydown, true);
    window.addEventListener("compositionstart", compositionstart, true);
    window.addEventListener("compositionend", compositionend, true);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void events.menuCommand
      .listen(({ payload }) => {
        if (!disposed)
          commandDispatcher.dispatch(payload.command, {
            source: "menu",
            target: document.activeElement,
            isComposing: composing,
          });
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) =>
        console.error("Menu command listener failed", error),
      );
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("keydown", keydown, true);
      window.removeEventListener("compositionstart", compositionstart, true);
      window.removeEventListener("compositionend", compositionend, true);
      for (const stop of unregister) stop();
    };
  }, []);
}
