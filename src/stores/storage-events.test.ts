import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, events, type Health, type QuitStatus } from "../ipc";
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
  const mock = window.__aelomiMock;
  if (!mock) throw new Error("Missing IPC mock");
  return mock;
}
async function requestQuit() {
  const mock = mockBackend();
  mock.quit.request();
  const { revision, phase } = mock.quit.status;
  if (phase === "Draining") await commands.completeQuit(revision);
  return mock;
}
async function refuseQuit() {
  const mock = mockBackend();
  mock.storage.failWrites(true);
  await mock.storage.type("page", "x");
  return requestQuit();
}
function deferred<T>() {
  let resolve = (_value: T): void => {};
  let reject = (_reason: unknown): void => {};
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  useStorageHealthStore.setState({ health: null });
  useQuitStore.setState(useQuitStore.getInitialState(), true);
});
afterEach(() => {
  for (const connection of connections.splice(0)) connection.dispose();
  vi.restoreAllMocks();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("reads failing startup health after listeners register", async () => {
  vi.spyOn(commands, "storageHealth").mockResolvedValue(failing);
  await connect().ready;
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
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

test("a Keep app open that lands after a reload's snapshot still closes the new page's dialog", async () => {
  const old = connect();
  await old.ready;
  await refuseQuit();
  const keepOpen = commands.keepOpen;
  let land = () => {};
  vi.spyOn(commands, "keepOpen").mockImplementationOnce((revision) => {
    land = () => void keepOpen(revision);
    return new Promise<QuitStatus>(() => {});
  });
  void useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 2,
  });

  old.dispose();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
  await connect().ready;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 2,
    pendingDocs: ["page"],
    error: null,
  });
  land();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 3,
  });
});

test("an answer read by a fresh page's snapshot outranks the refusal's event", async () => {
  const snapshot = deferred<QuitStatus>();
  const read = vi
    .spyOn(commands, "quitStatus")
    .mockReturnValue(snapshot.promise);
  const connection = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
    pendingDocs: ["page"],
    error: null,
  });
  snapshot.resolve({ revision: 2, phase: "Open" });
  await connection.ready;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 2,
  });
});

test("a fresh page reads a kept-open quit as answered, and a delayed refusal event changes nothing", async () => {
  await refuseQuit();
  expect(await commands.keepOpen(1)).toEqual({
    revision: 2,
    phase: "Open",
  });
  await connect().ready;
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 2,
  });
});

test("a stale snapshot never hides a newer refusal", async () => {
  const snapshot = deferred<QuitStatus>();
  const read = vi
    .spyOn(commands, "quitStatus")
    .mockReturnValue(snapshot.promise);
  const connection = connect();
  await vi.waitFor(() => expect(read).toHaveBeenCalledOnce());
  await events.quitStatusChanged.emit({
    revision: 3,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  snapshot.resolve({ revision: 2, phase: "Open" });
  await connection.ready;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 3,
    pendingDocs: ["page"],
    error: null,
  });
});

test("keeping open survives a stale reconnect snapshot and takes Rust's reply", async () => {
  const reply = deferred<QuitStatus>();
  vi.spyOn(commands, "keepOpen").mockReturnValue(reply.promise);
  vi.spyOn(commands, "quitStatus").mockResolvedValue({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  useQuitStore.getState().receiveQuit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  const kept = useQuitStore.getState().keepOpen();
  await connect().ready;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 1,
  });
  reply.resolve({ revision: 2, phase: "Open" });
  await kept;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 2,
  });
});

test("a failed Keep app open asks again with an error, and a retry answers", async () => {
  await connect().ready;
  await refuseQuit();
  vi.spyOn(commands, "keepOpen").mockRejectedValueOnce(
    new Error("IPC unavailable"),
  );
  await useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 2,
    pendingDocs: ["page"],
    error: "Aelomi couldn't record your choice. Try again.",
  });
  await useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 3,
  });
});

test("a refusal that lands before a failed Keep app open is not overwritten", async () => {
  const reply = deferred<QuitStatus>();
  vi.spyOn(commands, "keepOpen").mockReturnValue(reply.promise);
  const store = useQuitStore.getState();
  store.receiveQuit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  const kept = store.keepOpen();
  store.receiveQuit({
    revision: 2,
    phase: { Asking: { pending_docs: ["other"] } },
  });
  reply.reject(new Error("IPC unavailable"));
  await kept;
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 2,
    pendingDocs: ["other"],
    error: null,
  });
});

test.each(["reply", "event"])(
  "a refusal Rust recorded before the Keep app open still asks when its %s arrives first",
  async (first) => {
    await connect().ready;
    const reply = deferred<QuitStatus>();
    const ack = vi
      .spyOn(commands, "keepOpen")
      .mockReturnValueOnce(reply.promise);
    await events.quitStatusChanged.emit({
      revision: 1,
      phase: { Asking: { pending_docs: ["page"] } },
    });
    const kept = useQuitStore.getState().keepOpen();
    const refused = {
      revision: 2,
      phase: { Asking: { pending_docs: ["page"] } },
    };
    if (first === "event") await events.quitStatusChanged.emit(refused);
    reply.resolve(refused);
    await kept;
    if (first === "reply") await events.quitStatusChanged.emit(refused);
    expect(useQuitStore.getState().decision).toEqual({
      kind: "asking",
      revision: 2,
      pendingDocs: ["page"],
      error: null,
    });
    await useQuitStore.getState().keepOpen();
    expect(ack.mock.calls).toEqual([[1], [2]]);
  },
);

test("repeated quits ask the newest question, and keeping open answers that one", async () => {
  await connect().ready;
  const mock = await refuseQuit();
  await requestQuit();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 4,
    pendingDocs: ["page"],
    error: null,
  });
  await useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 5,
  });
  await requestQuit();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 7,
    pendingDocs: ["page"],
    error: null,
  });
  expect(mock.quit.exited).toBe(false);
});

test("a hot reload keeps an answer even when the old refusal is replayed", async () => {
  const first = connect();
  await first.ready;
  await refuseQuit();
  await useQuitStore.getState().keepOpen();
  first.dispose();
  await connect().ready;
  await events.quitStatusChanged.emit({
    revision: 2,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 3,
  });
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
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 0,
  });
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
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
    pendingDocs: ["page"],
    error: null,
  });
});

test("a failed quit snapshot is reported while startup health still applies", async () => {
  vi.spyOn(commands, "quitStatus").mockRejectedValue(
    new Error("IPC unavailable"),
  );
  vi.spyOn(commands, "storageHealth").mockResolvedValue(failing);
  await expect(connect().ready).rejects.toThrow(
    "Storage event connection failed",
  );
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
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
  await events.quitStatusChanged.emit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  expect(useStorageHealthStore.getState().health).toEqual(failing);
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
    pendingDocs: ["page"],
    error: null,
  });
});

test("quit listener failure preserves health events", async () => {
  vi.spyOn(events.quitStatusChanged, "listen").mockRejectedValue(
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
  const mock = await refuseQuit();
  expect(useStorageHealthStore.getState().health).toEqual({
    Failing: {
      reason: { Io: "mock write failure" },
      pending_docs: ["page"],
    },
  });
  mock.storage.failWrites(false);
  expect(useStorageHealthStore.getState().health).toBe("Healthy");
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 2,
    pendingDocs: ["page"],
    error: null,
  });
  await useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "open",
    revision: 3,
  });
});

test("Quit anyway exits once and stays quitting through its own status", async () => {
  await connect().ready;
  const mock = await refuseQuit();
  await useQuitStore.getState().quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "quitting",
    revision: 3,
    pendingDocs: ["page"],
    asked: true,
  });
  mock.quit.request();
  await useQuitStore.getState().quitAnyway();
  expect(
    mock.calls.filter((call) => call.command === "quitAnyway").length,
  ).toBe(1);
  expect(mock.quit.exited).toBe(true);
  expect(mock.storage.text("page")).toBe("x");
});

test("double confirmation and repeated blocked requests exit once", async () => {
  const exit = deferred<void>();
  const command = vi
    .spyOn(commands, "quitAnyway")
    .mockReturnValue(exit.promise);
  const store = useQuitStore.getState();
  store.receiveQuit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  const confirmed = store.quitAnyway();
  store.receiveQuit({
    revision: 2,
    phase: { Asking: { pending_docs: ["other"] } },
  });
  await store.keepOpen();
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "quitting",
    revision: 2,
    pendingDocs: ["other"],
    asked: true,
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
  store.receiveQuit({
    revision: 1,
    phase: { Asking: { pending_docs: ["page"] } },
  });
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "asking",
    revision: 1,
    pendingDocs: ["page"],
    error: "Aelomi couldn't quit. Try again, or keep the app open.",
  });
  await store.quitAnyway();
  expect(useQuitStore.getState().decision).toEqual({
    kind: "quitting",
    revision: 1,
    pendingDocs: ["page"],
    asked: true,
  });
});
