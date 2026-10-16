import { afterEach, beforeEach, expect, test, vi } from "vitest";
import * as recordState from "../stores/record-commands";
import { installMock, commands as transport } from "./mock";
import { drainCommands, setCommandAdmission } from "./pending-commands";
import { withRecordCommands } from "./record-commands";

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  setCommandAdmission("open");
  window.__aelomiMock?.workspace.seed([
    { id: "a", title: "Before" },
    { id: "b", title: "Other" },
  ]);
});
afterEach(async () => {
  vi.restoreAllMocks();
  await drainCommands();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test.each([
  "NotActive",
  "OrderExhausted",
  "InvalidUpdate",
  "Unavailable",
  { Internal: "planner panicked" },
  { Malformed: { reason: "bad source" } },
] as const)(
  "SHELL-10 an Io then terminal retry %j settles its caller and keeps the failure visible while later pages run",
  async (error) => {
    vi.spyOn(transport, "pageRename")
      .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
      .mockResolvedValueOnce({ status: "error", error });
    const commands = withRecordCommands(transport);
    const head = commands.pageRename("a", "Lost");
    await drainCommands();
    expect(await head).toEqual({ status: "error", error });
    expect([
      ...recordState.useRecordCommandStore.getState().failed.values(),
    ]).toEqual([
      {
        id: expect.any(String),
        op: "rename",
        pageId: "a",
        error,
        at: expect.any(Number),
      },
    ]);
    expect(await commands.pageRename("b", "Next")).toEqual({
      status: "ok",
      data: null,
    });
    expect(
      recordState.failing(recordState.useRecordCommandStore.getState()),
    ).toBe(true);
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Before", "Next"]);
    expect(await commands.pageRename("a", "Recovered")).toEqual({
      status: "ok",
      data: null,
    });
    expect(
      recordState.failing(recordState.useRecordCommandStore.getState()),
    ).toBe(false);
  },
);

test("SHELL-10 a failing create exposes no page ID until acceptance and durable recovery", async () => {
  const create = vi
    .spyOn(transport, "pageCreate")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } });
  let flush = (
    _reply: Awaited<ReturnType<typeof transport.storageFlush>>,
  ) => {};
  vi.spyOn(transport, "storageFlush").mockImplementationOnce(
    () =>
      new Promise((done) => {
        flush = done;
      }),
  );
  const commands = withRecordCommands(transport);
  const replies: unknown[] = [];
  const result = commands.pageCreate(null);
  void result.then((reply) => replies.push(reply));
  await new Promise((done) => setTimeout(done, 0));
  expect(replies).toEqual([]);
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
    "b",
  ]);
  const drain = drainCommands();
  await new Promise((done) => setTimeout(done, 0));
  expect(replies).toEqual([]);
  expect(recordState.pendingCreate(null)).toBe(true);
  flush({ status: "ok", data: true });
  await drain;
  const id = create.mock.calls[0]?.[1];
  expect(await result).toEqual({ status: "ok", data: id });
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
    "b",
    id,
  ]);
  expect(recordState.pendingCreate(null)).toBe(false);
});

test("SHELL-4 two explicit creates behind a failing rename remain distinct and pending for the whole wait", async () => {
  const native = transport.pageRename;
  const rename = vi
    .spyOn(transport, "pageRename")
    .mockResolvedValue({ status: "error", error: { Io: "read failed" } });
  const commands = withRecordCommands(transport);
  const head = commands.pageRename("a", "Recovered");
  await drainCommands();
  const first = commands.pageCreate(null);
  const second = commands.pageCreate(null);
  expect(first).not.toBe(second);
  expect(recordState.pendingCreate(null)).toBe(true);
  expect(recordState.pendingCreate("a")).toBe(false);
  await drainCommands();
  expect(recordState.pendingCreate(null)).toBe(true);
  rename.mockImplementation(native);
  await drainCommands();
  expect(await head).toEqual({ status: "ok", data: null });
  const replies = await Promise.all([first, second]);
  const ids = replies.map((r) => (r.status === "ok" ? r.data : "failed"));
  expect(new Set(ids).size).toBe(2);
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
    "b",
    ...ids,
  ]);
  expect(recordState.pendingCreate(null)).toBe(false);
});

test("SHELL-9 Quit anyway settles retained running and queued callers with Abandoned before native exit", async () => {
  let release = (
    _reply: Awaited<ReturnType<typeof transport.pageRename>>,
  ) => {};
  vi.spyOn(transport, "pageRename")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
    .mockImplementationOnce(
      () =>
        new Promise((done) => {
          release = done;
        }),
    );
  const commands = withRecordCommands(transport);
  const head = commands.pageRename("a", "Retained");
  await drainCommands();
  const next = commands.pageCreate(null);
  const replies: unknown[] = [];
  void head.then((reply) => replies.push(reply));
  void next.then((reply) => replies.push(reply));
  vi.spyOn(transport, "quitAnyway").mockImplementation(async () => {
    await Promise.resolve();
    expect(replies).toEqual([
      { status: "error", error: "Abandoned" },
      { status: "error", error: "Abandoned" },
    ]);
  });
  try {
    await commands.quitAnyway();
    expect(await head).toEqual({ status: "error", error: "Abandoned" });
    expect(await next).toEqual({ status: "error", error: "Abandoned" });
    expect(recordState.pendingCreate(null)).toBe(false);
  } finally {
    release({ status: "ok", data: null });
  }
});

test("SHELL-10 banner dismissal removes the terminal failure without replaying its edit", async () => {
  const rename = vi
    .spyOn(transport, "pageRename")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
    .mockResolvedValueOnce({ status: "error", error: "NotActive" });
  const result = withRecordCommands(transport).pageRename("a", "Rejected");
  await drainCommands();
  expect(await result).toEqual({ status: "error", error: "NotActive" });
  const state = recordState.useRecordCommandStore.getState();
  expect(recordState.failing(state)).toBe(true);
  for (const id of state.failed.keys()) state.dismissFailed(id);
  await drainCommands();
  expect(
    recordState.failing(recordState.useRecordCommandStore.getState()),
  ).toBe(false);
  expect(rename.mock.calls).toEqual([
    ["a", "Rejected"],
    ["a", "Rejected"],
  ]);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Before", "Other"]);
});

test("SHELL-10 resetting the observer snapshot cannot change retained create retry semantics", async () => {
  vi.spyOn(transport, "pageCreate").mockResolvedValueOnce({
    status: "error",
    error: { Io: "read failed" },
  });
  const flush = vi.spyOn(transport, "storageFlush");
  const result = withRecordCommands(transport).pageCreate(null);
  await new Promise((done) => setTimeout(done, 0));
  recordState.useRecordCommandStore.setState({
    retained: new Map(),
    pendingCreates: new Set(),
  });
  await drainCommands();
  const reply = await result;
  expect(reply).toEqual({
    status: "ok",
    data: window.__aelomiMock?.workspace.tree().pages[2]?.id,
  });
  expect(flush.mock.calls).toEqual([[]]);
});
