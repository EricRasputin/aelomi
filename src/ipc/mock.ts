import type { EventCallback } from "@tauri-apps/api/event";
import * as Y from "yjs";
import { fromBase64, toBase64 } from "./base64";
import type {
  commands as generatedCommands,
  events as generatedEvents,
  Health,
  QuitBlocked,
  StorageError,
  StorageStatus,
} from "./bindings";

type Commands = typeof generatedCommands;
type Events = typeof generatedEvents;
type MockCall = {
  [Name in keyof Commands]: {
    command: Name;
    args: Parameters<Commands[Name]>;
  };
}[keyof Commands];

const calls: MockCall[] = [];

type MockDoc = { doc: Y.Doc; updates: Uint8Array[]; pending: boolean };

const storage = {
  status: "Ready" as StorageStatus,
  failWrites: false,
  docs: new Map<string, MockDoc>(),
  seq: 0,
  health: "Healthy" as Health,
};

const listeners = new Map<string, Set<EventCallback<unknown>>>();
let eventId = 0;

function mockEvent<T>(name: string) {
  const listen = async (cb: EventCallback<T>) => {
    const set = listeners.get(name) ?? new Set();
    listeners.set(name, set);
    const listener = cb as EventCallback<unknown>;
    set.add(listener);
    return () => {
      set.delete(listener);
    };
  };
  const once = async (cb: EventCallback<T>) => {
    const unlisten = await listen((event) => {
      unlisten();
      cb(event);
    });
    return unlisten;
  };
  const emit = async (payload: T) => {
    eventId += 1;
    for (const cb of [...(listeners.get(name) ?? [])])
      cb({ event: name, id: eventId, payload });
  };
  const base = { listen, once, emit };
  return Object.assign(() => base, base);
}

export const events = {
  quitBlocked: mockEvent<QuitBlocked>("quit-blocked"),
  storageHealthChanged: mockEvent<Health>("storage-health-changed"),
} satisfies Events;

function refreshHealth() {
  const pending = [...storage.docs]
    .filter(([, doc]) => doc.pending)
    .map(([id]) => id);
  const next: Health =
    pending.length === 0
      ? "Healthy"
      : {
          Failing: {
            reason: { Io: "mock write failure" },
            pending_docs: pending,
          },
        };
  if (JSON.stringify(next) === JSON.stringify(storage.health)) return;
  storage.health = next;
  void events.storageHealthChanged.emit(next);
}

function refusal(docId: string): StorageError | null {
  if (storage.status !== "Ready") return "Unavailable";
  if (!/^[0-9a-z-]{1,64}$/.test(docId)) return "InvalidDocId";
  return null;
}

function mockDoc(docId: string): MockDoc {
  let doc = storage.docs.get(docId);
  if (!doc) {
    doc = { doc: new Y.Doc(), updates: [], pending: false };
    storage.docs.set(docId, doc);
  }
  return doc;
}

export const commands = {
  ping: async (request) => {
    calls.push({ command: "ping", args: [structuredClone(request)] });
    return { seq: request.seq, core_version: "0.1.0-mock" };
  },
  perfReport: async (marks) => {
    calls.push({ command: "perfReport", args: [structuredClone(marks)] });
  },
  storageStatus: async () => {
    calls.push({ command: "storageStatus", args: [] });
    return storage.status;
  },
  storageHealth: async () => {
    calls.push({ command: "storageHealth", args: [] });
    return storage.health;
  },
  docLoad: async (docId) => {
    calls.push({ command: "docLoad", args: [docId] });
    const error = refusal(docId);
    if (error) return { status: "error", error };
    const { doc } = mockDoc(docId);
    return { status: "ok", data: toBase64(Y.encodeStateAsUpdate(doc)) };
  },
  docAppend: async (docId, update) => {
    calls.push({ command: "docAppend", args: [docId, update] });
    const error = refusal(docId);
    if (error) return { status: "error", error };
    const doc = mockDoc(docId);
    let bytes: Uint8Array;
    try {
      bytes = fromBase64(update);
      Y.applyUpdate(doc.doc, bytes);
    } catch {
      return { status: "error", error: "InvalidUpdate" };
    }
    doc.updates.push(bytes);
    doc.pending = storage.failWrites;
    refreshHealth();
    storage.seq += 1;
    return { status: "ok", data: storage.seq };
  },
  devProbeLog: async (line) => {
    calls.push({ command: "devProbeLog", args: [line] });
  },
} satisfies Commands;

const control = {
  get calls(): readonly MockCall[] {
    return calls;
  },
  reset() {
    calls.length = 0;
    storage.status = "Ready";
    storage.failWrites = false;
    storage.docs.clear();
    storage.seq = 0;
    storage.health = "Healthy";
  },
  storage: {
    /** While on, appends stay pending and health reports the failure. */
    failWrites(on: boolean) {
      storage.failWrites = on;
      if (!on) for (const doc of storage.docs.values()) doc.pending = false;
      refreshHealth();
    },
    setStatus(status: StorageStatus) {
      storage.status = status;
    },
    /** Every update appended to `docId`, in order, pending or not. */
    updates(docId: string): Uint8Array[] {
      return (storage.docs.get(docId)?.updates ?? []).map((u) => u.slice());
    },
  },
};

declare global {
  interface Window {
    __aelomiMock?: typeof control;
  }
}

export function installMock(): { commands: Commands; events: Events } {
  window.__aelomiMock = control;
  return { commands, events };
}
