type FieldHistory = (action: "undo" | "redo") => void;
const histories = new WeakMap<Element, FieldHistory>();

export function registerFieldHistory(field: Element, history: FieldHistory) {
  histories.set(field, history);
  return () => histories.delete(field);
}

export function fieldHistory(field: Element) {
  return histories.get(field);
}

export function createTitleHistory(initial: string) {
  const past: string[] = [];
  const future: string[] = [];
  let current = initial;
  return {
    value: () => current,
    reset(value: string) {
      past.length = 0;
      future.length = 0;
      current = value;
    },
    record(value: string) {
      if (value === current) return;
      past.push(current);
      future.length = 0;
      current = value;
    },
    move(action: "undo" | "redo") {
      const from = action === "undo" ? past : future;
      const to = action === "undo" ? future : past;
      const value = from.pop();
      if (value === undefined) return current;
      to.push(current);
      current = value;
      return current;
    },
  };
}

const titleHistories = new Map<string, ReturnType<typeof createTitleHistory>>();

export function pageTitleHistory(pageId: string, initial: string) {
  let history = titleHistories.get(pageId);
  if (!history) {
    history = createTitleHistory(initial);
    titleHistories.set(pageId, history);
  }
  return history;
}
