import {
  createExtension,
  type ExtensionOptions,
  type SuggestionMenuOptions,
} from "@blocknote/core";

export type PickerKey = "ArrowUp" | "ArrowDown" | "Enter" | "Escape";

export const pickerTrigger = "[[";

/**
 * The `[[` picker's keys. They reach the picker only while it is open; the
 * handler returns false otherwise so the editor keeps them.
 */
export const pagePicker = createExtension(
  ({ options }: ExtensionOptions<{ onKey: (key: PickerKey) => boolean }>) => ({
    key: "aelomiPagePicker",
    runsBefore: ["default"],
    keyboardShortcuts: {
      ArrowUp: () => options.onKey("ArrowUp"),
      ArrowDown: () => options.onKey("ArrowDown"),
      Enter: () => options.onKey("Enter"),
      Escape: () => options.onKey("Escape"),
    },
  }),
);

type Transaction = Parameters<
  NonNullable<SuggestionMenuOptions["shouldOpen"]>
>[0];

export function inCode(tr: Transaction): boolean {
  const { $from } = tr.selection;
  if ($from.parent.type.spec.code) return true;
  const marks = tr.storedMarks ?? $from.marks();
  return marks.some((mark) => mark.type.name === "code");
}
