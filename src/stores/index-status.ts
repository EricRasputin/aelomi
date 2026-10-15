import { create } from "zustand";
import { commands, events, type IndexStatus } from "../ipc";

type IndexStatusStore = {
  status: IndexStatus | null;
  /**
   * Grows with every status or index change this webview applies. The core's
   * status carries no revision, so this local count orders what the palette
   * has seen.
   */
  revision: number;
  receiveStatus: (status: IndexStatus) => void;
  receiveChange: () => void;
};

export const useIndexStatusStore = create<IndexStatusStore>((set) => ({
  status: null,
  revision: 0,
  receiveStatus: (status) =>
    set((state) => ({ status, revision: state.revision + 1 })),
  receiveChange: () => set((state) => ({ revision: state.revision + 1 })),
}));

/**
 * Listens before it reads, as `connectStorageEvents` does: a status event
 * that arrives before the snapshot wins over it, because the core publishes a
 * status before it emits it.
 */
export function connectIndexEvents() {
  let disposed = false;
  let statusEvents = 0;
  const unlisteners: (() => void)[] = [];
  const dispose = () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
  const keep = (unlisten: () => void) => {
    if (disposed) unlisten();
    else unlisteners.push(unlisten);
  };
  const ready = (async () => {
    const registrations = await Promise.allSettled([
      events.indexStatusChanged
        .listen(({ payload }) => {
          if (disposed) return;
          statusEvents += 1;
          useIndexStatusStore.getState().receiveStatus(payload);
        })
        .then(keep),
      events.indexChanged
        .listen(() => {
          if (disposed) return;
          useIndexStatusStore.getState().receiveChange();
        })
        .then(keep),
    ]);
    if (disposed) return;
    const errors: unknown[] = registrations.flatMap((registration) =>
      registration.status === "rejected" ? [registration.reason] : [],
    );
    try {
      const status = await commands.indexStatus();
      if (!disposed && statusEvents === 0)
        useIndexStatusStore.getState().receiveStatus(status);
    } catch (error: unknown) {
      errors.push(error);
    }
    if (errors.length > 0)
      throw new AggregateError(errors, "Index event connection failed");
  })();
  return { ready, dispose };
}
