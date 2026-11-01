import type { CommandContext, CommandHandler } from "../command-dispatcher";
import { getActiveEditor } from "./active-editor";
import { fieldHistory } from "./field-history";

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
      const { target } = context;
      if (!target || target.closest("[inert]")) return false;
      const input = field(context);
      if (input) return !input.disabled && !input.readOnly;
      const editor = getActiveEditor();
      return (
        editor?.isEditable() === true &&
        editor.dom.contains(target) &&
        !editor.dom.closest("[inert]") &&
        !document.querySelector('[aria-modal="true"], dialog[open]')
      );
    },
    run(context) {
      const input = field(context);
      if (input) {
        const managed = fieldHistory(input);
        if (managed) managed(action);
        else document.execCommand(action);
      } else {
        getActiveEditor()?.[action]();
      }
    },
  };
}
