import { shortcutFor, shortcuts, type WebviewCommand } from "./shortcuts";
import { useQuitStore } from "./stores/quit";
import { useStorageStartupStore } from "./stores/storage-startup";

export type CommandContext = {
  source: "keyboard" | "menu";
  target: Element | null;
  isComposing: boolean;
};
export type CommandHandler = {
  run: (context: CommandContext) => void;
  enabled?: (context: CommandContext) => boolean;
};

export function createCommandDispatcher(
  available: (command: WebviewCommand) => boolean = () => true,
) {
  const handlers = new Map<WebviewCommand, CommandHandler>();
  function dispatch(
    command: string,
    context: CommandContext,
    beforeRun?: () => void,
  ): boolean {
    if (context.isComposing) return false;
    const entry = shortcuts.find((entry) => entry.command === command);
    if (entry?.owner !== "webview" || !available(entry.command)) return false;
    const handler = handlers.get(entry.command);
    if (!handler || handler.enabled?.(context) === false) return false;
    beforeRun?.();
    handler.run(context);
    return true;
  }
  return {
    register(command: WebviewCommand, handler: CommandHandler) {
      handlers.set(command, handler);
      return () => {
        if (handlers.get(command) === handler) handlers.delete(command);
      };
    },
    dispatch,
    keydown(event: KeyboardEvent, target: Element | null, composing = false) {
      const command = shortcutFor(event);
      if (event.defaultPrevented || !command) return false;
      return dispatch(
        command,
        {
          source: "keyboard",
          target,
          isComposing: composing || event.isComposing,
        },
        () => {
          event.preventDefault();
          event.stopPropagation();
        },
      );
    },
  };
}

export const commandDispatcher = createCommandDispatcher(
  (command) =>
    useQuitStore.getState().decision.kind === "open" &&
    (command === "closeWindow" ||
      useStorageStartupStore.getState().startup.kind === "ready"),
);
