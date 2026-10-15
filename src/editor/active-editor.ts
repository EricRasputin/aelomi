type Focusable = { dom: HTMLElement; focus: () => void };

let active: Focusable | null = null;

export function setActiveEditor(editor: Focusable | null) {
  active = editor;
}

/** Focuses `element`, through ProseMirror when it is the editor so its selection returns at once. */
export function restoreFocus(element: Element | null) {
  if (active && element && active.dom.contains(element)) active.focus();
  else if (element instanceof HTMLElement) element.focus();
}
