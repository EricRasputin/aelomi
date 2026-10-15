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
const context = { source: "menu", target: null, isComposing: false } as const;
afterEach(() => {
  setActiveEditor(null);
  document.body.replaceChildren();
});

test("SHELL-7 composition blocks both keyboard and menu commands", () => {
  const dispatcher = createCommandDispatcher();
  let text = "base token";
  dispatcher.register("openPalette", {
    run: () => {
      text = "palette";
    },
  });
  dispatcher.register("undo", {
    run: () => {
      text = "base";
    },
  });
  dispatcher.register("redo", {
    run: () => {
      text = "base token redo";
    },
  });
  const event = new KeyboardEvent("keydown", {
    key: "k",
    metaKey: true,
    isComposing: true,
    cancelable: true,
  });
  expect(dispatcher.keydown(event, null)).toBe(false);
  for (const command of ["openPalette", "undo", "redo"]) {
    dispatcher.dispatch(command, { ...context, isComposing: true });
    expect(text).toBe("base token");
  }
  dispatcher.dispatch("undo", context);
  expect(text).toBe("base");
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

test("SHELL-7 declined webview keys are cancelled and native keys remain native", () => {
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
    true,
    true,
    0,
  ]);
  dispatcher.register("toggleSidebar", {
    run: () => {
      count += 1;
    },
  });
  expect(dispatcher.keydown(key("\\"), null)).toBe(true);
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
  const dom = document.createElement("div");
  const target = document.createElement("span");
  dom.append(target);
  document.body.append(dom);
  setActiveEditor({
    dom,
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
  dispatcher.keydown(key("z"), target);
  expect([text, actions]).toEqual(["base", 1]);
  dispatcher.keydown(key("Z", true), target);
  expect([text, actions]).toEqual(["base token", 2]);
  dispatcher.dispatch("undo", { ...context, target });
  expect([text, actions]).toEqual(["base", 3]);
  dispatcher.dispatch("redo", { ...context, target });
  expect([text, actions]).toEqual(["base token", 4]);
});

test("SHELL-7 field keyboard history runs and cancels without a menu listener", () => {
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
  const actions: string[] = [];
  const original = document.execCommand;
  document.execCommand = (action) => {
    actions.push(action);
    return true;
  };
  try {
    for (const target of [
      document.createElement("input"),
      document.createElement("textarea"),
    ]) {
      const undo = key("z");
      const redo = key("Z", true);
      expect(isPlainTextField(target)).toBe(true);
      expect(dispatcher.keydown(undo, target)).toBe(true);
      expect(dispatcher.keydown(redo, target)).toBe(true);
      expect([
        undo.defaultPrevented,
        redo.defaultPrevented,
        editorChanges,
      ]).toEqual([true, true, 0]);
    }
    expect(actions).toEqual(["undo", "redo", "undo", "redo"]);
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    expect(isPlainTextField(checkbox)).toBe(false);
    expect(dispatcher.keydown(key("z"), checkbox)).toBe(false);
    expect(editorChanges).toBe(0);
  } finally {
    document.execCommand = original;
  }
});

test("SHELL-7 palette chrome, inert editors, and modal targets cannot undo the page", () => {
  const dispatcher = createCommandDispatcher();
  const main = document.createElement("main");
  const dom = document.createElement("div");
  main.append(dom);
  const palette = document.createElement("div");
  palette.setAttribute("role", "dialog");
  palette.setAttribute("aria-modal", "true");
  const row = document.createElement("button");
  palette.append(row);
  document.body.append(main, palette);
  let text = "base token";
  setActiveEditor({
    dom,
    focus: () => {},
    isEditable: () => true,
    undo: () => {
      text = "base";
      return true;
    },
    redo: () => {
      text = "base token";
      return true;
    },
  });
  dispatcher.register("undo", historyCommand("undo"));
  dispatcher.register("redo", historyCommand("redo"));
  for (const target of [document.body, row]) {
    dispatcher.keydown(key("z"), target);
    dispatcher.dispatch("undo", { ...context, target });
    expect(text).toBe("base token");
  }
  main.setAttribute("inert", "");
  dispatcher.keydown(key("z"), dom);
  dispatcher.dispatch("undo", { ...context, target: dom });
  expect(text).toBe("base token");
  main.removeAttribute("inert");
  palette.append(dom);
  dispatcher.dispatch("undo", { ...context, target: dom });
  expect(text).toBe("base token");
  main.append(dom);
  palette.remove();
  dispatcher.keydown(key("z"), dom);
  expect(text).toBe("base");
  dispatcher.dispatch("redo", { ...context, target: dom });
  expect(text).toBe("base token");
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
    dispatcher.dispatch("undo", { ...context, target: input });
    dispatcher.dispatch("redo", { ...context, target: input });
    expect(actions).toEqual(["undo", "redo"]);
    input.readOnly = true;
    expect(dispatcher.dispatch("undo", { ...context, target: input })).toBe(
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
    expect([event.defaultPrevented, actions]).toEqual([true, ["undo", "redo"]]);
  } finally {
    document.execCommand = original;
  }
});

test("SHELL-7 a modal-declined editor Undo cannot reach a downstream keymap", () => {
  const dispatcher = createCommandDispatcher();
  const dom = document.createElement("div");
  const modal = document.createElement("div");
  modal.setAttribute("aria-modal", "true");
  document.body.append(dom, modal);
  let text = "base token";
  setActiveEditor({
    dom,
    focus: () => {},
    isEditable: () => true,
    undo: () => {
      text = "base";
      return true;
    },
    redo: () => false,
  });
  dispatcher.register("undo", historyCommand("undo"));
  const capture = (event: KeyboardEvent) => dispatcher.keydown(event, dom);
  const downstream = () => {
    text = "BlockNote undo";
  };
  window.addEventListener("keydown", capture, true);
  dom.addEventListener("keydown", downstream);
  try {
    const event = new KeyboardEvent("keydown", {
      key: "z",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    dom.dispatchEvent(event);
    expect([event.defaultPrevented, text]).toEqual([true, "base token"]);
    modal.remove();
    dom.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "z",
        metaKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    expect(text).toBe("base");
  } finally {
    window.removeEventListener("keydown", capture, true);
  }
});
