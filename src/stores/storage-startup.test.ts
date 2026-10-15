import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, type StorageStatus } from "../ipc";
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

test("reads the immutable native startup status once", async () => {
  const read = vi.spyOn(commands, "storageStatus").mockResolvedValue({
    CloudPath: {
      configured: "/local/data",
      resolved: "/cloud/data",
      recovery: {
        LinkAtDataFolder: {
          configured_parent: "/local",
          configured_name: "data",
          resolved_parent: "/cloud",
          resolved_name: "data",
        },
      },
    },
  });
  await connect().ready;
  expect(read.mock.calls).toEqual([[]]);
  expect(useStorageStartupStore.getState().startup).toMatchObject({
    kind: "refused",
    recovery: { configured: "/local/data", resolved: "/cloud/data" },
  });
});

test("a failed status read refuses to mount the editor", async () => {
  vi.spyOn(commands, "storageStatus").mockRejectedValue(
    new Error("IPC unavailable"),
  );
  await connect().ready;
  expect(useStorageStartupStore.getState().startup).toMatchObject({
    kind: "refused",
    title: "Aelomi couldn't start",
    detail: "IPC unavailable",
  });
});

test("disposal ignores late snapshots", async () => {
  let resolve = (_status: StorageStatus) => {};
  vi.spyOn(commands, "storageStatus").mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const connection = connect();
  connection.dispose();
  resolve("Ready");
  await connection.ready;
  expect(useStorageStartupStore.getState().startup).toEqual({
    kind: "loading",
  });
});
