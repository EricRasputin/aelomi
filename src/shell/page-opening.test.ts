import { afterEach, expect, test } from "vitest";
import { useRecordCommandStore } from "../stores/record-commands";
import { useShellStore } from "../stores/shell";
import { canOpenPage } from "./page-opening";

afterEach(() => {
  useRecordCommandStore.setState(useRecordCommandStore.getInitialState(), true);
  useShellStore.setState(useShellStore.getInitialState(), true);
});

test("TREE-1 opening a first-attempt create waits for its final result", () => {
  const intent = {
    originPageId: "a",
    existingPageIds: new Set(["a"]),
    editorFocus: 1,
    title: "",
    source: "keyboard" as const,
    stage: { kind: "pending" as const },
  };
  useShellStore.setState({ createIntent: intent });
  useRecordCommandStore.setState({ pendingCreates: new Set([null]) });
  expect(canOpenPage("a")).toBe(true);
  expect(canOpenPage("published-before-reply")).toBe(false);
  useRecordCommandStore.setState({ pendingCreates: new Set() });
  expect(canOpenPage("published-before-reply")).toBe(true);
});
