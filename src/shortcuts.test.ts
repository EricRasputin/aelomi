import { expect, test } from "vitest";
import { shortcutFor, shortcuts } from "./shortcuts";

const key = (init: KeyboardEventInit & { key: string }) =>
  ({
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...init,
  }) as KeyboardEvent;

test("SHELL-7 no shortcut is assigned twice", () => {
  const combos = shortcuts.map((s) => `${s.meta ? "Meta+" : ""}${s.key}`);
  expect(new Set(combos).size).toBe(combos.length);
});

test("CMD-1 ⌘K opens the palette and nothing else does", () => {
  expect(shortcutFor(key({ key: "k", metaKey: true }))).toBe("openPalette");
  expect(shortcutFor(key({ key: "K", metaKey: true }))).toBe("openPalette");
  expect(shortcutFor(key({ key: "k" }))).toBe(null);
  expect(shortcutFor(key({ key: "k", metaKey: true, shiftKey: true }))).toBe(
    null,
  );
  expect(shortcutFor(key({ key: "k", ctrlKey: true }))).toBe(null);
});
