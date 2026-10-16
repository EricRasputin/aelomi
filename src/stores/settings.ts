import { create } from "zustand";
import { isPageId } from "../editor/body-reader";
import { commands, type SettingsError, type SettingsPatch } from "../ipc";
import { retryCommandError, type TransportFailure } from "../ipc/command-retry";
import {
  commandsAccepted,
  forgetCommand,
  onCommandAdmissionOpen,
  retainCommand,
  trackCommand,
} from "../ipc/pending-commands";
import { storageRetry } from "../storage-retry";

export type SettingsWriteError = SettingsError | TransportFailure;
export type SettingsWriteFailure = {
  kind: "retrying" | "failed";
  error: SettingsWriteError;
};
type UpdateSettings = typeof commands.settingsUpdate;

function mergePatch(
  older: SettingsPatch | null,
  newer: SettingsPatch | null,
): SettingsPatch {
  const patch = { ...older };
  if (newer?.theme != null) patch.theme = newer.theme;
  if (newer?.open_page != null) patch.open_page = newer.open_page;
  if (newer?.recent_pages != null) patch.recent_pages = newer.recent_pages;
  if (newer?.expanded_rows != null) patch.expanded_rows = newer.expanded_rows;
  return patch;
}

function withoutInvalidPageFields(patch: SettingsPatch): SettingsPatch | null {
  const valid = { ...patch };
  if (patch.expanded_rows?.some((id) => !isPageId(id)))
    delete valid.expanded_rows;
  if (patch.recent_pages?.some((id) => !isPageId(id)))
    delete valid.recent_pages;
  if (
    patch.open_page &&
    typeof patch.open_page === "object" &&
    !isPageId(patch.open_page.Set.page_id)
  )
    delete valid.open_page;
  return Object.keys(valid).length < Object.keys(patch).length &&
    Object.keys(valid).length > 0
    ? valid
    : null;
}

export function createSettingsWriteJob(
  update: UpdateSettings,
  report: (failure: SettingsWriteFailure | null) => void,
) {
  let pending: SettingsPatch | null = null;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let backoff = storageRetry.firstMs;
  const job = {};
  let retainedError: SettingsWriteError | null = null;
  let stopAdmission: (() => void) | undefined;
  const forget = () => {
    forgetCommand(job);
    retainedError = null;
    stopAdmission?.();
    stopAdmission = undefined;
    backoff = storageRetry.firstMs;
  };
  const pump = () => {
    clearTimeout(timer);
    if (running || pending === null || !commandsAccepted()) return;
    running = true;
    void trackCommand(
      (async () => {
        try {
          while (pending !== null) {
            const patch = pending;
            pending = null;
            let failure: SettingsWriteError | null = null;
            try {
              const reply = await update(patch);
              if (
                reply.status === "error" &&
                !(typeof reply.error === "object" && "Write" in reply.error)
              )
                failure = reply.error;
            } catch (error: unknown) {
              failure = {
                Transport:
                  error instanceof Error ? error.message : String(error),
              };
            }
            if (failure !== null) {
              if (retryCommandError(failure, retainedError !== null)) {
                pending = mergePatch(patch, pending);
                retainCommand(job, { retry: pump, docs: [], settings: true });
                if (failure !== "Quitting" || retainedError === null) {
                  retainedError = failure;
                  report({ kind: "retrying", error: failure });
                }
                stopAdmission ??= onCommandAdmissionOpen(pump);
                timer = setTimeout(pump, backoff);
                backoff = Math.min(backoff * 2, storageRetry.maxMs);
                return;
              }
              if (failure === "InvalidPageId") {
                const valid = withoutInvalidPageFields(patch);
                if (valid !== null) pending = mergePatch(valid, pending);
              }
              forget();
              report({ kind: "failed", error: failure });
              continue;
            }
            forget();
            report(null);
          }
        } finally {
          running = false;
        }
      })(),
    );
  };
  return (patch: SettingsPatch) => {
    pending = mergePatch(pending, patch);
    pump();
  };
}

export const useSettingsWriteStore = create<{
  failure: SettingsWriteFailure | null;
  dismissFailed: () => void;
}>((set, get) => ({
  failure: null,
  dismissFailed: () => {
    if (get().failure?.kind === "failed") set({ failure: null });
  },
}));

export const updateSettings = createSettingsWriteJob(
  (patch) => commands.settingsUpdate(patch),
  (failure) => useSettingsWriteStore.setState({ failure }),
);
