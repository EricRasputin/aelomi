import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { installMock, commands as mockTransport } from "./mock";
import { drainCommands, setCommandAdmission } from "./pending-commands";
import { withRecordCommands } from "./record-commands";

const transport: typeof import("./bindings").commands = mockTransport;

function deferred<T>() {
  let resolve = (_value: T) => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  setCommandAdmission("open");
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
  const drain = drainCommands().then((docs) => {
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
  const drain = drainCommands();
  rename.resolve({ status: "error", error: { Io: "read failed" } });
  expect(await command).toEqual({
    status: "error",
    error: { Io: "read failed" },
  });
  expect(await drain).toEqual(["workspace"]);
});

test("back-to-back renames retain submission order even when the native scheduler runs newest first", async () => {
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Before" }]);
  const nativeRename = transport.pageRename;
  const jobs: (() => Promise<void>)[] = [];
  const submitted: string[] = [];
  vi.spyOn(transport, "pageRename").mockImplementation((id, title) => {
    submitted.push(title);
    return new Promise((resolve) => {
      jobs.push(async () => resolve(await nativeRename(id, title)));
    });
  });
  const commands = withRecordCommands(transport);
  const first = commands.pageRename("a", "a");
  const last = commands.pageRename("a", "ab");
  let drained = false;
  const drain = drainCommands().then((docs) => {
    drained = true;
    return docs;
  });
  for (let i = 0; i < 10 && !drained; i++) {
    await jobs.pop()?.();
    await new Promise((done) => setTimeout(done, 0));
  }
  expect(await Promise.all([first, last])).toEqual([
    { status: "ok", data: null },
    { status: "ok", data: null },
  ]);
  expect(await drain).toEqual([]);
  expect(submitted).toEqual(["a", "ab"]);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((page) => page.title),
  ).toEqual(["ab"]);
});

test("queued consecutive renames coalesce but never cross another record command", async () => {
  const held = deferred<Awaited<ReturnType<typeof transport.pageCreate>>>();
  vi.spyOn(transport, "pageCreate").mockReturnValueOnce(held.promise);
  window.__aelomiMock?.workspace.seed([
    { id: "a", title: "Before" },
    { id: "b", title: "B" },
  ]);
  const rename = vi.spyOn(transport, "pageRename");
  const commands = withRecordCommands(transport);
  const pending = [
    commands.pageCreate(null),
    commands.pageRename("a", "a"),
    commands.pageRename("a", "ab"),
    commands.pageTrash("b"),
    commands.pageRename("a", "abc"),
  ];
  const drain = drainCommands();
  try {
    expect(rename.mock.calls).toEqual([]);
  } finally {
    held.resolve({ status: "ok", data: "new-page" });
    await Promise.all(pending);
  }
  expect(await drain).toEqual([]);
  expect(rename.mock.calls).toEqual([
    ["a", "ab"],
    ["a", "abc"],
  ]);
  expect(
    window.__aelomiMock?.workspace
      .tree()
      .pages.map((page) => [page.id, page.title]),
  ).toEqual([["a", "abc"]]);
});

test.each(["Quitting", "NotActive", "InvalidPageId", "Cycle"] as const)(
  "the drain leaves %s refusals on the command reply",
  async (error) => {
    const rename = deferred<Awaited<ReturnType<typeof transport.pageRename>>>();
    vi.spyOn(transport, "pageRename").mockReturnValueOnce(rename.promise);
    const command = withRecordCommands(transport).pageRename("a", "Title");
    const drain = drainCommands();
    rename.resolve({ status: "error", error });
    expect(await command).toEqual({ status: "error", error });
    expect(await drain).toEqual([]);
  },
);
