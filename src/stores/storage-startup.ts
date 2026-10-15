import { create } from "zustand";
import { commands, events, type StorageStatus } from "../ipc";
import { type StorageStartup, storageStartup } from "../storage-copy";

export const useStorageStartupStore = create<{
  startup: StorageStartup;
  receive: (status: StorageStatus) => void;
}>((set) => ({
  startup: { kind: "loading" },
  receive: (status) => set({ startup: storageStartup(status) }),
}));

export function connectStorageStartup() {
  let disposed = false;
  let changed = false;
  let unlisten: (() => void) | undefined;
  const receive = (status: StorageStatus) => {
    if (!disposed) useStorageStartupStore.getState().receive(status);
  };
  const ready = (async () => {
    try {
      unlisten = await events.storageStatusChanged.listen(({ payload }) => {
        changed = true;
        receive(payload);
      });
      if (disposed) {
        unlisten();
        return;
      }
      const status = await commands.storageStatus();
      if (!changed) receive(status);
    } catch (error: unknown) {
      if (!changed)
        receive({ Io: error instanceof Error ? error.message : String(error) });
    }
  })();
  return {
    ready,
    dispose: () => {
      disposed = true;
      unlisten?.();
    },
  };
}
