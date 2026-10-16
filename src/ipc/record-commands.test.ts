import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { installMock, commands as mockTransport } from "./mock";
import {
  commandFailures,
  drainCommands,
  setCommandAdmission,
} from "./pending-commands";
import { newPageId, withRecordCommands } from "./record-commands";

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
afterEach(async () => {
  vi.spyOn(transport, "pageRename").mockResolvedValue({
    status: "ok",
    data: null,
  });
  await drainCommands();
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
  await drain;
  expect(commandFailures()).toEqual({ docs: [], settings: false });
});

test("a refused in-flight record command reports the workspace to the quit drain", async () => {
  const rename = deferred<Awaited<ReturnType<typeof transport.pageRename>>>();
  vi.spyOn(transport, "pageRename").mockReturnValueOnce(rename.promise);
  const command = withRecordCommands(transport).pageRename("page-a", "Title");
  const drain = drainCommands();
  rename.resolve({ status: "error", error: { Io: "read failed" } });
  await drain;
  expect(commandFailures()).toEqual({ docs: ["workspace"], settings: false });
  await drainCommands();
  expect(await command).toEqual({ status: "error", error: "NotActive" });
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
  await drain;
  expect(commandFailures()).toEqual({ docs: [], settings: false });
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
  expect(pending[1]).toBe(pending[2]);
  const drain = drainCommands();
  try {
    expect(rename.mock.calls).toEqual([]);
  } finally {
    held.resolve({ status: "ok", data: "new-page" });
    await Promise.all(pending);
  }
  await drain;
  expect(commandFailures()).toEqual({ docs: [], settings: false });
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
    await drain;
    expect(commandFailures()).toEqual({ docs: [], settings: false });
  },
);

test("failed operations retry before newer commands and keep each edit in FIFO order", async () => {
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Before" }]);
  const native = transport.pageRename;
  const calls: string[] = [];
  let failures = 1;
  vi.spyOn(transport, "pageRename").mockImplementation(async (id, title) => {
    calls.push(title);
    return failures-- > 0
      ? { status: "error", error: { Io: "read failed" } }
      : native(id, title);
  });
  const commands = withRecordCommands(transport);
  const first = commands.pageRename("a", "First");
  await new Promise((done) => setTimeout(done, 0));
  const newer = commands.pageRename("a", "Second");
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(await newer).toEqual({ status: "ok", data: null });
  expect(await first).toEqual({ status: "ok", data: null });
  expect(calls).toEqual(["First", "First", "Second"]);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Second"]);
});

test("an uncertain create retries the same client ID and never creates a second page", async () => {
  const native = transport.pageCreate;
  const calls: (string | null)[] = [];
  let first = true;
  vi.spyOn(transport, "pageCreate").mockImplementation(async (parent, id) => {
    calls.push(id);
    const reply = await native(parent, id);
    if (first) {
      first = false;
      throw new Error("reply lost after acceptance");
    }
    return reply;
  });
  const result = withRecordCommands(transport).pageCreate(null);
  await drainCommands();
  expect(await result).toEqual({ status: "ok", data: calls[0] });
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  const tree = window.__aelomiMock?.workspace.tree();
  expect(tree?.pages.map((p) => p.id)).toEqual([calls[0]]);
  expect(calls).toEqual([tree?.pages[0]?.id, tree?.pages[0]?.id]);
});

test("an uncertain trash retries the original batch without changing its records", async () => {
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "A" }]);
  const native = transport.pageTrash;
  let first = true;
  let accepted: unknown;
  vi.spyOn(transport, "pageTrash").mockImplementation(async (id) => {
    const reply = await native(id);
    if (first) {
      first = false;
      accepted = window.__aelomiMock?.workspace.tree();
      throw new Error("reply lost after acceptance");
    }
    return reply;
  });
  const result = withRecordCommands(transport).pageTrash("a");
  await drainCommands();
  expect(await result).toEqual({
    status: "ok",
    data: window.__aelomiMock?.workspace.tree().trash[0]?.batch_id,
  });
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(window.__aelomiMock?.workspace.tree()).toEqual(accepted);
});

test("failed record retries back off from 100 ms to a five second cap and recover without another command", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Before" }]);
  const native = transport.pageRename;
  const attempts: number[] = [];
  let failing = true;
  vi.spyOn(transport, "pageRename").mockImplementation(async (id, title) => {
    attempts.push(Date.now());
    return failing
      ? { status: "error", error: { Io: "read failed" } }
      : native(id, title);
  });
  try {
    const result = withRecordCommands(transport).pageRename("a", "Recovered");
    await vi.advanceTimersByTimeAsync(16300);
    expect(attempts).toEqual([
      0, 100, 300, 700, 1500, 3100, 6300, 11300, 16300,
    ]);
    failing = false;
    await vi.advanceTimersByTimeAsync(5000);
    expect(await result).toEqual({ status: "ok", data: null });
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Recovered"]);
    await drainCommands();
    expect(commandFailures()).toEqual({ docs: [], settings: false });
  } finally {
    failing = false;
    await drainCommands();
    vi.useRealTimers();
  }
});

test("TREE-1 a caller-owned create ID orders its title behind its create", async () => {
  const calls: unknown[][] = [];
  const create = transport.pageCreate;
  const rename = transport.pageRename;
  vi.spyOn(transport, "pageCreate").mockImplementation((parent, id) => {
    calls.push(["pageCreate", parent, id]);
    return create(parent, id);
  });
  vi.spyOn(transport, "pageRename").mockImplementation((id, title) => {
    calls.push(["pageRename", id, title]);
    return rename(id, title);
  });
  const commands = withRecordCommands(transport);
  const created = commands.pageCreate(null, "c0ffee");
  const renamed = commands.pageRename("c0ffee", "T");
  expect(await created).toEqual({ status: "ok", data: "c0ffee" });
  await renamed;
  expect(calls).toEqual([
    ["pageCreate", null, "c0ffee"],
    ["pageRename", "c0ffee", "T"],
  ]);
  expect(newPageId()).toMatch(/^[0-9a-f]{32}$/);
});

test("TREE-1 a retained create coalesces its queued title to the latest draft", async () => {
  vi.useFakeTimers();
  const calls: unknown[][] = [];
  const create = transport.pageCreate;
  const rename = transport.pageRename;
  let failing = true;
  vi.spyOn(transport, "pageCreate").mockImplementation((parent, id) => {
    calls.push(["pageCreate", parent, id]);
    return failing
      ? Promise.resolve({ status: "error", error: { Io: "held" } })
      : create(parent, id);
  });
  vi.spyOn(transport, "pageRename").mockImplementation((id, title) => {
    calls.push(["pageRename", id, title]);
    return rename(id, title);
  });
  const commands = withRecordCommands(transport);
  const pending = [
    commands.pageCreate(null, "c0ffee"),
    commands.pageRename("c0ffee", "T1"),
    commands.pageRename("c0ffee", "T2"),
  ];
  await vi.advanceTimersByTimeAsync(0);
  failing = false;
  await vi.advanceTimersByTimeAsync(100);
  await Promise.all(pending);
  expect(calls).toEqual([
    ["pageCreate", null, "c0ffee"],
    ["pageCreate", null, "c0ffee"],
    ["pageRename", "c0ffee", "T2"],
  ]);
  vi.useRealTimers();
});
