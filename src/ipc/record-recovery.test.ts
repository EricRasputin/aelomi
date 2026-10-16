import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useRecordCommandStore } from "../stores/record-commands";
import { installMock, commands as transport } from "./mock";
import {
  commandFailures,
  drainCommands,
  setCommandAdmission,
} from "./pending-commands";
import { withRecordCommands } from "./record-commands";

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  setCommandAdmission("open");
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Before" }]);
});
afterEach(async () => {
  vi.restoreAllMocks();
  await drainCommands();
  window.__aelomiMock?.reset();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test("SHELL-10 a second user create reuses the retained page and lands exactly once", async () => {
  const native = transport.pageCreate;
  const create = vi
    .spyOn(transport, "pageCreate")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } });
  const commands = withRecordCommands(transport);
  const first = await commands.pageCreate(null);
  expect(first).toMatchObject({
    status: "retrying",
    pageId: expect.any(String),
    operationId: expect.any(String),
  });
  create.mockImplementation(native);
  expect(await commands.pageCreate(null)).toEqual(first);
  await drainCommands();
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
    first.status === "retrying" ? first.pageId : "missing",
  ]);
  expect(useRecordCommandStore.getState().retained.size).toBe(0);
});

test("SHELL-10 retained rename publishes every attempt and stays visible until durable recovery", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(1234);
  const native = transport.pageRename;
  const rename = vi
    .spyOn(transport, "pageRename")
    .mockResolvedValue({ status: "error", error: { Io: "read failed" } });
  const commands = withRecordCommands(transport);
  const result = await commands.pageRename("a", "Recovered");
  expect(result).toMatchObject({ status: "retrying", pageId: "a" });
  const retained = [...useRecordCommandStore.getState().retained.values()][0];
  expect(retained).toEqual({
    id: result.status === "retrying" ? result.operationId : "missing",
    op: "rename",
    pageId: "a",
    error: { Io: "read failed" },
    attempts: 1,
    since: 1234,
  });
  await vi.advanceTimersByTimeAsync(100);
  expect([...useRecordCommandStore.getState().retained.values()][0]).toEqual({
    ...retained,
    attempts: 2,
  });
  rename.mockImplementation(native);
  let flush = (
    _value: Awaited<ReturnType<typeof transport.storageFlush>>,
  ) => {};
  vi.spyOn(transport, "storageFlush").mockImplementationOnce(
    () =>
      new Promise((done) => {
        flush = done;
      }),
  );
  const drain = drainCommands();
  await vi.advanceTimersByTimeAsync(0);
  expect(useRecordCommandStore.getState().failing).toBe(true);
  flush({ status: "ok", data: true });
  await drain;
  expect(useRecordCommandStore.getState().failing).toBe(false);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Recovered"]);
});

test("SHELL-10 trash reports retrying before the retained operation lands", async () => {
  vi.spyOn(transport, "pageTrash").mockResolvedValueOnce({
    status: "error",
    error: { Io: "read failed" },
  });
  const result = await withRecordCommands(transport).pageTrash("a");
  expect(result).toMatchObject({
    status: "retrying",
    pageId: "a",
    operationId: expect.any(String),
  });
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
  ]);
  await drainCommands();
  expect(window.__aelomiMock?.workspace.tree().pages).toEqual([]);
  expect(window.__aelomiMock?.workspace.tree().trash.map((p) => p.id)).toEqual([
    "a",
  ]);
});

test.each([
  "Unavailable",
  "InvalidUpdate",
  { Malformed: { reason: "bad source" } },
  { Internal: "planner panicked" },
] as const)(
  "a terminal head %j lets the next queued operation run",
  async (error) => {
    const native = transport.pageRename;
    vi.spyOn(transport, "pageRename").mockImplementation((id, title) =>
      title === "Rejected"
        ? Promise.resolve({ status: "error", error })
        : native(id, title),
    );
    const commands = withRecordCommands(transport);
    const head = commands.pageRename("a", "Rejected");
    const next = commands.pageRename("a", "Next");
    expect(await head).toEqual({ status: "error", error });
    expect(await next).toEqual({ status: "ok", data: null });
    await drainCommands();
    expect(commandFailures()).toEqual([]);
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Next"]);
  },
);

test.each(["NotActive", "OrderExhausted"] as const)(
  "a %s refusal on retry dequeues the operation",
  async (error) => {
    vi.spyOn(transport, "pageRename")
      .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
      .mockResolvedValueOnce({ status: "error", error });
    const commands = withRecordCommands(transport);
    expect(await commands.pageRename("a", "Refused")).toMatchObject({
      status: "retrying",
    });
    const next = commands.pageRename("a", "Next");
    expect(await next).toEqual({ status: "ok", data: null });
    await drainCommands();
    expect(useRecordCommandStore.getState().failing).toBe(false);
    expect(commandFailures()).toEqual([]);
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Next"]);
  },
);

test("Quitting on retry retains the operation until admission recovers", async () => {
  vi.spyOn(transport, "pageRename")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
    .mockResolvedValueOnce({ status: "error", error: "Quitting" });
  const result = await withRecordCommands(transport).pageRename(
    "a",
    "Recovered",
  );
  expect(result).toMatchObject({ status: "retrying" });
  await drainCommands();
  expect(commandFailures()).toEqual(["workspace"]);
  expect(
    [...useRecordCommandStore.getState().retained.values()].map((op) => [
      op.pageId,
      op.error,
      op.attempts,
    ]),
  ).toEqual([["a", "Quitting", 2]]);
  await drainCommands();
  expect(commandFailures()).toEqual([]);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Recovered"]);
});
