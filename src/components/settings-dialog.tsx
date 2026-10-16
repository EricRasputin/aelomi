import { Dialog } from "@base-ui/react/dialog";
import { useId, useLayoutEffect, useRef } from "react";
import { restoreFocus } from "../editor/active-editor";
import { useQuitStore } from "../stores/quit";
import { useSettingsDialogStore } from "../stores/settings-dialog";
import { useThemeStore } from "../stores/theme";
import { selectedTheme } from "../theme";

const choices = ["System", "Light", "Dark"] as const;

export function SettingsDialog() {
  const dialog = useSettingsDialogStore((state) => state.dialog);
  const hide = useSettingsDialogStore((state) => state.hide);
  const theme = useThemeStore((state) => state.choice);
  const choose = useThemeStore((state) => state.choose);
  const disabled = useQuitStore((state) => state.decision.kind !== "open");
  const groupId = useId();
  const selected = useRef<HTMLInputElement>(null);
  const returnTo = useRef<Element | null>(null);
  if (dialog.kind === "open") returnTo.current = dialog.returnTo;
  useLayoutEffect(() => {
    if (dialog.kind !== "open") return;
    const page = document.querySelector("main");
    if (page) page.inert = true;
    return () => {
      if (page) page.inert = false;
    };
  }, [dialog.kind]);

  return (
    <Dialog.Root
      open={dialog.kind === "open"}
      onOpenChange={(open) => {
        if (!open) hide();
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop className="settings-backdrop" />
        <Dialog.Popup
          className="settings-dialog"
          initialFocus={selected}
          finalFocus={() => {
            restoreFocus(returnTo.current);
            return false;
          }}
        >
          <div className="settings-heading">
            <Dialog.Close
              className="settings-close"
              aria-label="Close Settings"
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="none"
                aria-hidden="true"
              >
                <path
                  d="m4 4 8 8M12 4l-8 8"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </Dialog.Close>
            <Dialog.Title className="settings-title">Settings</Dialog.Title>
          </div>
          <div className="settings-sections">
            <fieldset className="settings-section" disabled={disabled}>
              <legend>Appearance</legend>
              <Dialog.Description className="settings-description">
                System follows your Mac's appearance.
              </Dialog.Description>
              <div className="theme-choices">
                {choices.map((choice) => (
                  <label key={choice} className="theme-choice">
                    <input
                      ref={selectedTheme(theme) === choice ? selected : null}
                      type="radio"
                      name={groupId}
                      value={choice}
                      checked={selectedTheme(theme) === choice}
                      onChange={() => choose(choice)}
                      onClick={() => {
                        if (typeof theme !== "string" && choice === "System")
                          choose(choice);
                      }}
                    />
                    <span>{choice}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
