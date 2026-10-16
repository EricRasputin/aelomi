import { afterEach, beforeEach, expect, test, vi } from "vitest";
import * as Y from "yjs";
import { commands, events } from "../ipc";
import { fromBase64, toBase64 } from "../ipc/base64";
import { installMock } from "../ipc/mock";
import { withRecordCommands } from "../ipc/record-commands";
import { connectPageTreeEvents, usePageTreeStore } from "./page-tree";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: mock.commands,
    events: mock.events,
  };
});

const connections: ReturnType<typeof connectPageTreeEvents>[] = [];
beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  usePageTreeStore.setState(usePageTreeStore.getInitialState(), true);
});
afterEach(() => {
  for (const connection of connections.splice(0)) connection.dispose();
  window.__aelomiMock?.reset();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

test("a failed refresh retains the last tree and retries with storage backoff", async () => {
  vi.useFakeTimers();
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Last known" }]);
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await connection.ready;
  await vi.runOnlyPendingTimersAsync();
  const read = vi
    .spyOn(commands, "pageTree")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read refused" } })
    .mockResolvedValueOnce({
      status: "error",
      error: { Io: "read refused again" },
    });
  await usePageTreeStore.getState().refresh();
  expect(usePageTreeStore.getState().source).toMatchObject({
    kind: "ready",
    tree: { pages: [{ id: "a", title: "Last known" }] },
    error: "read refused",
  });
  await vi.advanceTimersByTimeAsync(99);
  expect(read.mock.calls).toHaveLength(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(usePageTreeStore.getState().source).toMatchObject({
    kind: "ready",
    error: "read refused again",
  });
  await vi.advanceTimersByTimeAsync(199);
  expect(read.mock.calls).toHaveLength(2);
  await vi.advanceTimersByTimeAsync(1);
  expect(usePageTreeStore.getState().source).toMatchObject({
    kind: "ready",
    tree: { pages: [{ id: "a", title: "Last known" }] },
    error: null,
  });
});

async function create(parent: string | null = null) {
  const reply = await commands.pageCreate(parent);
  if (reply.status !== "ok") throw new Error(JSON.stringify(reply));
  return reply.data;
}

function shown() {
  const source = usePageTreeStore.getState().source;
  if (source.kind !== "ready") throw new Error(source.kind);
  return source.tree;
}

test("create rename and subtree trash refresh the source tree while the index rebuilds and writes fail", async () => {
  window.__aelomiMock?.index.setStatus("Rebuilding");
  window.__aelomiMock?.storage.failWrites(true);
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await connection.ready;
  await usePageTreeStore.getState().refresh();
  const parent = await create();
  const child = await create(parent);
  await commands.pageRename(parent, "Projects");
  await vi.waitFor(() =>
    expect(shown().pages.map((p) => [p.id, p.title, p.parent_id])).toEqual([
      [parent, "Projects", null],
      [child, "", parent],
    ]),
  );
  expect(await commands.storageHealth()).toEqual({
    Failing: {
      reason: { Io: "mock write failure" },
      pending_docs: ["workspace"],
      pending_settings: false,
    },
  });
  const reply = await commands.pageTrash(parent);
  if (reply.status !== "ok") throw new Error(JSON.stringify(reply));
  await vi.waitFor(() => expect(shown().pages).toEqual([]));
  expect(shown().trash).toEqual([
    {
      id: parent,
      title: "Projects",
      batch_id: reply.data,
      trashed_at: expect.any(Number),
    },
  ]);
  const loaded = await commands.docLoad("workspace");
  if (loaded.status === "error") throw new Error(JSON.stringify(loaded.error));
  const doc = new Y.Doc();
  Y.applyUpdate(doc, fromBase64(loaded.data));
  const pages = doc.getMap<Y.Map<unknown>>("pages");
  const placement = pages.get(child)?.get("placement");
  expect(placement).toEqual({ parentId: parent, order: 2 ** 20 });
  expect(pages.get(child)?.get("trash")).toEqual({
    originalParentId: parent,
    originalOrder: 2 ** 20,
    batchId: reply.data,
    trashedAt: shown().trash[0]?.trashed_at,
  });
  doc.destroy();
});

test("page commands reject invalid inactive and unavailable targets without adding a page", async () => {
  const id = await create();
  expect(await commands.pageCreate("workspace")).toEqual({
    status: "error",
    error: "InvalidPageId",
  });
  expect(await commands.pageCreate("missing")).toEqual({
    status: "error",
    error: "NotActive",
  });
  expect(await commands.pageRename("missing", "name")).toEqual({
    status: "error",
    error: "NotActive",
  });
  const trash = await commands.pageTrash(id);
  expect(await commands.pageRename(id, "name")).toEqual({
    status: "error",
    error: "NotActive",
  });
  expect(await commands.pageTrash(id)).toEqual(trash);
  expect(window.__aelomiMock?.workspace.tree().trash.map((p) => p.id)).toEqual([
    id,
  ]);
  window.__aelomiMock?.storage.setStatus("Locked");
  expect(await commands.pageCreate(null)).toEqual({
    status: "error",
    error: "Unavailable",
  });
  expect(await commands.pageRename(id, "name")).toEqual({
    status: "error",
    error: "Unavailable",
  });
  expect(await commands.pageTrash(id)).toEqual({
    status: "error",
    error: "Unavailable",
  });
});

test("a source event overtakes a stale tree snapshot", async () => {
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await connection.ready;
  let finish = (_reply: Awaited<ReturnType<typeof commands.pageTree>>) => {};
  const stale = new Promise<Awaited<ReturnType<typeof commands.pageTree>>>(
    (resolve) => {
      finish = resolve;
    },
  );
  const original = commands.pageTree;
  vi.spyOn(commands, "pageTree")
    .mockReturnValueOnce(stale)
    .mockImplementation(original);
  const pending = usePageTreeStore.getState().refresh();
  const id = await create();
  finish({ status: "ok", data: { pages: [], favorites: [], trash: [] } });
  await pending;
  expect(shown().pages.map((p) => p.id)).toEqual([id]);
  vi.restoreAllMocks();
});

test("mock record edits store their transaction delta without pending foreign structs", async () => {
  const id = await create();
  const loaded = await commands.docLoad("workspace");
  if (loaded.status === "error") throw new Error(JSON.stringify(loaded.error));
  const foreign = new Y.Doc();
  foreign.clientID = 9;
  Y.applyUpdate(foreign, fromBase64(loaded.data));
  let delta: Uint8Array = new Uint8Array();
  foreign.on("update", (update: Uint8Array) => {
    delta = update;
  });
  foreign.getText("future").insert(0, "dependency ");
  const dependency = delta;
  foreign.getText("future").insert(11, "arrives");
  await commands.docAppend("workspace", toBase64(delta));
  await commands.pageRename(id, "After");
  const accepted = window.__aelomiMock?.storage.updates("workspace").at(-1);
  if (!accepted) throw new Error("missing record update");
  expect(
    Y.decodeUpdate(accepted)
      .structs.map((struct) => struct.id.client)
      .includes(9),
  ).toBe(false);
  await commands.docAppend("workspace", toBase64(dependency));
  const state = await commands.docLoad("workspace");
  if (state.status === "error") throw new Error(JSON.stringify(state.error));
  const doc = new Y.Doc();
  Y.applyUpdate(doc, fromBase64(state.data));
  expect(doc.getText("future").toString()).toBe("dependency arrives");
  expect(doc.getMap<Y.Map<unknown>>("pages").get(id)?.get("title")).toBe(
    "After",
  );
  foreign.destroy();
  doc.destroy();
});

test("a burst keeps one tree request in flight and installs its reply before the next finishes", async () => {
  type Reply = Awaited<ReturnType<typeof commands.pageTree>>;
  let first = (_reply: Reply) => {};
  let last = (_reply: Reply) => {};
  const initial = new Promise<Reply>((done) => {
    first = done;
  });
  const followup = new Promise<Reply>((done) => {
    last = done;
  });
  const spy = vi
    .spyOn(commands, "pageTree")
    .mockReturnValueOnce(initial)
    .mockReturnValueOnce(followup);
  const pending = usePageTreeStore.getState().refresh();
  for (let i = 0; i < 20; i++) void usePageTreeStore.getState().refresh();
  const reply = (title: string): Reply => ({
    status: "ok",
    data: {
      pages: [
        { id: "a", title, parent_id: null, created_at: 1, updated_at: 1 },
      ],
      favorites: [],
      trash: [],
    },
  });
  try {
    expect(spy.mock.calls).toEqual([[]]);
    first(reply("first"));
    await vi.waitFor(() =>
      expect(shown().pages.map((page) => page.title)).toEqual(["first"]),
    );
    expect(spy.mock.calls).toEqual([[], []]);
  } finally {
    first(reply("first"));
    last(reply("last"));
    await pending;
  }
  expect(shown().pages.map((page) => page.title)).toEqual(["last"]);
});

test("tree subscriptions finish before the initial snapshot is requested", async () => {
  let release = () => {};
  const registration = new Promise<void>((done) => {
    release = done;
  });
  const original = events.pageRecordsChanged.listen;
  vi.spyOn(events.pageRecordsChanged, "listen").mockImplementationOnce(
    async (...args) => {
      await registration;
      return original(...args);
    },
  );
  const read = vi.spyOn(commands, "pageTree");
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await new Promise((done) => setTimeout(done, 0));
  expect(read.mock.calls).toEqual([]);
  release();
  await connection.ready;
  const id = await create();
  await vi.waitFor(() =>
    expect(shown().pages.map((page) => page.id)).toEqual([id]),
  );
});

test("an accepted command refreshes the source tree even when its native event is lost", async () => {
  window.__aelomiMock?.index.setStatus("Rebuilding");
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await connection.ready;
  vi.spyOn(events.pageRecordsChanged, "emit").mockResolvedValueOnce(undefined);
  const reply = await withRecordCommands(
    (await import("../ipc/mock")).commands,
  ).pageCreate(null);
  if (reply.status !== "ok") throw new Error(JSON.stringify(reply));
  await vi.waitFor(() =>
    expect(shown().pages.map((page) => page.id)).toEqual([reply.data]),
  );
});

test("mock end append follows projected orphan roots", async () => {
  window.__aelomiMock?.workspace.seed([
    { id: "orphan", title: "Orphan", parentId: "missing", order: 10 * 2 ** 20 },
    { id: "root", title: "Root", order: 2 ** 20 },
  ]);
  const id = await create();
  expect(id).toMatch(/^[0-9a-f]{32}$/);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((page) => page.id),
  ).toEqual(["root", "orphan", id]);
});

test("a repeated mock rename preserves its accepted delta and timestamp", async () => {
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "A", updatedAt: 42 }]);
  vi.spyOn(Date, "now").mockReturnValue(42);
  expect(await commands.pageRename("a", "Renamed")).toEqual({
    status: "ok",
    data: null,
  });
  const accepted = window.__aelomiMock?.storage.updates("workspace");
  expect(accepted).toHaveLength(1);
  vi.spyOn(Date, "now").mockReturnValue(100);
  expect(await commands.pageRename("a", "Renamed")).toEqual({
    status: "ok",
    data: null,
  });
  expect(window.__aelomiMock?.storage.updates("workspace")).toEqual(accepted);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((page) => page.title),
  ).toEqual(["Renamed"]);
});

test("a failed tree listener still reads the initial page and retries the subscription", async () => {
  window.__aelomiMock?.workspace.seed([
    { id: "getting-started", title: "Getting started" },
  ]);
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  const listen = vi
    .spyOn(events.pageRecordsChanged, "listen")
    .mockRejectedValueOnce(new Error("registration refused"));
  const connection = connectPageTreeEvents();
  connections.push(connection);
  await expect(connection.ready).resolves.toBeUndefined();
  expect(shown().pages.map((page) => [page.id, page.title])).toEqual([
    ["getting-started", "Getting started"],
  ]);
  await vi.waitFor(() => expect(listen.mock.calls.length).toBe(2));
  await commands.pageRename("getting-started", "After subscription recovery");
  await vi.waitFor(() =>
    expect(shown().pages.map((page) => page.title)).toEqual([
      "After subscription recovery",
    ]),
  );
  expect(error.mock.calls[0]?.[0]).toBe(
    "Page tree event connection failed; retrying",
  );
});

test("mock create replays require the original active placement and empty title and reject orphaned bodies", async () => {
  const { commands: transport } = await import("../ipc/mock");
  expect(await transport.pageCreate(null, "same")).toEqual({
    status: "ok",
    data: "same",
  });
  expect(await transport.pageCreate(null, "same")).toEqual({
    status: "ok",
    data: "same",
  });
  expect(await transport.pageCreate("missing", "same")).toEqual({
    status: "error",
    error: "AlreadyExists",
  });
  await transport.pageRename("same", "Changed");
  expect(await transport.pageCreate(null, "same")).toEqual({
    status: "error",
    error: "AlreadyExists",
  });
  await transport.pageTrash("same");
  expect(await transport.pageCreate(null, "same")).toEqual({
    status: "error",
    error: "AlreadyExists",
  });
  window.__aelomiMock?.bodies.seed("orphan", [
    { id: "old", content: ["Old body"] },
  ]);
  expect(await transport.pageCreate(null, "orphan")).toEqual({
    status: "error",
    error: "AlreadyExists",
  });
  expect(window.__aelomiMock?.workspace.tree().pages).toEqual([]);
  expect(await transport.pageCreate(null, "new")).toEqual({
    status: "ok",
    data: "new",
  });
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "new",
  ]);
});

test("mock create replay refuses a record hidden under a trashed ancestor", async () => {
  const { commands: transport } = await import("../ipc/mock");
  window.__aelomiMock?.workspace.seed([
    {
      id: "parent",
      title: "Parent",
      trash: {
        originalParentId: null,
        originalOrder: 0,
        batchId: "batch",
        trashedAt: 1,
      },
    },
    { id: "child", title: "", parentId: "parent" },
  ]);
  expect(await transport.pageCreate("parent", "child")).toEqual({
    status: "error",
    error: "AlreadyExists",
  });
  expect(window.__aelomiMock?.workspace.tree().pages).toEqual([]);
  expect(window.__aelomiMock?.workspace.tree().trash.map((p) => p.id)).toEqual([
    "parent",
  ]);
});
