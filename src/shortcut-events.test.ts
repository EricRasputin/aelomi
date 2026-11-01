// @vitest-environment happy-dom
import { afterEach, expect, test } from "vitest";
import { createCommandDispatcher } from "./command-dispatcher";
import { connectShortcutEvents } from "./shortcut-events";
import { acceptsUserCommands, type QuitDecision } from "./stores/quit";

afterEach(() => document.body.replaceChildren());

test("SHELL-7 menu commands recover when the composing input is removed", () => {
  const dispatcher = createCommandDispatcher();
  const connection = connectShortcutEvents(dispatcher);
  const input = document.createElement("input");
  document.body.append(input);
  input.focus();
  const actions: string[] = [];
  dispatcher.register("openPalette", {
    run: ({ source }) => actions.push(source),
  });
  try {
    input.dispatchEvent(
      new CompositionEvent("compositionstart", { bubbles: true }),
    );
    connection.menu("openPalette");
    input.remove();
    connection.menu("openPalette");
    expect(actions).toEqual(["menu"]);
  } finally {
    connection.dispose();
  }
});

test("SHELL-7 keyboard composition comes from the event, independent of menu state", () => {
  const dispatcher = createCommandDispatcher();
  const connection = connectShortcutEvents(dispatcher);
  const input = document.createElement("input");
  document.body.append(input);
  input.focus();
  const actions: string[] = [];
  dispatcher.register("openPalette", {
    run: ({ source }) => actions.push(source),
  });
  try {
    input.dispatchEvent(
      new CompositionEvent("compositionstart", { bubbles: true }),
    );
    const event = new KeyboardEvent("keydown", {
      key: "k",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(event);
    expect([event.defaultPrevented, actions]).toEqual([true, ["keyboard"]]);
  } finally {
    connection.dispose();
  }
});

test("SHELL-7 user commands stop at Draining, before IPC admission closes", () => {
  let decision: QuitDecision = { kind: "open", revision: 1 };
  const dispatcher = createCommandDispatcher(() =>
    acceptsUserCommands(decision),
  );
  const writes: string[] = [];
  dispatcher.register("toggleSidebar", {
    run: () => writes.push("expanded rows"),
  });
  dispatcher.dispatch("toggleSidebar", {
    source: "menu",
    target: null,
    isComposing: false,
  });
  decision = { kind: "draining", revision: 2 };
  const event = new KeyboardEvent("keydown", {
    key: "\\",
    metaKey: true,
    cancelable: true,
  });
  dispatcher.keydown(event, null);
  dispatcher.dispatch("toggleSidebar", {
    source: "menu",
    target: null,
    isComposing: false,
  });
  expect([event.defaultPrevented, writes]).toEqual([true, ["expanded rows"]]);
});
