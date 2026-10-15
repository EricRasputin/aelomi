import { create } from "zustand";
import { commands, type StorageStatus } from "../ipc";
import {
  type StorageStartup,
  storageCopy,
  storageStartup,
} from "../storage-copy";

export const useStorageStartupStore = create<{
  startup: StorageStartup;
  receive: (status: StorageStatus) => void;
}>((set) => ({
  startup: { kind: "loading" },
  receive: (status) => set({ startup: storageStartup(status) }),
}));

export function connectStorageStartup() {
  let disposed = false;
  const ready = commands.storageStatus().then(
    (status) => {
      if (!disposed) useStorageStartupStore.getState().receive(status);
    },
    (error: unknown) => {
      if (!disposed)
        useStorageStartupStore.setState({
          startup: {
            kind: "refused",
            title: storageCopy.startupTitle,
            message: storageCopy.startup,
            detail: error instanceof Error ? error.message : String(error),
            recovery: null,
          },
        });
    },
  );
  return {
    ready,
    dispose: () => {
      disposed = true;
    },
  };
}
