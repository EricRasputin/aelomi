import { useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "./command-dispatcher";
import { historyCommand } from "./editor/history-commands";
import { events } from "./ipc";
import { usePaletteStore } from "./stores/palette";

export function useShortcutCommands() {
  useEffect(() => {
    const unregister = [
      commandDispatcher.register("openPalette", {
        run: ({ target }) =>
          flushSync(() => usePaletteStore.getState().show(target)),
      }),
      commandDispatcher.register("undo", historyCommand("undo")),
      commandDispatcher.register("redo", historyCommand("redo")),
    ];
    const keydown = (event: KeyboardEvent) =>
      commandDispatcher.keydown(event, document.activeElement);
    window.addEventListener("keydown", keydown, true);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void events.menuCommand
      .listen(({ payload }) => {
        if (!disposed)
          commandDispatcher.dispatch(payload.command, {
            source: "menu",
            target: document.activeElement,
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
      for (const stop of unregister) stop();
    };
  }, []);
}
