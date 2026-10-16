import type { EventCallback } from "@tauri-apps/api/event";
import * as Y from "yjs";
import {
  type Body,
  isPageId,
  readBody,
  renderSegments,
} from "../editor/body-reader";
import { fromBase64, toBase64 } from "./base64";
import {
  type FailReason,
  type commands as generatedCommands,
  type events as generatedEvents,
  type Health,
  type IndexChanged,
  type IndexStatus,
  MAX_EXCLUDED_PAGES,
  type MenuCommand,
  type PageIssue,
  type PageProblem,
  type PageTree,
  type QueryError,
  type QuitPhase,
  type QuitStatus,
  type SearchScope,
  type Settings,
  type StorageError,
  type StorageStatus,
} from "./bindings";
import { type SeedBlock, writeBody } from "./mock-body";
import { createRecordCommands } from "./mock-records";
import { createRecoveryMock, type RecoverySetup } from "./mock-recovery";
import {
  type MockHit,
  PageIndex,
  prepare,
  type Scope,
  type SearchablePage,
  searchPages,
} from "./mock-search";
import { createSettingsMock } from "./mock-settings";
import {
  type PageRecord,
  pagesRootProblem,
  projectTree,
  readRecords,
  type SeedPage,
  trashPage,
  workspaceDocId,
  writeRecords,
} from "./mock-workspace";

type Commands = typeof generatedCommands;
type Events = typeof generatedEvents;
type MockCall = {
  [Name in keyof Commands]: {
    command: Name;
    args: Parameters<Commands[Name]>;
  };
}[keyof Commands];
export type MockSetup = {
  settings?: Partial<Settings>;
  appearance?: "light" | "dark";
  holdRegistration?: boolean;
  failSettingsRead?: boolean;
  storageStatus?: StorageStatus;
  recovery?: RecoverySetup;
  failWrites?: FailReason;
  indexStatus?: IndexStatus;
  pages?: SeedPage[];
  bodies?: Record<string, SeedBlock[]>;
  /** Page bodies as base64 Yjs updates, applied as stored. */
  rawBodies?: Record<string, string>;
  /** A base64 Yjs update applied to the workspace doc as stored. */
  rawWorkspace?: string;
  /** Loads the 10,000-page search fixture as the workspace. */
  searchFixture?: boolean;
};

declare global {
  interface Window {
    __aelomiMockSetup?: MockSetup;
  }
}
const calls: MockCall[] = [];
const errors: string[] = [];
let failTreeRead = false;
let failSettingsRead = false;

type MockDoc = { doc: Y.Doc; updates: Uint8Array[]; pending: boolean };
const storage: {
  status: StorageStatus;
  appendError: StorageError | null;
  recordError: import("./bindings").PageError | null;
  failWrites: boolean;
  docs: Map<string, MockDoc>;
  seq: number;
  health: Health;
  reason: FailReason;
} = {
  status: "Ready",
  appendError: null,
  recordError: null,
  failWrites: false,
  docs: new Map<string, MockDoc>(),
  seq: 0,
  health: "Healthy",
  reason: { Io: "mock write failure" },
};
let exited = false;
let indexStatus: IndexStatus = "Ready";
let settings: ReturnType<typeof createSettingsMock> | undefined;
function settingsMock() {
  settings ??= createSettingsMock({
    ready: () => setupReady,
    status: () => storage.status,
    writeFailure: () => (storage.failWrites ? storage.reason : null),
    changed: () => refreshHealth(),
    record: (call) => calls.push(call),
  });
  return settings;
}
let registration = Promise.resolve();
let releaseRegistration = () => {};
let appearance: "light" | "dark" | null = null;
type FixturePage = { id: string; title: string; lines: string[] };
const fixtureBodies = new Map<string, string[]>();
const quit: { revision: number; phase: QuitPhase; frontendReady: boolean } = {
  revision: 0,
  phase: "Open",
  frontendReady: false,
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

function mockSignal(name: string) {
  const event = mockEvent<null>(name);
  const base = { ...event(), emit: () => event.emit(null) };
  return Object.assign(() => base, base);
}
export const events = {
  menuCommand: mockEvent<MenuCommand>("menu-command"),
  pageRecordsChanged: mockSignal("page-records-changed"),
  indexChanged: mockEvent<IndexChanged>("index-changed"),
  indexStatusChanged: mockEvent<IndexStatus>("index-status-changed"),
  quitStatusChanged: mockEvent<QuitStatus>("quit-status-changed"),
  storageHealthChanged: mockEvent<Health>("storage-health-changed"),
} satisfies Events;
function quitStatus(): QuitStatus {
  return { revision: quit.revision, phase: structuredClone(quit.phase) };
}
function changeQuit(phase: QuitPhase) {
  quit.revision += 1;
  quit.phase = phase;
  void events.quitStatusChanged.emit(quitStatus());
}
function pendingDocs(): string[] {
  return [...storage.docs]
    .filter(([, doc]) => doc.pending)
    .map(([id]) => id)
    .sort();
}
function flushThenQuit() {
  const pending = pendingDocs();
  if (pending.length === 0 && !settingsMock().pending) {
    changeQuit("Confirmed");
    exited = true;
  } else
    changeQuit({
      Asking: {
        pending_docs: pending,
        pending_settings: settingsMock().pending,
      },
    });
}
function refreshHealth() {
  const pending = pendingDocs();
  const next: Health =
    pending.length === 0 && !settingsMock().pending
      ? "Healthy"
      : {
          Failing: {
            reason: storage.reason,
            pending_docs: pending,
            pending_settings: settingsMock().pending,
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
    const lines = fixtureBodies.get(docId);
    if (lines) {
      writeBody(
        doc.doc,
        lines.map((line, i) => ({ id: `${docId}-b${i}`, content: [line] })),
      );
    }
    storage.docs.set(docId, doc);
  }
  return doc;
}

const derived: {
  tree: { version: number; value: ReturnType<typeof projectTree> } | null;
  records: Map<string, PageRecord> | null;
  bodies: Map<string, Body>;
  searchable: Map<string, SearchablePage>;
  ordered: PageIndex | null;
  workspaceVersion: number;
  changed: { workspace: boolean; pages: Set<string> };
  timer: ReturnType<typeof setTimeout> | null;
} = {
  tree: null,
  records: null,
  bodies: new Map(),
  searchable: new Map(),
  ordered: null,
  workspaceVersion: 0,
  changed: { workspace: false, pages: new Set() },
  timer: null,
};

function workspaceDoc(): Y.Doc {
  return mockDoc(workspaceDocId).doc;
}

function records(): Map<string, PageRecord> {
  derived.records ??= readRecords(workspaceDoc());
  return derived.records;
}

function sourceTree() {
  if (derived.tree?.version !== derived.workspaceVersion)
    derived.tree = {
      version: derived.workspaceVersion,
      value: projectTree(records()),
    };
  return derived.tree.value;
}

function body(pageId: string): Body {
  let read = derived.bodies.get(pageId);
  if (!read) {
    read = readBody(mockDoc(pageId).doc);
    derived.bodies.set(pageId, read);
  }
  return read;
}

function label(pageId: string): string {
  const page = sourceTree().tree.pages.find((p) => p.id === pageId);
  if (!page) return "Deleted page";
  return page.title === "" ? "Untitled" : page.title;
}

function searchable(pageId: string, record: PageRecord): SearchablePage {
  let page = derived.searchable.get(pageId);
  if (!page) {
    const lines = storage.docs.has(pageId) ? null : fixtureBodies.get(pageId);
    page = {
      id: pageId,
      title: prepare(record.title),
      createdAt: record.createdAt,
      blocks: lines
        ? lines.map((line, i) => ({
            id: `${pageId}-b${i}`,
            text: prepare(line),
          }))
        : body(pageId).blocks.map((block) => ({
            id: block.id,
            text: prepare(renderSegments(block.segments, label)),
          })),
    };
    derived.searchable.set(pageId, page);
  }
  return page;
}

function markChanged(change: { workspace: boolean; pages: string[] }) {
  if (change.workspace) {
    derived.workspaceVersion += 1;
    derived.records = null;
    derived.searchable.clear();
    derived.ordered = null;
    derived.changed.workspace = true;
  }
  for (const page of change.pages) {
    derived.bodies.delete(page);
    derived.searchable.delete(page);
    derived.ordered = null;
    derived.changed.pages.add(page);
  }
  if (indexStatus !== "Ready" || derived.timer !== null) return;
  derived.timer = setTimeout(() => {
    derived.timer = null;
    const changed = {
      workspace: derived.changed.workspace,
      pages: [...derived.changed.pages].sort(),
    };
    derived.changed = { workspace: false, pages: new Set() };
    if (indexStatus === "Ready") void events.indexChanged.emit(changed);
  }, 25);
}

function scopeError(scope: SearchScope): QueryError | null {
  if (scope === "All") return null;
  const exclude = new Set(scope.Content.exclude);
  if ([...exclude].some((id) => !isPageId(id))) return "InvalidPageId";
  if (exclude.size > MAX_EXCLUDED_PAGES)
    return { InvalidQuery: { TooManyExcluded: { max: MAX_EXCLUDED_PAGES } } };
  return null;
}

function pageProblem(problem: NonNullable<Body["problem"]>): PageProblem {
  switch (problem.kind) {
    case "UnrecognizedRoot":
      return { UnrecognizedRoot: { root: problem.root } };
    default:
      return problem.kind;
  }
}

function issues(): PageIssue[] {
  const active = new Set(sourceTree().tree.pages.map((page) => page.id));
  return [...storage.docs.keys()]
    .filter((docId) => active.has(docId))
    .sort()
    .flatMap((pageId) => {
      const read = body(pageId);
      return read.problem
        ? [
            {
              page_id: pageId,
              problem: pageProblem(read.problem),
              indexed_blocks: read.blocks.length,
            },
          ]
        : [];
    });
}

function orderedPages(): PageIndex {
  if (!derived.ordered) {
    const all = records();
    derived.ordered = new PageIndex(
      sourceTree().tree.pages.flatMap((page) => {
        const record = all.get(page.id);
        return record ? [searchable(page.id, record)] : [];
      }),
    );
  }
  return derived.ordered;
}

function search(text: string, limit: number, scope: Scope): MockHit[] {
  return searchPages(orderedPages(), text, limit, scope);
}

function acceptRecordEdit(edit: () => void) {
  const stored = mockDoc(workspaceDocId);
  const updates: Uint8Array[] = [];
  const collect = (update: Uint8Array) => {
    updates.push(update);
  };
  stored.doc.on("update", collect);
  try {
    stored.doc.transact(edit);
  } finally {
    stored.doc.off("update", collect);
  }
  stored.updates.push(
    ...(updates.length === 0 ? [Uint8Array.of(0, 0)] : updates),
  );
  stored.pending = storage.failWrites;
  storage.seq += 1;
  refreshHealth();
  markChanged({ workspace: true, pages: [] });
  void events.pageRecordsChanged.emit();
}

const recordCommands = createRecordCommands({
  doc: workspaceDoc,
  records,
  tree: () => sourceTree().tree,
  accept: acceptRecordEdit,
  error: () =>
    storage.status !== "Ready"
      ? "Unavailable"
      : quit.phase === "Confirmed"
        ? "Quitting"
        : storage.recordError,
  hasBody: (id) => {
    const doc = storage.docs.get(id)?.doc;
    return doc !== undefined && Y.encodeStateAsUpdate(doc).length > 2;
  },
});
let recovery: ReturnType<typeof createRecoveryMock> | undefined;
function recoveryMock() {
  if (recovery) return recovery;
  recovery = createRecoveryMock({
    ready: () => setupReady,
    status: () => storage.status,
    setup: () => window.__aelomiMockSetup?.recovery,
    record: (command) => calls.push({ command, args: [] }),
    open: async () => {
      storage.status = "Ready";
      indexStatus = "Ready";
      await events.indexStatusChanged.emit("Ready");
      quit.frontendReady = true;
    },
  });
  return recovery;
}

export const commands = {
  closeWindow: async () => {
    calls.push({ command: "closeWindow", args: [] });
    control.quit.request();
    return { status: "ok", data: null };
  },
  settingsGet: async () => {
    await setupReady;
    if (failSettingsRead)
      return {
        status: "error",
        error: { Io: "synthetic settings read failure" },
      };
    return settingsMock().commands.settingsGet();
  },
  settingsUpdate: (patch) => settingsMock().commands.settingsUpdate(patch),
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
  showRecoveryFolder: () => recoveryMock().commands.showRecoveryFolder(),
  startEmptyWorkspace: () => recoveryMock().commands.startEmptyWorkspace(),
  storageHealth: async () => {
    calls.push({ command: "storageHealth", args: [] });
    return storage.health;
  },
  indexStatus: async () => {
    calls.push({ command: "indexStatus", args: [] });
    if (storage.status !== "Ready")
      return { Failed: { reason: "the data folder is not open" } };
    const snapshot = recoveryMock().snapshot();
    if (snapshot) return snapshot;
    return structuredClone(indexStatus);
  },
  pageTree: async () => {
    calls.push({ command: "pageTree", args: [] });
    await setupReady;
    await recoveryMock().waitForTree();
    if (failTreeRead)
      return { status: "error", error: { Io: "synthetic tree read failure" } };
    if (storage.status !== "Ready")
      return { status: "error", error: "Unavailable" };
    const problem = pagesRootProblem(workspaceDoc());
    if (problem !== null)
      return {
        status: "error",
        error: { Failed: { reason: `workspace doc: ${problem}` } },
      };
    const tree: PageTree = structuredClone(sourceTree().tree);
    return { status: "ok", data: tree };
  },
  search: async (query, limit, scope) => {
    calls.push({
      command: "search",
      args: [query, limit, structuredClone(scope)],
    });
    await setupReady;
    const error = queryFailure() ?? scopeError(scope);
    if (error) return { status: "error", error };
    if (indexStatus === "Rebuilding") return { status: "ok", data: "Indexing" };
    return {
      status: "ok",
      data: {
        Ready: {
          hits: search(
            query,
            limit,
            scope === "All"
              ? { kind: "all" }
              : { kind: "content", exclude: new Set(scope.Content.exclude) },
          ),
          issues: issues(),
        },
      },
    };
  },
  backlinks: async (pageId) => {
    calls.push({ command: "backlinks", args: [pageId] });
    if (!isPageId(pageId)) return { status: "error", error: "InvalidPageId" };
    const error = queryFailure();
    if (error) return { status: "error", error };
    return {
      status: "ok",
      data:
        indexStatus === "Rebuilding"
          ? "Indexing"
          : { Ready: { backlinks: [], issues: issues() } },
    };
  },
  quitStatus: async () => {
    calls.push({ command: "quitStatus", args: [] });
    return quitStatus();
  },
  keepOpen: async (revision) => {
    calls.push({ command: "keepOpen", args: [revision] });
    if (typeof quit.phase === "object" && quit.revision === revision)
      changeQuit("Open");
    return quitStatus();
  },
  quitAnyway: async () => {
    calls.push({ command: "quitAnyway", args: [] });
    if (quit.phase === "Confirmed") return;
    changeQuit("Confirmed");
    exited = true;
  },
  frontendReady: async () => {
    calls.push({ command: "frontendReady", args: [] });
    await setupReady;
    await registration;
    if (storage.status === "Ready") quit.frontendReady = true;
  },
  completeQuit: async (revision) => {
    calls.push({ command: "completeQuit", args: [revision] });
    if (quit.phase === "Draining" && quit.revision === revision)
      flushThenQuit();
    return quitStatus();
  },
  quitDrainFailed: async (revision, docIds, pendingSettings) => {
    calls.push({
      command: "quitDrainFailed",
      args: [revision, [...docIds], pendingSettings],
    });
    const invalid = docIds.find((id) => !/^[0-9a-z-]{1,64}$/.test(id));
    if (invalid !== undefined)
      return { status: "error", error: "InvalidDocId" };
    if (quit.phase === "Draining" && quit.revision === revision)
      changeQuit({
        Asking: {
          pending_docs: [...new Set([...docIds, ...pendingDocs()])].sort(),
          pending_settings: settingsMock().pending || pendingSettings,
        },
      });
    return { status: "ok", data: quitStatus() };
  },
  docLoad: async (docId) => {
    calls.push({ command: "docLoad", args: [docId] });
    await setupReady;
    const error = refusal(docId);
    if (error) return { status: "error", error };
    const { doc } = mockDoc(docId);
    return { status: "ok", data: toBase64(Y.encodeStateAsUpdate(doc)) };
  },
  docAppend: async (docId, update) => {
    calls.push({ command: "docAppend", args: [docId, update] });
    const error = refusal(docId);
    if (error) return { status: "error", error };
    if (storage.appendError !== null)
      return { status: "error", error: storage.appendError };
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
    markChanged(
      docId === workspaceDocId
        ? { workspace: true, pages: [] }
        : { workspace: false, pages: [docId] },
    );
    return { status: "ok", data: storage.seq };
  },
  pageCreate: async (parentId, pageId = null) => {
    calls.push({ command: "pageCreate", args: [parentId, pageId] });
    await setupReady;
    return recordCommands.pageCreate(parentId, pageId);
  },
  pageRename: async (pageId, title) => {
    calls.push({ command: "pageRename", args: [pageId, title] });
    await setupReady;
    return recordCommands.pageRename(pageId, title);
  },
  pageTrash: async (pageId) => {
    calls.push({ command: "pageTrash", args: [pageId] });
    await setupReady;
    return recordCommands.pageTrash(pageId);
  },
  devProbeLog: async (line) => {
    calls.push({ command: "devProbeLog", args: [line] });
  },
  openUrl: async (url) => {
    calls.push({ command: "openUrl", args: [url] });
    return { status: "ok", data: null };
  },
  storageFlush: async () => {
    calls.push({ command: "storageFlush", args: [] });
    if (storage.status !== "Ready")
      return { status: "error", error: "Unavailable" };
    return {
      status: "ok",
      data: [...storage.docs.values()].every((doc) => !doc.pending),
    };
  },
} satisfies Commands;

function seedBodies(bodies: Record<string, SeedBlock[]>) {
  for (const [pageId, blocks] of Object.entries(bodies)) {
    writeBody(mockDoc(pageId).doc, blocks);
    markChanged({ workspace: false, pages: [pageId] });
  }
}

function seedRawBodies(bodies: Record<string, string>) {
  for (const [pageId, update] of Object.entries(bodies)) {
    Y.applyUpdate(mockDoc(pageId).doc, fromBase64(update));
    markChanged({ workspace: false, pages: [pageId] });
  }
}

async function loadSearchFixture() {
  const { default: jsonl } = await import(
    "../../fixtures/generated/search-10k-pages.jsonl?raw"
  );
  const pages: FixturePage[] = jsonl
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => {
      const page = JSON.parse(line) as {
        id: string;
        title: string;
        parentId: string | null;
        trash: { batchId: string; trashedAt: number } | null;
        body: string;
      };
      fixtureBodies.set(page.id, page.body.split("\n"));
      return { id: page.id, title: page.title, lines: [] };
    });
  const parents = new Map<string, string | null>();
  const trash = new Map<
    string,
    { batchId: string; trashedAt: number } | null
  >();
  for (const line of jsonl.split("\n")) {
    if (line.trim() === "") continue;
    const page = JSON.parse(line) as {
      id: string;
      parentId: string | null;
      trash: { batchId: string; trashedAt: number } | null;
    };
    parents.set(page.id, page.parentId);
    trash.set(page.id, page.trash);
  }
  writeRecords(
    workspaceDoc(),
    pages.map((page, line) => {
      const trashed = trash.get(page.id) ?? null;
      return {
        id: page.id,
        title: page.title,
        parentId: parents.get(page.id) ?? null,
        order: line,
        createdAt: 1_700_000_000_000 + line * 1000,
        updatedAt: 1_700_000_000_000 + line * 1000,
        trash: trashed && {
          originalParentId: parents.get(page.id) ?? null,
          originalOrder: line,
          batchId: trashed.batchId,
          trashedAt: trashed.trashedAt,
        },
      };
    }),
  );
  markChanged({ workspace: true, pages: [] });
  orderedPages();
}

async function applySetup(setup: MockSetup | undefined) {
  if (!setup) return;
  if (setup.holdRegistration)
    registration = new Promise<void>((resolve) => {
      releaseRegistration = resolve;
    });
  if (setup.settings) settingsMock().seed(setup.settings);
  if (setup.appearance) appearance = setup.appearance;
  if (setup.storageStatus)
    storage.status = structuredClone(setup.storageStatus);
  failSettingsRead = setup.failSettingsRead ?? false;
  if (setup.failWrites) {
    storage.failWrites = true;
    storage.reason = setup.failWrites;
  }
  if (setup.indexStatus) indexStatus = structuredClone(setup.indexStatus);
  if (setup.pages) {
    writeRecords(workspaceDoc(), setup.pages);
    markChanged({ workspace: true, pages: [] });
  }
  if (setup.bodies) seedBodies(setup.bodies);
  if (setup.rawBodies) seedRawBodies(setup.rawBodies);
  if (setup.rawWorkspace) {
    Y.applyUpdate(workspaceDoc(), fromBase64(setup.rawWorkspace));
    markChanged({ workspace: true, pages: [] });
  }
  if (setup.searchFixture) await loadSearchFixture();
}

let setupReady: Promise<void> = Promise.resolve();

const reloadKey = "aelomi-mock-reload";
type KeptState = { indexStatus: IndexStatus; docs: Record<string, string> };

function restoreKept(): boolean {
  const kept = window.sessionStorage?.getItem(reloadKey);
  if (!kept) return false;
  window.sessionStorage.removeItem(reloadKey);
  const state = JSON.parse(kept) as KeptState;
  indexStatus = state.indexStatus;
  for (const [docId, update] of Object.entries(state.docs))
    Y.applyUpdate(mockDoc(docId).doc, fromBase64(update));
  markChanged({ workspace: true, pages: Object.keys(state.docs) });
  return true;
}

function resetState() {
  failTreeRead = false;
  failSettingsRead = false;
  calls.length = 0;
  errors.length = 0;
  settingsMock().reset();
  releaseRegistration();
  registration = Promise.resolve();
  appearance = null;
  storage.status = "Ready";
  storage.appendError = null;
  storage.recordError = null;
  storage.failWrites = false;
  storage.docs.clear();
  storage.seq = 0;
  storage.health = "Healthy";
  storage.reason = { Io: "mock write failure" };
  exited = false;
  indexStatus = "Ready";
  quit.revision = 0;
  quit.phase = "Open";
  quit.frontendReady = false;
  fixtureBodies.clear();
  if (derived.timer !== null) clearTimeout(derived.timer);
  derived.tree = null;
  derived.records = null;
  derived.bodies.clear();
  derived.searchable.clear();
  derived.ordered = null;
  derived.workspaceVersion = 0;
  derived.changed = { workspace: false, pages: new Set() };
  derived.timer = null;
}

function pageWrite(update: () => void) {
  update();
  markChanged({ workspace: true, pages: [] });
}
const control = {
  recovery: { releaseTree: () => recoveryMock().control.releaseTree() },
  menu: {
    activate(command: MenuCommand["command"]) {
      return events.menuCommand.emit({ command });
    },
  },
  get calls(): readonly MockCall[] {
    return calls;
  },
  /** Uncaught errors and unhandled rejections since the page loaded. */
  get errors(): readonly string[] {
    return errors;
  },
  reset() {
    resetState();
  },
  quit: {
    releaseRegistration: () => releaseRegistration(),
    get exited() {
      return exited;
    },
    get status(): QuitStatus {
      return quitStatus();
    },
    /** A native quit: ⌘Q, closing the window, or AppleScript `quit`. */
    request() {
      if (exited) return;
      if (quit.phase === "Confirmed") {
        exited = true;
        return;
      }
      if (!quit.frontendReady) {
        const pending = pendingDocs();
        if (pending.length === 0 && !settingsMock().pending) exited = true;
        else
          changeQuit({
            Asking: {
              pending_docs: pending,
              pending_settings: settingsMock().pending,
            },
          });
        return;
      }
      if (quit.phase !== "Draining") changeQuit("Draining");
    },
    /** The shell's drain timeout: flushes the Store and asks. */
    timeOut() {
      if (quit.phase !== "Draining") return;
      changeQuit({
        Asking: {
          pending_docs: pendingDocs(),
          pending_settings: settingsMock().pending,
        },
      });
    },
  },
  index: {
    setStatus(status: IndexStatus) {
      indexStatus = structuredClone(status);
      void events.indexStatusChanged.emit(structuredClone(status));
    },
  },
  workspace: {
    async rename(pageId: string, title: string) {
      const { commands } = await import("./index");
      return commands.pageRename(pageId, title);
    },
    failReads(on: boolean, notify = true) {
      failTreeRead = on;
      if (notify) void events.pageRecordsChanged.emit();
    },
    seed(pages: SeedPage[]) {
      pageWrite(() => writeRecords(workspaceDoc(), pages));
    },
    trash(pageId: string, batchId: string) {
      pageWrite(() => trashPage(workspaceDoc(), pageId, batchId, Date.now()));
    },
    tree(): PageTree {
      return structuredClone(sourceTree().tree);
    },
  },
  bodies: {
    seed(pageId: string, blocks: SeedBlock[]) {
      seedBodies({ [pageId]: blocks });
    },
    /** Applies a base64 Yjs update to the page's doc as stored bytes. */
    raw(pageId: string, update: string) {
      seedRawBodies({ [pageId]: update });
    },
    /** The body's blocks as the Rust reader would index them. */
    blocks(pageId: string) {
      return readBody(mockDoc(pageId).doc);
    },
    /** Every block's rendered text, one line per block. */
    text(pageId: string): string {
      return readBody(mockDoc(pageId).doc)
        .blocks.map((block) => renderSegments(block.segments, label))
        .join("\n");
    },
    /** The doc's full state as base64. */
    state(pageId: string): string {
      return toBase64(Y.encodeStateAsUpdate(mockDoc(pageId).doc));
    },
  },
  appearance: {
    current: () =>
      appearance ??
      (window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"),
    set(value: "light" | "dark") {
      appearance = value;
      window.dispatchEvent(new Event("aelomi-test-appearance"));
    },
  },
  get settings() {
    return {
      ...settingsMock().control,
      failReads(on: boolean) {
        failSettingsRead = on;
      },
    };
  },
  storage: {
    refuseRecords(error: import("./bindings").PageError | null) {
      storage.recordError = error;
    },
    refuseAppends(error: StorageError | null) {
      storage.appendError = error;
    },
    /** While on, appends stay pending and health reports the failure. */
    failWrites(on: boolean, reason: FailReason = { Io: "mock write failure" }) {
      storage.failWrites = on;
      storage.reason = reason;
      if (!on) {
        for (const doc of storage.docs.values()) doc.pending = false;
        settingsMock().recover();
      }
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
    /**
     * Keeps every doc for the next load of this tab, as the store keeps
     * accepted updates across a relaunch. Acceptance tests cannot relaunch,
     * so they reload instead.
     */
    keepForReload() {
      const state: KeptState = {
        indexStatus,
        docs: Object.fromEntries(
          [...storage.docs].map(([id, { doc }]) => [
            id,
            toBase64(Y.encodeStateAsUpdate(doc)),
          ]),
        ),
      };
      window.sessionStorage.setItem(reloadKey, JSON.stringify(state));
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
  window.addEventListener?.("error", (event) => {
    errors.push(String(event.error ?? event.message));
  });
  window.addEventListener?.("unhandledrejection", (event) => {
    errors.push(String(event.reason));
  });
  setupReady = restoreKept()
    ? Promise.resolve()
    : applySetup(window.__aelomiMockSetup);
  return { commands, events };
}
