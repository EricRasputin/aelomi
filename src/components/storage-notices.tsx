import { AlertDialog } from "@base-ui/react/alert-dialog";
import { useRef } from "react";
import { useDeliveryStore } from "../editor/delivery";
import {
  recordErrorText,
  storageCopy,
  storageErrorText,
  storageFailureReason,
} from "../storage-copy";
import { quitDialogOpen, useQuitStore } from "../stores/quit";
import { failing, useRecordCommandStore } from "../stores/record-commands";
import { useStorageHealthStore } from "../stores/storage-health";
import { useThemeStore } from "../stores/theme";

export function SaveFailureBanner() {
  const health = useStorageHealthStore((state) => state.health);
  const refused = useDeliveryStore(
    (state) => state.failing.values().next().value ?? null,
  );
  const record = useRecordCommandStore(
    (state) => state.retained.values().next().value ?? null,
  );
  const failed = useRecordCommandStore(
    (state) => state.failed.values().next().value ?? null,
  );
  const dismissFailed = useRecordCommandStore((state) => state.dismissFailed);
  const recordFailing = useRecordCommandStore(failing);
  const recordFailure = record ?? failed;
  const settingsFailure = useThemeStore((state) => state.transportFailure);
  const reason =
    health && health !== "Healthy"
      ? storageFailureReason(health.Failing.reason)
      : refused
        ? storageErrorText(refused)
        : recordFailing && recordFailure
          ? recordErrorText(recordFailure.error)
          : settingsFailure;
  if (reason === null) return null;
  return (
    <div role="alert" className="save-failure-banner">
      <p>
        <span className="font-medium">{storageCopy.banner}</span> {reason}
      </p>
      {record || !failed ? <p className="mt-1">{storageCopy.retry}</p> : null}
      {failed ? (
        <button
          type="button"
          className="quit-button mt-1"
          onClick={() => dismissFailed(failed.id)}
        >
          Dismiss failed change
        </button>
      ) : null}
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
            {decision.kind === "asking" && decision.pendingSettings
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
