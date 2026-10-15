import { AlertDialog } from "@base-ui/react/alert-dialog";
import { useRef } from "react";
import { useDeliveryStore } from "../editor/delivery";
import {
  storageCopy,
  storageErrorText,
  storageFailureReason,
} from "../storage-copy";
import { quitDialogOpen, useQuitStore } from "../stores/quit";
import { useStorageHealthStore } from "../stores/storage-health";

export function SaveFailureBanner() {
  const health = useStorageHealthStore((state) => state.health);
  const refused = useDeliveryStore(
    (state) => state.failing.values().next().value ?? null,
  );
  const reason =
    health && health !== "Healthy"
      ? storageFailureReason(health.Failing.reason)
      : refused
        ? storageErrorText(refused)
        : null;
  if (reason === null) return null;
  return (
    <div role="alert" className="save-failure-banner">
      <p>
        <span className="font-medium">{storageCopy.banner}</span> {reason}
      </p>
      <p className="mt-1">{storageCopy.retry}</p>
    </div>
  );
}

export function QuitConfirmation() {
  const decision = useQuitStore((state) => state.decision);
  const keepOpen = useQuitStore((state) => state.keepOpen);
  const quitAnyway = useQuitStore((state) => state.quitAnyway);
  const keepButton = useRef<HTMLButtonElement>(null);
  const quitting = decision.kind === "quitting";

  return (
    <AlertDialog.Root
      open={quitDialogOpen(decision)}
      onOpenChange={(open, eventDetails) => {
        if (open) return;
        if (quitting) eventDetails.cancel();
        else void keepOpen();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="quit-overlay" />
        <AlertDialog.Popup
          className="quit-dialog"
          aria-busy={quitting}
          initialFocus={keepButton}
        >
          <AlertDialog.Title className="text-base font-semibold">
            {storageCopy.quitTitle}
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-3 text-sm leading-6 text-[var(--muted)]">
            {storageCopy.quitWarning}
            {decision.kind === "asking" &&
            decision.pendingDocs.includes("settings.json")
              ? ` ${storageCopy.settingsPending}`
              : null}
          </AlertDialog.Description>
          {decision.kind === "asking" && decision.error && (
            <p role="alert" className="mt-3 text-sm text-[var(--warning-text)]">
              {decision.error}
            </p>
          )}
          <div className="mt-6 flex flex-wrap justify-end gap-2">
            <AlertDialog.Close
              ref={keepButton}
              disabled={quitting}
              className="quit-button"
            >
              {storageCopy.keepOpen}
            </AlertDialog.Close>
            <button
              type="button"
              disabled={quitting}
              onClick={() => void quitAnyway()}
              className="quit-button quit-button-danger"
            >
              {storageCopy.quitAnyway}
            </button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
