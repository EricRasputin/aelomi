import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, events, type StorageStatus } from "../ipc";
import { installMock } from "../ipc/mock";
import {
  connectStorageStartup,
  useStorageStartupStore,
} from "./storage-startup";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: mock.commands,
    events: mock.events,
  };
});
const connections: ReturnType<typeof connectStorageStartup>[] = [];
function connect() {
  const connection = connectStorageStartup();
  connections.push(connection);
  return connection;
}
beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  useStorageStartupStore.setState(
    useStorageStartupStore.getInitialState(),
    true,
  );
});
afterEach(() => {
  for (const connection of connections.splice(0)) connection.dispose();
  vi.restoreAllMocks();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("reads startup and receives later refusals", async () => {
  await connect().ready;
  expect(useStorageStartupStore.getState().startup).toEqual({ kind: "ready" });
  await events.storageStatusChanged.emit({
    CloudPath: { resolved: "/cloud/data" },
  });
  expect(useStorageStartupStore.getState().startup).toMatchObject({
    kind: "refused",
    detail: "/cloud/data",
    recovery: "moveFolder",
  });
});

test("an event supersedes a delayed startup snapshot", async () => {
  let resolve = (_status: StorageStatus): void => {};
  const snapshot = new Promise<StorageStatus>((done) => {
    resolve = done;
  });
  const read = vi.spyOn(commands, "storageStatus").mockReturnValue(snapshot);
  const connection = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  await events.storageStatusChanged.emit("Locked");
  resolve("Ready");
  await connection.ready;
  expect(useStorageStartupStore.getState().startup).toMatchObject({
    kind: "refused",
    title: "This data folder is already open",
  });
});

test("a failed status read refuses to mount the editor", async () => {
  vi.spyOn(commands, "storageStatus").mockRejectedValue(
    new Error("IPC unavailable"),
  );
  await connect().ready;
  expect(useStorageStartupStore.getState().startup).toMatchObject({
    kind: "refused",
    title: "Aelomi couldn't open its data folder",
    detail: "IPC unavailable",
  });
});

test("disposal ignores late snapshots and events", async () => {
  const connection = connect();
  connection.dispose();
  await connection.ready;
  await events.storageStatusChanged.emit("Ready");
  expect(useStorageStartupStore.getState().startup).toEqual({
    kind: "loading",
  });
});
