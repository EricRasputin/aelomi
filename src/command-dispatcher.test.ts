// @vitest-environment happy-dom
import { afterEach, expect, test } from "vitest";
import { createCommandDispatcher } from "./command-dispatcher";
import { setActiveEditor } from "./editor/active-editor";
import { historyCommand, isPlainTextField } from "./editor/history-commands";

const key = (key: string, shiftKey = false) =>
  new KeyboardEvent("keydown", {
    key,
    metaKey: true,
    shiftKey,
    cancelable: true,
  });
const context = { source: "menu", target: null } as const;
afterEach(() => {
  setActiveEditor(null);
  document.body.replaceChildren();
});

test("SHELL-7 one enabled command runs once per key and once per menu activation", () => {
  const dispatcher = createCommandDispatcher();
  const actions: string[] = [];
  dispatcher.register("openPalette", {
    run: ({ source }) => actions.push(source),
  });
  const event = key("k");
  expect(dispatcher.keydown(event, null)).toBe(true);
  expect(event.defaultPrevented).toBe(true);
  expect(dispatcher.keydown(event, null)).toBe(false);
  expect(dispatcher.dispatch("openPalette", context)).toBe(true);
  expect(actions).toEqual(["keyboard", "menu"]);
});

test("SHELL-7 missing, disabled, and native commands leave keys uncancelled", () => {
  const dispatcher = createCommandDispatcher();
  let count = 0;
  const missing = key(",");
  expect(dispatcher.keydown(missing, null)).toBe(false);
  dispatcher.register("toggleSidebar", {
    run: () => {
      count += 1;
    },
    enabled: () => false,
  });
  const disabled = key("\\");
  expect(dispatcher.keydown(disabled, null)).toBe(false);
  expect(dispatcher.dispatch("toggleSidebar", context)).toBe(false);
  expect(dispatcher.dispatch("quit", context)).toBe(false);
  expect(dispatcher.dispatch("unknown", context)).toBe(false);
  expect([missing.defaultPrevented, disabled.defaultPrevented, count]).toEqual([
    false,
    false,
    0,
  ]);
  dispatcher.register("toggleSidebar", {
    run: () => {
      count += 1;
    },
  });
  expect(dispatcher.keydown(disabled, null)).toBe(true);
  expect([disabled.defaultPrevented, count]).toEqual([true, 1]);
});

test("SHELL-7 disposing an old registration preserves its replacement", () => {
  const dispatcher = createCommandDispatcher();
  let result = "initial";
  const stop = dispatcher.register("openPalette", {
    run: () => {
      result = "old";
    },
  });
  const stopReplacement = dispatcher.register("openPalette", {
    run: () => {
      result = "replacement";
    },
  });
  stop();
  dispatcher.dispatch("openPalette", context);
  expect(result).toBe("replacement");
  stopReplacement();
  expect(dispatcher.dispatch("openPalette", context)).toBe(false);
});

test("SHELL-7 editor Undo and Redo share the keyboard and menu handler", () => {
  const dispatcher = createCommandDispatcher();
  let text = "base token";
  let actions = 0;
  setActiveEditor({
    dom: document.createElement("div"),
    focus: () => {},
    isEditable: () => true,
    undo: () => {
      text = "base";
      actions += 1;
      return true;
    },
    redo: () => {
      text = "base token";
      actions += 1;
      return true;
    },
  });
  dispatcher.register("undo", historyCommand("undo"));
  dispatcher.register("redo", historyCommand("redo"));
  dispatcher.keydown(key("z"), null);
  expect([text, actions]).toEqual(["base", 1]);
  dispatcher.keydown(key("Z", true), null);
  expect([text, actions]).toEqual(["base token", 2]);
  dispatcher.dispatch("undo", context);
  expect([text, actions]).toEqual(["base", 3]);
  dispatcher.dispatch("redo", context);
  expect([text, actions]).toEqual(["base token", 4]);
});

test("SHELL-7 text inputs and textareas retain native keyboard history", () => {
  const dispatcher = createCommandDispatcher();
  let editorChanges = 0;
  setActiveEditor({
    dom: document.createElement("div"),
    focus: () => {},
    isEditable: () => true,
    undo: () => {
      editorChanges += 1;
      return true;
    },
    redo: () => {
      editorChanges += 1;
      return true;
    },
  });
  dispatcher.register("undo", historyCommand("undo"));
  dispatcher.register("redo", historyCommand("redo"));
  for (const target of [
    document.createElement("input"),
    document.createElement("textarea"),
  ]) {
    const undo = key("z");
    const redo = key("Z", true);
    expect(isPlainTextField(target)).toBe(true);
    expect(dispatcher.keydown(undo, target)).toBe(false);
    expect(dispatcher.keydown(redo, target)).toBe(false);
    expect([
      undo.defaultPrevented,
      redo.defaultPrevented,
      editorChanges,
    ]).toEqual([false, false, 0]);
  }
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  expect(isPlainTextField(checkbox)).toBe(false);
  expect(dispatcher.keydown(key("z"), checkbox)).toBe(true);
  expect(editorChanges).toBe(1);
});

test("SHELL-7 menu history targets the plain field and a read-only editor is disabled", () => {
  const dispatcher = createCommandDispatcher();
  const actions: string[] = [];
  const original = document.execCommand;
  document.execCommand = (action) => {
    actions.push(action);
    return true;
  };
  try {
    dispatcher.register("undo", historyCommand("undo"));
    dispatcher.register("redo", historyCommand("redo"));
    const input = document.createElement("input");
    dispatcher.dispatch("undo", { source: "menu", target: input });
    dispatcher.dispatch("redo", { source: "menu", target: input });
    expect(actions).toEqual(["undo", "redo"]);
    input.readOnly = true;
    expect(dispatcher.dispatch("undo", { source: "menu", target: input })).toBe(
      false,
    );
    setActiveEditor({
      dom: document.createElement("div"),
      focus: () => {},
      isEditable: () => false,
      undo: () => {
        actions.push("editor undo");
        return true;
      },
      redo: () => false,
    });
    const event = key("z");
    expect(dispatcher.keydown(event, null)).toBe(false);
    expect([event.defaultPrevented, actions]).toEqual([
      false,
      ["undo", "redo"],
    ]);
  } finally {
    document.execCommand = original;
  }
});
