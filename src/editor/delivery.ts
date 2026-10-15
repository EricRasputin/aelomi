import { create } from "zustand";
import { commands, type StorageError } from "../ipc";
import { toBase64 } from "../ipc/base64";

type DocDelivery = {
  outstanding: Set<Promise<void>>;
  failed: Uint8Array[];
  error: StorageError | null;
};

type DeliveryStore = {
  failing: ReadonlyMap<string, StorageError>;
};

/**
 * Source delivery failures: `doc_append` itself was refused, so the store
 * never held the update. Disk failures after acceptance arrive as `Health`.
 */
export const useDeliveryStore = create<DeliveryStore>(() => ({
  failing: new Map(),
}));

const deliveries = new Map<string, DocDelivery>();
const loads = new Set<Promise<unknown>>();

export function hasOutstandingDeliveries(): boolean {
  return (
    loads.size > 0 ||
    [...deliveries.values()].some((delivery) => delivery.outstanding.size > 0)
  );
}

/** A drain also waits for doc loads in flight, so it never races an editor binding. */
export function trackLoad<T>(load: Promise<T>): Promise<T> {
  loads.add(load);
  const done = () => loads.delete(load);
  load.then(done, done);
  return load;
}

function deliveryFor(docId: string): DocDelivery {
  let delivery = deliveries.get(docId);
  if (!delivery) {
    delivery = { outstanding: new Set(), failed: [], error: null };
    deliveries.set(docId, delivery);
  }
  return delivery;
}

function publish() {
  const failing = new Map<string, StorageError>();
  for (const [docId, delivery] of deliveries) {
    if (delivery.failed.length > 0 && delivery.error)
      failing.set(docId, delivery.error);
  }
  useDeliveryStore.setState({ failing });
}

function invoke(docId: string, delivery: DocDelivery, update: Uint8Array) {
  const fail = (error: StorageError) => {
    delivery.failed.push(update);
    delivery.error = error;
    publish();
  };
  const sent: Promise<void> = commands
    .docAppend(docId, toBase64(update))
    .then(
      (reply) => {
        if (reply.status === "error") fail(reply.error);
      },
      (error: unknown) =>
        fail({ Io: error instanceof Error ? error.message : String(error) }),
    )
    .finally(() => {
      delivery.outstanding.delete(sent);
    });
  delivery.outstanding.add(sent);
}

function retry(docId: string, delivery: DocDelivery) {
  if (delivery.failed.length === 0) return;
  const failed = delivery.failed.splice(0);
  delivery.error = null;
  publish();
  for (const update of failed) invoke(docId, delivery, update);
}

/**
 * Hands one Yjs update to the store at once: the invoke starts before this
 * returns, so no painted edit waits in the webview. Earlier failures are
 * retried first. Updates commute, so several invokes may be in flight.
 */
export function deliver(docId: string, update: Uint8Array) {
  const delivery = deliveryFor(docId);
  retry(docId, delivery);
  invoke(docId, delivery, update);
}

async function settle(docId: string, delivery: DocDelivery) {
  retry(docId, delivery);
  while (delivery.outstanding.size > 0)
    await Promise.all([...delivery.outstanding]);
}

/**
 * Retries every failed update once and waits for every invoke this webview
 * started, on every doc, including pages already closed and docs whose first
 * update starts while the drain waits. Returns the docs whose updates the
 * store still refused.
 */
export async function drainDeliveries(): Promise<string[]> {
  const retried = new Set<DocDelivery>();
  for (;;) {
    while (loads.size > 0) await Promise.allSettled([...loads]);
    const waiting = [...deliveries].filter(
      ([, delivery]) => !retried.has(delivery) || delivery.outstanding.size > 0,
    );
    if (waiting.length === 0) break;
    await Promise.all(
      waiting.map(([docId, delivery]) => {
        if (retried.has(delivery)) return Promise.all(delivery.outstanding);
        retried.add(delivery);
        return settle(docId, delivery);
      }),
    );
  }
  return [...deliveries]
    .filter(([, delivery]) => delivery.failed.length > 0)
    .map(([docId]) => docId)
    .sort();
}

export function retryDeliveries() {
  for (const [docId, delivery] of deliveries) retry(docId, delivery);
}
