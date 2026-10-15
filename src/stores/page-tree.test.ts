import { afterEach, beforeEach, expect, test, vi } from "vitest";
import * as Y from "yjs";
import { commands } from "../ipc";
import { fromBase64 } from "../ipc/base64";
import { installMock } from "../ipc/mock";
import { connectIndexEvents } from "./index-status";
import { usePageTreeStore } from "./page-tree";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: mock.commands,
    events: mock.events,
  };
});

const connections: ReturnType<typeof connectIndexEvents>[] = [];
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
});

async function create(parent: string | null = null) {
  const reply = await commands.pageCreate(parent);
  if (reply.status === "error") throw new Error(JSON.stringify(reply.error));
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
  const connection = connectIndexEvents();
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
    },
  });
  const reply = await commands.pageTrash(parent);
  if (reply.status === "error") throw new Error(JSON.stringify(reply.error));
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
  await commands.pageTrash(id);
  expect(await commands.pageRename(id, "name")).toEqual({
    status: "error",
    error: "NotActive",
  });
  expect(await commands.pageTrash(id)).toEqual({
    status: "error",
    error: "NotActive",
  });
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
  const connection = connectIndexEvents();
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
  await vi.waitFor(() => expect(shown().pages.map((p) => p.id)).toEqual([id]));
  finish({ status: "ok", data: { pages: [], favorites: [], trash: [] } });
  await pending;
  expect(shown().pages.map((p) => p.id)).toEqual([id]);
  vi.restoreAllMocks();
});
