import { shortcutFor, shortcuts, type WebviewCommand } from "./shortcuts";
import { acceptsUserCommands, useQuitStore } from "./stores/quit";

export type CommandContext = {
  source: "keyboard" | "menu";
  target: Element | null;
  isComposing: boolean;
};
export type CommandHandler = {
  run: (context: CommandContext) => void;
  enabled?: (context: CommandContext) => boolean;
};

export function createCommandDispatcher(available: () => boolean = () => true) {
  const handlers = new Map<WebviewCommand, CommandHandler>();
  function dispatch(command: string, context: CommandContext): boolean {
    if (context.isComposing) return false;
    const entry = shortcuts.find((entry) => entry.command === command);
    if (entry?.owner !== "webview" || !available()) return false;
    const handler = handlers.get(entry.command);
    if (!handler || handler.enabled?.(context) === false) return false;
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
    keydown(event: KeyboardEvent, target: Element | null) {
      const command = shortcutFor(event);
      if (event.defaultPrevented || !command) return false;
      event.preventDefault();
      event.stopPropagation();
      return dispatch(command, {
        source: "keyboard",
        target,
        isComposing: event.isComposing,
      });
    },
  };
}

export const commandDispatcher = createCommandDispatcher(() =>
  acceptsUserCommands(useQuitStore.getState().decision),
);
