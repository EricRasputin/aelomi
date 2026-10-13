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
        const quitUnlisten = await events.quitStatusChanged.listen(
          ({ payload }) => {
            if (!disposed) useQuitStore.getState().receiveQuit(payload);
          },
        );
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
    const [health, quit] = await Promise.allSettled([
      commands.storageHealth(),
      commands.quitStatus(),
    ]);
    if (health.status === "rejected") errors.push(health.reason);
    else if (!disposed && healthEvents === 0) {
      useStorageHealthStore.getState().receiveHealth(health.value);
    }
    if (quit.status === "rejected") errors.push(quit.reason);
    else if (!disposed) useQuitStore.getState().receiveQuit(quit.value);
    if (errors.length > 0) {
      throw new AggregateError(errors, "Storage event connection failed");
    }
  })();
  return { ready, dispose };
}
