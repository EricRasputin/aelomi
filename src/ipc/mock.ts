import type { EventCallback } from "@tauri-apps/api/event";
import * as Y from "yjs";
import { fromBase64, toBase64 } from "./base64";
import type {
  FailReason,
  commands as generatedCommands,
  events as generatedEvents,
  Health,
  IndexChanged,
  IndexStatus,
  QueryError,
  QuitStatus,
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

const storage: {
  status: StorageStatus;
  failWrites: boolean;
  docs: Map<string, MockDoc>;
  seq: number;
  health: Health;
  reason: FailReason;
} = {
  status: "Ready",
  failWrites: false,
  docs: new Map<string, MockDoc>(),
  seq: 0,
  health: "Healthy",
  reason: { Io: "mock write failure" },
};
let exited = false;
let indexStatus: IndexStatus = "Ready";

type QuitState =
  | { kind: "open" }
  | { kind: "asking"; pendingDocs: string[] }
  | { kind: "confirmed" };
const quit: { revision: number; state: QuitState } = {
  revision: 0,
  state: { kind: "open" },
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
  indexChanged: mockEvent<IndexChanged>("index-changed"),
  indexStatusChanged: mockEvent<IndexStatus>("index-status-changed"),
  quitStatusChanged: mockEvent<QuitStatus>("quit-status-changed"),
  storageHealthChanged: mockEvent<Health>("storage-health-changed"),
} satisfies Events;

function quitStatus(): QuitStatus {
  return {
    revision: quit.revision,
    pending_docs:
      quit.state.kind === "asking" ? [...quit.state.pendingDocs] : null,
  };
}

function changeQuit(state: QuitState) {
  quit.revision += 1;
  quit.state = state;
  void events.quitStatusChanged.emit(quitStatus());
}

function refreshHealth() {
  const pending = [...storage.docs]
    .filter(([, doc]) => doc.pending)
    .map(([id]) => id);
  const next: Health =
    pending.length === 0
      ? "Healthy"
      : {
          Failing: {
            reason: storage.reason,
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

function queryFailure(): QueryError | null {
  if (storage.status !== "Ready") return "Unavailable";
  if (typeof indexStatus === "object") return structuredClone(indexStatus);
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
  indexStatus: async () => {
    calls.push({ command: "indexStatus", args: [] });
    if (storage.status !== "Ready")
      return { Failed: { reason: "the data folder is not open" } };
    return structuredClone(indexStatus);
  },
  pageTree: async () => {
    calls.push({ command: "pageTree", args: [] });
    if (storage.status !== "Ready")
      return { status: "error", error: "Unavailable" };
    return { status: "ok", data: { pages: [], favorites: [], trash: [] } };
  },
  search: async (query, limit) => {
    calls.push({ command: "search", args: [query, limit] });
    const error = queryFailure();
    if (error) return { status: "error", error };
    return {
      status: "ok",
      data: indexStatus === "Rebuilding" ? "Indexing" : { Ready: [] },
    };
  },
  backlinks: async (pageId) => {
    calls.push({ command: "backlinks", args: [pageId] });
    if (pageId === "workspace" || !/^[0-9a-z-]{1,64}$/.test(pageId))
      return { status: "error", error: "InvalidPageId" };
    const error = queryFailure();
    if (error) return { status: "error", error };
    return {
      status: "ok",
      data: indexStatus === "Rebuilding" ? "Indexing" : { Ready: [] },
    };
  },
  quitStatus: async () => {
    calls.push({ command: "quitStatus", args: [] });
    return quitStatus();
  },
  keepOpen: async (revision) => {
    calls.push({ command: "keepOpen", args: [revision] });
    if (quit.state.kind === "asking" && quit.revision === revision) {
      changeQuit({ kind: "open" });
    }
    return quitStatus();
  },
  quitAnyway: async () => {
    calls.push({ command: "quitAnyway", args: [] });
    if (quit.state.kind === "confirmed") return;
    changeQuit({ kind: "confirmed" });
    exited = true;
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
    storage.reason = { Io: "mock write failure" };
    exited = false;
    indexStatus = "Ready";
    quit.revision = 0;
    quit.state = { kind: "open" };
  },
  quit: {
    get exited() {
      return exited;
    },
    request() {
      if (exited) return;
      const pendingDocs = [...storage.docs]
        .filter(([, doc]) => doc.pending)
        .map(([id]) => id);
      if (pendingDocs.length === 0) exited = true;
      else changeQuit({ kind: "asking", pendingDocs });
    },
  },
  index: {
    setStatus(status: IndexStatus) {
      indexStatus = structuredClone(status);
      void events.indexStatusChanged.emit(structuredClone(status));
    },
  },
  storage: {
    /** While on, appends stay pending and health reports the failure. */
    failWrites(on: boolean, reason: FailReason = { Io: "mock write failure" }) {
      storage.failWrites = on;
      storage.reason = reason;
      if (!on) for (const doc of storage.docs.values()) doc.pending = false;
      refreshHealth();
    },
    async type(docId: string, text: string) {
      const loaded = await commands.docLoad(docId);
      if (loaded.status === "error") return loaded;
      const doc = new Y.Doc();
      try {
        Y.applyUpdate(doc, fromBase64(loaded.data));
        const before = Y.encodeStateVector(doc);
        const body = doc.getText("body");
        body.insert(body.length, text);
        return await commands.docAppend(
          docId,
          toBase64(Y.encodeStateAsUpdate(doc, before)),
        );
      } finally {
        doc.destroy();
      }
    },
    text(docId: string): string {
      return storage.docs.get(docId)?.doc.getText("body").toString() ?? "";
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
