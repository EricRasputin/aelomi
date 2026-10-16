import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { failing, useRecordCommandStore } from "../stores/record-commands";
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

test("SHELL-10 retained rename publishes every attempt and stays visible until durable recovery", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(1234);
  const native = transport.pageRename;
  const rename = vi
    .spyOn(transport, "pageRename")
    .mockResolvedValue({ status: "error", error: { Io: "read failed" } });
  const commands = withRecordCommands(transport);
  const result = commands.pageRename("a", "Recovered");
  await drainCommands();
  const retained = [...useRecordCommandStore.getState().retained.values()][0];
  expect(retained).toEqual({
    id: expect.any(String),
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
  expect(failing(useRecordCommandStore.getState())).toBe(true);
  flush({ status: "ok", data: true });
  await drain;
  expect(await result).toEqual({ status: "ok", data: null });
  expect(failing(useRecordCommandStore.getState())).toBe(false);
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Recovered"]);
});

test("SHELL-10 trash stays pending until the retained operation lands", async () => {
  vi.spyOn(transport, "pageTrash").mockResolvedValueOnce({
    status: "error",
    error: { Io: "read failed" },
  });
  const result = withRecordCommands(transport).pageTrash("a");
  await new Promise((done) => setTimeout(done, 0));
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.id)).toEqual([
    "a",
  ]);
  await drainCommands();
  const tree = window.__aelomiMock?.workspace.tree();
  expect(await result).toEqual({
    status: "ok",
    data: tree?.trash[0]?.batch_id,
  });
  expect(tree?.pages).toEqual([]);
  expect(tree?.trash.map((p) => p.id)).toEqual(["a"]);
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
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Next"]);
  },
);

test.each(["NotActive", "OrderExhausted"] as const)(
  "a %s refusal on retry reaches its caller before a later success clears its failed notice",
  async (error) => {
    vi.spyOn(transport, "pageRename")
      .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
      .mockResolvedValueOnce({ status: "error", error });
    const commands = withRecordCommands(transport);
    const refused = commands.pageRename("a", "Refused");
    await new Promise((done) => setTimeout(done, 0));
    const next = commands.pageRename("a", "Next");
    expect(await next).toEqual({ status: "ok", data: null });
    expect(await refused).toEqual({ status: "error", error });
    await drainCommands();
    expect(failing(useRecordCommandStore.getState())).toBe(false);
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    expect(
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ).toEqual(["Next"]);
  },
);

test("Quitting on retry retains the operation until admission recovers", async () => {
  vi.spyOn(transport, "pageRename")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } })
    .mockResolvedValueOnce({ status: "error", error: "Quitting" });
  const result = withRecordCommands(transport).pageRename("a", "Recovered");
  await new Promise((done) => setTimeout(done, 0));
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: ["workspace"], settings: false });
  expect(
    [...useRecordCommandStore.getState().retained.values()].map((op) => [
      op.pageId,
      op.error,
      op.attempts,
    ]),
  ).toEqual([["a", "Quitting", 2]]);
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(await result).toEqual({ status: "ok", data: null });
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Recovered"]);
});
