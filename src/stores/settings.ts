import { create } from "zustand";
import { commands, type SettingsError, type SettingsPatch } from "../ipc";
import {
  forgetCommand,
  retainCommand,
  trackCommand,
} from "../ipc/pending-commands";
import { storageRetry } from "../storage-retry";
import type { TransportFailure } from "./record-commands";

export type SettingsWriteError = SettingsError | TransportFailure;
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

export function createSettingsWriteJob(
  update: UpdateSettings,
  report: (error: SettingsWriteError | null) => void,
) {
  let pending: SettingsPatch | null = null;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let backoff = storageRetry.firstMs;
  const job = {};
  const pump = () => {
    clearTimeout(timer);
    if (running || pending === null) return;
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
              pending = mergePatch(patch, pending);
              retainCommand(job, { retry: pump, docs: [], settings: true });
              report(failure);
              timer = setTimeout(pump, backoff);
              backoff = Math.min(backoff * 2, storageRetry.maxMs);
              return;
            }
            forgetCommand(job);
            report(null);
            backoff = storageRetry.firstMs;
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
  error: SettingsWriteError | null;
}>(() => ({ error: null }));

export const updateSettings = createSettingsWriteJob(
  (patch) => commands.settingsUpdate(patch),
  (error) => useSettingsWriteStore.setState({ error }),
);
