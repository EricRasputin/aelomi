import {
  afterEach,
  beforeEach,
  expect,
  onTestFinished,
  test,
  vi,
} from "vitest";
import { commands, events, type Health } from "../ipc";
import { installMock } from "../ipc/mock";
import { useQuitStore } from "./quit";
import { connectStorageEvents } from "./storage-events";
import { useStorageHealthStore } from "./storage-health";

vi.mock("../ipc", async () => {
  const mock = await import("../ipc/mock");
  return { commands: mock.commands, events: mock.events };
});

const failing: Health = {
  Failing: { reason: "DiskFull", pending_docs: ["page"] },
};
const connections: ReturnType<typeof connectStorageEvents>[] = [];
function connect() {
  const connection = connectStorageEvents();
  connections.push(connection);
  return connection;
}
function mockBackend() {
  vi.stubGlobal("window", {});
  installMock();
  const mock = window.__aelomiMock;
  if (!mock) throw new Error("Missing IPC mock");
  onTestFinished(() => {
    mock.reset();
    vi.unstubAllGlobals();
  });
  return mock;
}
function deferred<T>() {
  let resolve = (_value: T): void => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  useStorageHealthStore.setState({ health: null });
  useQuitStore.setState({ decision: { kind: "open" } });
});
afterEach(() => {
  for (const connection of connections.splice(0)) connection.dispose();
  vi.restoreAllMocks();
});

test("reads failing startup health after listeners register", async () => {
  vi.spyOn(commands, "storageHealth").mockResolvedValue(failing);
  await connect().ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  await events.quitBlocked.emit({ pending_docs: ["page"] });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    pendingDocs: ["page"],
    error: null,
  });
});

test("a quit refused before the listeners register still asks, and keeping open answers it", async () => {
  const mock = mockBackend();
  mock.storage.failWrites(true);
  await mock.storage.type("page", "x");
  mock.quit.request();
  mock.storage.failWrites(false);

  const first = connect();
  await first.ready;
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
  expect(useQuitStore.getState().decision).toMatchObject({
    kind: "asking",
    pendingDocs: ["page"],
    error: null,
  });

  useQuitStore.getState().keepOpen();
  first.dispose();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
  await connect().ready;
  expect(useQuitStore.getState().decision).toMatchObject({ kind: "open" });
});

test("an event overtakes a delayed startup snapshot", async () => {
  const snapshot = deferred<Health>();
  const read = vi
    .spyOn(commands, "storageHealth")
    .mockReturnValue(snapshot.promise);
  const connection = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  await events.storageHealthChanged.emit(failing);
  snapshot.resolve("Healthy");
  await connection.ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
});

test("an event during delayed listener registration also overtakes the snapshot", async () => {
  const registered = deferred<void>();
  const listen = events.storageHealthChanged.listen;
  vi.spyOn(events.storageHealthChanged, "listen").mockImplementation(
    async (callback) => {
      const unlisten = await listen(callback);
      await registered.promise;
      return unlisten;
    },
  );
  vi.spyOn(commands, "storageHealth").mockResolvedValue("Healthy");
  const connection = connect();
  await events.storageHealthChanged.emit(failing);
  registered.resolve();
  await connection.ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
});

test("disposing a late registration removes it and ignores later events", async () => {
  const registered = deferred<void>();
  const listen = events.storageHealthChanged.listen;
  vi.spyOn(events.storageHealthChanged, "listen").mockImplementation(
    async (callback) => {
      const unlisten = await listen(callback);
      await registered.promise;
      return unlisten;
    },
  );
  const connection = connect();
  useStorageHealthStore.getState().receiveHealth(failing);
  connection.dispose();
  registered.resolve();
  await connection.ready;
  await events.storageHealthChanged.emit("Healthy");
  await events.quitBlocked.emit({ pending_docs: ["page"] });
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  expect(useQuitStore.getState().decision).toEqual({ kind: "open" });
});

test("disposed snapshots cannot overwrite a newer connection", async () => {
  const oldSnapshot = deferred<Health>();
  const read = vi
    .spyOn(commands, "storageHealth")
    .mockReturnValueOnce(oldSnapshot.promise)
    .mockResolvedValue(failing);
  const old = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  old.dispose();
  await connect().ready;
  oldSnapshot.resolve("Healthy");
  await old.ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
});

test("a fresh connection refreshes retained health", async () => {
  const read = vi
    .spyOn(commands, "storageHealth")
    .mockResolvedValueOnce(failing)
    .mockResolvedValueOnce("Healthy");
  const first = connect();
  await first.ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  first.dispose();
  await connect().ready;
  expect(read).toHaveBeenCalledTimes(2);
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
});

test("snapshot failure preserves subscriptions and subsequent recovery", async () => {
  vi.spyOn(commands, "storageHealth").mockRejectedValue(
    new Error("IPC unavailable"),
  );
  useStorageHealthStore.getState().receiveHealth(failing);
  await expect(connect().ready).rejects.toThrow(
    "Storage event connection failed",
  );
  await events.storageHealthChanged.emit("Healthy");
  await events.quitBlocked.emit({ pending_docs: ["page"] });
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    pendingDocs: ["page"],
    error: null,
  });
});

test("health listener failure preserves quit events and startup health", async () => {
  vi.spyOn(events.storageHealthChanged, "listen").mockRejectedValue(
    new Error("listen failed"),
  );
  vi.spyOn(commands, "storageHealth").mockResolvedValue(failing);
  await expect(connect().ready).rejects.toThrow(
    "Storage event connection failed",
  );
  await events.quitBlocked.emit({ pending_docs: ["page"] });
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    pendingDocs: ["page"],
    error: null,
  });
});

test("quit listener failure preserves health events", async () => {
  vi.spyOn(events.quitBlocked, "listen").mockRejectedValue(
    new Error("listen failed"),
  );
  vi.spyOn(commands, "storageHealth").mockResolvedValue(failing);
  await expect(connect().ready).rejects.toThrow(
    "Storage event connection failed",
  );
  await events.storageHealthChanged.emit("Healthy");
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
});

test("health recovery never dismisses a blocked quit decision", async () => {
  await connect().ready;
  await events.storageHealthChanged.emit(failing);
  await events.quitBlocked.emit({ pending_docs: ["page"] });
  await events.storageHealthChanged.emit("Healthy");
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    pendingDocs: ["page"],
    error: null,
  });
  useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({ kind: "open" });
});

test("double confirmation and repeated blocked requests exit once", async () => {
  const exit = deferred<void>();
  const command = vi
    .spyOn(commands, "quitAnyway")
    .mockReturnValue(exit.promise);
  const store = useQuitStore.getState();
  store.receiveBlocked({ pending_docs: ["page"] });
  const confirmed = store.quitAnyway();
  store.receiveBlocked({ pending_docs: ["other"] });
  store.keepOpen();
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "quitting",
    pendingDocs: ["page"],
  });
  exit.resolve();
  await confirmed;
  expect(command).toHaveBeenCalledOnce();
});

test("failed confirmation restores choices and allows another decision", async () => {
  vi.spyOn(commands, "quitAnyway").mockRejectedValueOnce(
    new Error("IPC unavailable"),
  );
  const store = useQuitStore.getState();
  store.receiveBlocked({ pending_docs: ["page"] });
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    pendingDocs: ["page"],
    error: "Aelomi couldn't quit. Try again, or keep the app open.",
  });
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "quitting",
    pendingDocs: ["page"],
  });
});
