import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import {
  matchesShortcut,
  nativeAccelerator,
  serializeNativeMenu,
  shortcutFor,
  shortcuts,
  webviewBindings,
} from "./shortcuts";

const key = (init: Partial<Parameters<typeof shortcutFor>[0]>) => ({
  key: "k",
  code: "KeyK",
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  isComposing: false,
  ...init,
});

test("SHELL-7 no shortcut is assigned twice including every modifier", () => {
  const combos = shortcuts.flatMap((entry) =>
    entry.shortcut
      ? [
          [...entry.shortcut.modifiers].sort().join("+") +
            "+" +
            entry.shortcut.key.toLowerCase(),
        ]
      : [],
  );
  expect(new Set(combos).size).toBe(combos.length);
});

test("SHELL-7 the committed native menu is generated from the shortcut table", () => {
  expect(
    readFileSync(new URL("../src-tauri/menu.json", import.meta.url), "utf8"),
  ).toBe(serializeNativeMenu());
});

test("SHELL-7 the dispatcher binds exactly the webview-owned shortcuts", () => {
  expect(webviewBindings.map((entry) => entry.command)).toEqual([
    "openSettings",
    "newPage",
    "closeWindow",
    "undo",
    "redo",
    "openPalette",
    "toggleSidebar",
  ]);
  for (const entry of shortcuts) {
    if (!entry.shortcut) continue;
    const event = key({
      key: entry.shortcut.key,
      metaKey: entry.shortcut.modifiers.some((modifier) => modifier === "Meta"),
      ctrlKey: entry.shortcut.modifiers.some(
        (modifier) => modifier === "Control",
      ),
      altKey: entry.shortcut.modifiers.some((modifier) => modifier === "Alt"),
      shiftKey: entry.shortcut.modifiers.some(
        (modifier) => modifier === "Shift",
      ),
      code: "",
    });
    expect(shortcutFor(event)).toBe(
      entry.owner === "webview" ? entry.command : null,
    );
  }
});

test("CMD-1 ⌘K opens the palette and nothing else does", () => {
  expect(shortcutFor(key({ metaKey: true }))).toBe("openPalette");
  expect(shortcutFor(key({ key: "K", metaKey: true }))).toBe("openPalette");
  expect(shortcutFor(key({}))).toBe(null);
  expect(shortcutFor(key({ metaKey: true, shiftKey: true }))).toBe(null);
  expect(shortcutFor(key({ ctrlKey: true }))).toBe(null);
  expect(shortcutFor(key({ metaKey: true, isComposing: true }))).toBe(null);
});

test("SHELL-7 Shift, Option and Control must match exactly", () => {
  expect(shortcutFor(key({ key: "Z", metaKey: true, shiftKey: true }))).toBe(
    "redo",
  );
  expect(shortcutFor(key({ key: "z", metaKey: true }))).toBe("undo");
  expect(shortcutFor(key({ key: "z", metaKey: true, altKey: true }))).toBe(
    null,
  );
  expect(
    matchesShortcut(
      key({ key: "˙", code: "KeyH", metaKey: true, altKey: true }),
      { key: "h", modifiers: ["Meta", "Alt"] },
    ),
  ).toBe(true);
  expect(
    matchesShortcut(key({ key: "f", metaKey: true, ctrlKey: true }), {
      key: "f",
      modifiers: ["Meta", "Control"],
    }),
  ).toBe(true);
  expect(nativeAccelerator({ key: "\\", modifiers: ["Meta"] })).toBe(
    "Super+Backslash",
  );
  expect(nativeAccelerator({ key: ",", modifiers: ["Meta"] })).toBe(
    "Super+Comma",
  );
  expect(nativeAccelerator({ key: "z", modifiers: ["Meta", "Shift"] })).toBe(
    "Super+Shift+Z",
  );
});
