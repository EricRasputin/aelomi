import type { CommandContext, CommandHandler } from "../command-dispatcher";
import { getActiveEditor } from "./active-editor";

export function isPlainTextField(
  target: Element | null,
): target is HTMLInputElement | HTMLTextAreaElement {
  return (
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLInputElement &&
      ["text", "search", "email", "url", "tel", "password", "number"].includes(
        target.type,
      ))
  );
}

export function historyCommand(action: "undo" | "redo"): CommandHandler {
  const field = ({ target }: CommandContext) =>
    isPlainTextField(target) ? target : null;
  return {
    enabled(context) {
      const input = field(context);
      if (input)
        return context.source === "menu" && !input.disabled && !input.readOnly;
      return getActiveEditor()?.isEditable() === true;
    },
    run(context) {
      if (field(context)) {
        document.execCommand(action);
      } else {
        getActiveEditor()?.[action]();
      }
    },
  };
}
