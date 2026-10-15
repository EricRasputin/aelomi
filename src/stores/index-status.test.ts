import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, events, type IndexStatus } from "../ipc";
import { installMock } from "../ipc/mock";
import { connectIndexEvents, useIndexStatusStore } from "./index-status";

vi.mock("../ipc", async () => {
  const mock = await import("../ipc/mock");
  return { commands: mock.commands, events: mock.events };
});

function deferred<T>() {
  let resolve = (_value: T): void => {};
  let reject = (_reason: unknown): void => {};
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const connections: ReturnType<typeof connectIndexEvents>[] = [];
function connect() {
  const connection = connectIndexEvents();
  connections.push(connection);
  return connection;
}

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  useIndexStatusStore.setState(useIndexStatusStore.getInitialState(), true);
});
afterEach(() => {
  for (const connection of connections.splice(0)) connection.dispose();
  vi.restoreAllMocks();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("reads the startup status after listening", async () => {
  window.__aelomiMock?.index.setStatus("Rebuilding");
  await connect().ready;
  expect(useIndexStatusStore.getState().status).toBe("Rebuilding");
  window.__aelomiMock?.index.setStatus("Ready");
  expect(useIndexStatusStore.getState().status).toBe("Ready");
});

test("a Ready event during a delayed snapshot wins over the stale Rebuilding snapshot", async () => {
  const snapshot = deferred<IndexStatus>();
  const read = vi
    .spyOn(commands, "indexStatus")
    .mockReturnValue(snapshot.promise);
  const connection = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  await events.indexStatusChanged.emit("Ready");
  snapshot.resolve("Rebuilding");
  await connection.ready;
  expect(useIndexStatusStore.getState().status).toBe("Ready");
});

test("every status and index change advances the local revision", async () => {
  await connect().ready;
  const start = useIndexStatusStore.getState().revision;
  await events.indexStatusChanged.emit("Rebuilding");
  await events.indexChanged.emit({ workspace: false, pages: ["page"] });
  await events.indexStatusChanged.emit("Ready");
  expect(useIndexStatusStore.getState()).toMatchObject({
    status: "Ready",
    revision: start + 3,
  });
});

test("a failed snapshot keeps the subscriptions", async () => {
  vi.spyOn(commands, "indexStatus").mockRejectedValue(new Error("IPC down"));
  await expect(connect().ready).rejects.toThrow(
    "Index event connection failed",
  );
  await events.indexStatusChanged.emit({ Failed: { reason: "disk" } });
  expect(useIndexStatusStore.getState().status).toEqual({
    Failed: { reason: "disk" },
  });
});

test("disposal drops late registrations and later events", async () => {
  const registered = deferred<() => void>();
  const listen = events.indexStatusChanged.listen;
  vi.spyOn(events.indexStatusChanged, "listen").mockImplementation(
    async (callback) => {
      const unlisten = await listen(callback);
      registered.resolve(unlisten);
      return unlisten;
    },
  );
  const connection = connect();
  connection.dispose();
  await registered.promise;
  await events.indexStatusChanged.emit("Rebuilding");
  expect(useIndexStatusStore.getState().status).toBe(null);
});
