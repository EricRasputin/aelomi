import { expect, test } from "vitest";
import { createTitleHistory } from "./field-history";

test("TREE-7 title undo and redo stop at that title's initial value", () => {
  const history = createTitleHistory("Inbox");
  history.record("Inbox draft");
  history.record("Inbox draft 2");
  expect(history.move("undo")).toBe("Inbox draft");
  expect(history.move("undo")).toBe("Inbox");
  expect(history.move("undo")).toBe("Inbox");
  expect(history.move("redo")).toBe("Inbox draft");
  history.record("Inbox revised");
  expect(history.move("redo")).toBe("Inbox revised");
  expect(history.move("undo")).toBe("Inbox draft");
});
