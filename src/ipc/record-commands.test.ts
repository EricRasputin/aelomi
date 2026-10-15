import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { installMock, commands as mockTransport } from "./mock";
import { drainRecordCommands, withRecordCommands } from "./record-commands";

const transport: typeof import("./bindings").commands = mockTransport;

function deferred<T>() {
  let resolve = (_value: T) => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
});
afterEach(() => {
  vi.restoreAllMocks();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("the IPC boundary drains create rename and trash promises before reporting completion", async () => {
  const create = deferred<Awaited<ReturnType<typeof transport.pageCreate>>>();
  const rename = deferred<Awaited<ReturnType<typeof transport.pageRename>>>();
  const trash = deferred<Awaited<ReturnType<typeof transport.pageTrash>>>();
  vi.spyOn(transport, "pageCreate").mockReturnValueOnce(create.promise);
  vi.spyOn(transport, "pageRename").mockReturnValueOnce(rename.promise);
  vi.spyOn(transport, "pageTrash").mockReturnValueOnce(trash.promise);
  const commands = withRecordCommands(transport);
  const created = commands.pageCreate(null);
  const renamed = commands.pageRename("page-a", "Title");
  const trashed = commands.pageTrash("page-b");
  let drained = false;
  const drain = drainRecordCommands().then((docs) => {
    drained = true;
    return docs;
  });
  try {
    create.resolve({ status: "ok", data: "new-page" });
    rename.resolve({ status: "ok", data: null });
    await new Promise((done) => setTimeout(done, 0));
    expect(drained).toBe(false);
  } finally {
    trash.resolve({ status: "ok", data: "trash-batch" });
  }
  expect(await Promise.all([created, renamed, trashed])).toEqual([
    { status: "ok", data: "new-page" },
    { status: "ok", data: null },
    { status: "ok", data: "trash-batch" },
  ]);
  expect(await drain).toEqual([]);
});

test("a refused in-flight record command reports the workspace to the quit drain", async () => {
  const rename = deferred<Awaited<ReturnType<typeof transport.pageRename>>>();
  vi.spyOn(transport, "pageRename").mockReturnValueOnce(rename.promise);
  const command = withRecordCommands(transport).pageRename("page-a", "Title");
  const drain = drainRecordCommands();
  rename.resolve({ status: "error", error: { Io: "read failed" } });
  expect(await command).toEqual({
    status: "error",
    error: { Io: "read failed" },
  });
  expect(await drain).toEqual(["workspace"]);
});
