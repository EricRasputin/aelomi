export type ActiveEditor = {
  dom: HTMLElement;
  focus: () => void;
  isEditable: () => boolean;
  undo: () => boolean;
  redo: () => boolean;
};

let active: ActiveEditor | null = null;

export function setActiveEditor(editor: ActiveEditor | null) {
  active = editor;
}

export function getActiveEditor(): ActiveEditor | null {
  return active;
}

/** Focuses `element`, through ProseMirror when it is the editor so its selection returns at once. */
export function restoreFocus(element: Element | null) {
  if (active && element && active.dom.contains(element)) active.focus();
  else if (element instanceof HTMLElement) element.focus();
}
