import { commands, events } from "../ipc";
import { useQuitStore } from "./quit";
import { useStorageHealthStore } from "./storage-health";

export function connectStorageEvents() {
  let disposed = false;
  let healthEvents = 0;
  const unlisteners: (() => void)[] = [];
  const dispose = () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
  const ready = (async () => {
    const registrations = await Promise.allSettled([
      (async () => {
        const healthUnlisten = await events.storageHealthChanged.listen(
          ({ payload }) => {
            if (disposed) return;
            healthEvents += 1;
            useStorageHealthStore.getState().receiveHealth(payload);
          },
        );
        if (disposed) {
          healthUnlisten();
          return;
        }
        unlisteners.push(healthUnlisten);
      })(),
      (async () => {
        const quitUnlisten = await events.quitBlocked.listen(({ payload }) => {
          if (!disposed) useQuitStore.getState().receiveBlocked(payload);
        });
        if (disposed) {
          quitUnlisten();
          return;
        }
        unlisteners.push(quitUnlisten);
      })(),
    ]);
    if (disposed) return;
    const errors: unknown[] = registrations.flatMap((registration) =>
      registration.status === "rejected" ? [registration.reason] : [],
    );
    try {
      const health = await commands.storageHealth();
      if (!disposed && healthEvents === 0) {
        useStorageHealthStore.getState().receiveHealth(health);
      }
    } catch (error: unknown) {
      errors.push(error);
    }
    if (errors.length > 0) {
      throw new AggregateError(errors, "Storage event connection failed");
    }
  })();
  return { ready, dispose };
}
