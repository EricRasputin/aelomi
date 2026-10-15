import { shortcutFor, shortcuts, type WebviewCommand } from "./shortcuts";

export type CommandContext = {
  source: "keyboard" | "menu";
  target: Element | null;
};
export type CommandHandler = {
  run: (context: CommandContext) => void;
  enabled?: (context: CommandContext) => boolean;
};

export function createCommandDispatcher() {
  const handlers = new Map<WebviewCommand, CommandHandler>();
  function dispatch(
    command: string,
    context: CommandContext,
    beforeRun?: () => void,
  ): boolean {
    const entry = shortcuts.find((entry) => entry.command === command);
    if (entry?.owner !== "webview") return false;
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
    keydown(event: KeyboardEvent, target: Element | null) {
      const command = shortcutFor(event);
      if (event.defaultPrevented || !command) return false;
      return dispatch(command, { source: "keyboard", target }, () => {
        event.preventDefault();
        event.stopPropagation();
      });
    },
  };
}

export const commandDispatcher = createCommandDispatcher();
