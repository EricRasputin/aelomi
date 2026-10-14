export const shortcuts = [
  {
    command: "openPalette",
    label: "Search and commands",
    key: "k",
    meta: true,
  },
] as const;

export type ShortcutCommand = (typeof shortcuts)[number]["command"];

export function shortcutFor(event: KeyboardEvent): ShortcutCommand | null {
  for (const shortcut of shortcuts) {
    if (
      event.key.toLowerCase() === shortcut.key &&
      event.metaKey === shortcut.meta &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey
    )
      return shortcut.command;
  }
  return null;
}
