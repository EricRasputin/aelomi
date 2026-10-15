import { linkLabel, usePageTreeStore } from "../stores/page-tree";

declare global {
  interface Window {
    __aelomiLinkLabels?: () => number;
  }
}

const shown = new Map<HTMLElement, string>();
let unsubscribe: (() => void) | null = null;

function paint(dom: HTMLElement, targetPageId: string) {
  const label = linkLabel(usePageTreeStore.getState().source, targetPageId);
  dom.textContent = label.kind === "loading" ? "" : label.text;
  dom.dataset.state = label.kind;
}

function repaint() {
  for (const [dom, targetPageId] of shown) paint(dom, targetPageId);
}

/** A label serialized outside the editor, for copy, drag or HTML export. */
export function paintLinkLabel(dom: HTMLElement, targetPageId: string) {
  paint(dom, targetPageId);
}

/**
 * A label the editor shows, repainted on every page tree change until the
 * returned release runs. The labels share one store subscription.
 */
export function showLinkLabel(
  dom: HTMLElement,
  targetPageId: string,
): () => void {
  paint(dom, targetPageId);
  shown.set(dom, targetPageId);
  unsubscribe ??= usePageTreeStore.subscribe(repaint);
  return () => {
    shown.delete(dom);
    if (shown.size > 0) return;
    unsubscribe?.();
    unsubscribe = null;
  };
}

if (import.meta.env.MODE === "mock")
  window.__aelomiLinkLabels = () => shown.size;
