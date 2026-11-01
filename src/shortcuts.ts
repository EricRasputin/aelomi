export type Modifier = "Meta" | "Control" | "Alt" | "Shift";
export type Shortcut = { key: string; modifiers: readonly Modifier[] };

export const nativeMenus = [
  { id: "app", label: "Aelomi" },
  { id: "file", label: "File" },
  { id: "edit", label: "Edit" },
  { id: "view", label: "View" },
  { id: "window", label: "Window" },
  { id: "help", label: "Help" },
] as const;

type Placement = {
  menu: (typeof nativeMenus)[number]["id"];
  group: number;
  order: number;
};
type NativeRole =
  | "about"
  | "services"
  | "hide"
  | "hideOthers"
  | "showAll"
  | "quit"
  | "closeWindow"
  | "cut"
  | "copy"
  | "paste"
  | "selectAll"
  | "fullscreen"
  | "minimize"
  | "zoom"
  | "bringAllToFront";

type CommandDefinition = {
  command: string;
  label: string;
  shortcut: Shortcut | null;
  menu: Placement | null;
} & (
  | { owner: "webview"; role?: never }
  | { owner: "native"; role: NativeRole }
);

export const shortcuts = [
  {
    command: "about",
    label: "About Aelomi",
    owner: "native",
    role: "about",
    shortcut: null,
    menu: { menu: "app", group: 0, order: 0 },
  },
  {
    command: "openSettings",
    label: "Settings…",
    owner: "webview",
    shortcut: { key: ",", modifiers: ["Meta"] },
    menu: { menu: "app", group: 1, order: 0 },
  },
  {
    command: "services",
    label: "Services",
    owner: "native",
    role: "services",
    shortcut: null,
    menu: { menu: "app", group: 2, order: 0 },
  },
  {
    command: "hide",
    label: "Hide Aelomi",
    owner: "native",
    role: "hide",
    shortcut: { key: "h", modifiers: ["Meta"] },
    menu: { menu: "app", group: 3, order: 0 },
  },
  {
    command: "hideOthers",
    label: "Hide Others",
    owner: "native",
    role: "hideOthers",
    shortcut: { key: "h", modifiers: ["Meta", "Alt"] },
    menu: { menu: "app", group: 3, order: 1 },
  },
  {
    command: "showAll",
    label: "Show All",
    owner: "native",
    role: "showAll",
    shortcut: null,
    menu: { menu: "app", group: 3, order: 2 },
  },
  {
    command: "quit",
    label: "Quit Aelomi",
    owner: "native",
    role: "quit",
    shortcut: { key: "q", modifiers: ["Meta"] },
    menu: { menu: "app", group: 4, order: 0 },
  },
  {
    command: "newPage",
    label: "New page",
    owner: "webview",
    shortcut: { key: "n", modifiers: ["Meta"] },
    menu: { menu: "file", group: 0, order: 0 },
  },
  {
    command: "trashPage",
    label: "Move to trash",
    owner: "webview",
    shortcut: null,
    menu: null,
  },
  {
    command: "closeWindow",
    label: "Close Window",
    owner: "webview",
    shortcut: { key: "w", modifiers: ["Meta"] },
    menu: { menu: "file", group: 1, order: 0 },
  },
  {
    command: "undo",
    label: "Undo",
    owner: "webview",
    shortcut: { key: "z", modifiers: ["Meta"] },
    menu: { menu: "edit", group: 0, order: 0 },
  },
  {
    command: "redo",
    label: "Redo",
    owner: "webview",
    shortcut: { key: "z", modifiers: ["Meta", "Shift"] },
    menu: { menu: "edit", group: 0, order: 1 },
  },
  {
    command: "cut",
    label: "Cut",
    owner: "native",
    role: "cut",
    shortcut: { key: "x", modifiers: ["Meta"] },
    menu: { menu: "edit", group: 1, order: 0 },
  },
  {
    command: "copy",
    label: "Copy",
    owner: "native",
    role: "copy",
    shortcut: { key: "c", modifiers: ["Meta"] },
    menu: { menu: "edit", group: 1, order: 1 },
  },
  {
    command: "paste",
    label: "Paste",
    owner: "native",
    role: "paste",
    shortcut: { key: "v", modifiers: ["Meta"] },
    menu: { menu: "edit", group: 1, order: 2 },
  },
  {
    command: "selectAll",
    label: "Select All",
    owner: "native",
    role: "selectAll",
    shortcut: { key: "a", modifiers: ["Meta"] },
    menu: { menu: "edit", group: 1, order: 3 },
  },
  {
    command: "openPalette",
    label: "Search and commands",
    owner: "webview",
    shortcut: { key: "k", modifiers: ["Meta"] },
    menu: { menu: "view", group: 0, order: 0 },
  },
  {
    command: "toggleSidebar",
    label: "Toggle Sidebar",
    owner: "webview",
    shortcut: { key: "\\", modifiers: ["Meta"] },
    menu: { menu: "view", group: 0, order: 1 },
  },
  {
    command: "fullscreen",
    label: "Enter Full Screen",
    owner: "native",
    role: "fullscreen",
    shortcut: { key: "f", modifiers: ["Meta", "Control"] },
    menu: { menu: "view", group: 1, order: 0 },
  },
  {
    command: "minimize",
    label: "Minimize",
    owner: "native",
    role: "minimize",
    shortcut: { key: "m", modifiers: ["Meta"] },
    menu: { menu: "window", group: 0, order: 0 },
  },
  {
    command: "zoom",
    label: "Zoom",
    owner: "native",
    role: "zoom",
    shortcut: null,
    menu: { menu: "window", group: 0, order: 1 },
  },
  {
    command: "bringAllToFront",
    label: "Bring All to Front",
    owner: "native",
    role: "bringAllToFront",
    shortcut: null,
    menu: { menu: "window", group: 1, order: 0 },
  },
] as const satisfies readonly CommandDefinition[];

export type ShortcutCommand = (typeof shortcuts)[number]["command"];
export type WebviewEntry = Extract<
  (typeof shortcuts)[number],
  { owner: "webview" }
>;
export type WebviewCommand = WebviewEntry["command"];
export const webviewBindings = shortcuts.filter(
  (entry) => entry.owner === "webview" && entry.shortcut !== null,
);

type KeyEvent = Pick<
  KeyboardEvent,
  "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "isComposing"
>;

export function matchesShortcut(event: KeyEvent, shortcut: Shortcut): boolean {
  const key =
    event.altKey && event.code.startsWith("Key")
      ? event.code.slice(3).toLowerCase()
      : event.key.toLowerCase();
  return (
    !event.isComposing &&
    key === shortcut.key.toLowerCase() &&
    event.metaKey === shortcut.modifiers.includes("Meta") &&
    event.ctrlKey === shortcut.modifiers.includes("Control") &&
    event.altKey === shortcut.modifiers.includes("Alt") &&
    event.shiftKey === shortcut.modifiers.includes("Shift")
  );
}

export function shortcutFor(event: KeyEvent): WebviewCommand | null {
  return (
    webviewBindings.find((entry) => matchesShortcut(event, entry.shortcut))
      ?.command ?? null
  );
}

export function nativeAccelerator(shortcut: Shortcut | null): string | null {
  if (!shortcut) return null;
  const key =
    shortcut.key === "\\"
      ? "Backslash"
      : shortcut.key === ","
        ? "Comma"
        : shortcut.key.toUpperCase();
  return [
    ...shortcut.modifiers.map((modifier) =>
      modifier === "Meta" ? "Super" : modifier,
    ),
    key,
  ].join("+");
}

export function serializeNativeMenu(): string {
  return `${JSON.stringify(
    nativeMenus.map((menu) => ({
      ...menu,
      items: shortcuts
        .flatMap((entry) =>
          entry.menu?.menu === menu.id ? [{ ...entry, menu: entry.menu }] : [],
        )
        .sort(
          (a, b) => a.menu.group - b.menu.group || a.menu.order - b.menu.order,
        )
        .map((entry) => ({
          command: entry.command,
          label: entry.label,
          group: entry.menu.group,
          accelerator: nativeAccelerator(entry.shortcut),
          action:
            entry.owner === "native"
              ? { owner: entry.owner, role: entry.role }
              : { owner: entry.owner },
        })),
    })),
    null,
    2,
  )}\n`;
}
