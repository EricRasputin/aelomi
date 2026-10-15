import type { createCommandDispatcher } from "./command-dispatcher";

type Dispatcher = ReturnType<typeof createCommandDispatcher>;

export function connectShortcutEvents(dispatcher: Dispatcher) {
  let composingTarget: Element | null = null;
  const compositionstart = (event: CompositionEvent) => {
    composingTarget = event.target instanceof Element ? event.target : null;
  };
  const clear = () => {
    composingTarget = null;
  };
  const keydown = (event: KeyboardEvent) =>
    dispatcher.keydown(event, document.activeElement);
  window.addEventListener("keydown", keydown, true);
  window.addEventListener("compositionstart", compositionstart, true);
  window.addEventListener("compositionend", clear, true);
  window.addEventListener("focusout", clear, true);
  return {
    menu(command: string) {
      return dispatcher.dispatch(command, {
        source: "menu",
        target: document.activeElement,
        isComposing:
          composingTarget?.isConnected === true &&
          composingTarget === document.activeElement,
      });
    },
    dispose() {
      window.removeEventListener("keydown", keydown, true);
      window.removeEventListener("compositionstart", compositionstart, true);
      window.removeEventListener("compositionend", clear, true);
      window.removeEventListener("focusout", clear, true);
    },
  };
}
