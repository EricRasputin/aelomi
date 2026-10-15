import { commands, events } from "../ipc";
import { storageRetry } from "../storage-retry";
import { useQuitStore } from "./quit";
import { useStorageHealthStore } from "./storage-health";

/**
 * Listens for health and quit events before reading their snapshots, then
 * registers this webview for quit drains. Editing waits for that
 * registration, so a refused quit status read or `frontendReady` call is
 * retried until it succeeds or the connection is disposed. `ready` reports
 * the first round's failures.
 */
export function connectStorageEvents() {
  let disposed = false;
  let healthEvents = 0;
  let quitListening = false;
  let quitRead = false;
  let retry: ReturnType<typeof setTimeout> | undefined;
  const unlisteners: (() => void)[] = [];
  const dispose = () => {
    disposed = true;
    clearTimeout(retry);
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
  const register = async () => {
    if (!quitRead) {
      const status = await commands.quitStatus();
      if (disposed) return;
      quitRead = true;
      useQuitStore.getState().receiveQuit(status);
    }
    await commands.frontendReady();
    if (!disposed) useQuitStore.getState().register();
  };
  const retryRegistration = (delayMs: number) => {
    retry = setTimeout(() => {
      if (disposed) return;
      register().catch(() =>
        retryRegistration(Math.min(delayMs * 2, storageRetry.maxMs)),
      );
    }, delayMs);
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
        quitListening = true;
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
    else if (!disposed) {
      quitRead = true;
      useQuitStore.getState().receiveQuit(quit.value);
    }
    if (!disposed && quitListening) {
      try {
        if (quitRead) await register();
        else retryRegistration(storageRetry.firstMs);
      } catch (error: unknown) {
        errors.push(error);
        if (!disposed) retryRegistration(storageRetry.firstMs);
      }
    }
    if (errors.length > 0) {
      throw new AggregateError(errors, "Storage event connection failed");
    }
  })();
  return { ready, dispose };
}
