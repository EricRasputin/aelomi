import { afterEach, beforeEach, expect, test, vi } from "vitest";
import * as Y from "yjs";
import { commands } from "../ipc";
import { installMock } from "../ipc/mock";
import { useQuitStore } from "../stores/quit";
import { connectStorageEvents } from "../stores/storage-events";
import { drainDeliveries } from "./delivery";
import { loadOrigin, syncPage } from "./page-sync";
import { connectQuitDrain, onQuitDrain } from "./quit-drain";

vi.mock("../ipc", async () => {
  const mock = await import("../ipc/mock");
  return { commands: mock.commands, events: mock.events };
});

const stops: (() => void)[] = [];
async function connect() {
  const events = connectStorageEvents();
  stops.push(events.dispose, connectQuitDrain());
  await events.ready;
}

function page(docId: string) {
  const ydoc = new Y.Doc();
  Y.applyUpdate(ydoc, Y.encodeStateAsUpdate(new Y.Doc()), loadOrigin);
  stops.push(
    syncPage({ docId, ydoc, hasUserInput: () => true, onTrip: () => {} }),
  );
  return ydoc;
}

const commandsCalled = () =>
  (window.__aelomiMock?.calls ?? [])
    .map((call) => call.command)
    .filter((command) =>
      ["docAppend", "completeQuit", "quitDrainFailed"].includes(command),
    );

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
});
afterEach(async () => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
  await drainDeliveries();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("a quit drains every edit still in flight before completing, then exits", async () => {
  await connect();
  expect(useQuitStore.getState().registered).toBe(true);
  const docAppend = commands.docAppend;
  let release = () => {};
  vi.spyOn(commands, "docAppend").mockImplementationOnce(
    (docId, update) =>
      new Promise((done) => {
        release = () => void docAppend(docId, update).then(done);
      }),
  );
  page("page-a").getText("t").insert(0, "x");
  window.__aelomiMock?.quit.request();
  window.__aelomiMock?.quit.request();
  expect(useQuitStore.getState().decision.kind).toBe("draining");
  await new Promise((done) => setTimeout(done, 10));
  expect(commandsCalled()).toEqual([]);
  release();
  await vi.waitFor(() => expect(window.__aelomiMock?.quit.exited).toBe(true));
  expect(commandsCalled()).toEqual(["docAppend", "completeQuit"]);
  expect(useQuitStore.getState().decision).toMatchObject({
    kind: "quitting",
    asked: false,
  });
});

test("a drain the store refuses asks with the refused docs instead of completing", async () => {
  await connect();
  window.__aelomiMock?.storage.setStatus("Locked");
  page("page-b").getText("t").insert(0, "y");
  window.__aelomiMock?.quit.request();
  await vi.waitFor(() =>
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      pendingDocs: ["page-b"],
    }),
  );
  expect(commandsCalled()).toEqual([
    "docAppend",
    "docAppend",
    "quitDrainFailed",
  ]);
  expect(window.__aelomiMock?.quit.exited).toBe(false);
  window.__aelomiMock?.storage.setStatus("Ready");
  await useQuitStore.getState().keepOpen();
  expect(useQuitStore.getState().decision.kind).toBe("open");
  window.__aelomiMock?.quit.request();
  await vi.waitFor(() => expect(window.__aelomiMock?.quit.exited).toBe(true));
});

test("each drain revision is answered once, and a late answer cannot complete a newer quit", async () => {
  await connect();
  const complete = vi.spyOn(commands, "completeQuit");
  window.__aelomiMock?.storage.failWrites(true);
  page("page-c").getText("t").insert(0, "z");
  window.__aelomiMock?.quit.request();
  await vi.waitFor(() =>
    expect(useQuitStore.getState().decision.kind).toBe("asking"),
  );
  expect(await commands.completeQuit(1)).toMatchObject({
    phase: { Asking: { pending_docs: ["page-c"] } },
  });
  await useQuitStore.getState().keepOpen();
  window.__aelomiMock?.storage.failWrites(false);
  window.__aelomiMock?.quit.request();
  await vi.waitFor(() => expect(window.__aelomiMock?.quit.exited).toBe(true));
  expect(complete.mock.calls.map(([revision]) => revision)).toEqual([1, 1, 4]);
});

test("before the webview registers, a quit keeps the store-only behavior", async () => {
  window.__aelomiMock?.quit.request();
  expect(window.__aelomiMock?.quit.exited).toBe(true);
  expect(commandsCalled()).toEqual([]);
});

test("a doc whose first edit lands while the drain waits on another is awaited too", async () => {
  await connect();
  const docAppend = commands.docAppend;
  const held: (() => void)[] = [];
  vi.spyOn(commands, "docAppend").mockImplementation(
    (docId, update) =>
      new Promise((done) => {
        held.push(() => void docAppend(docId, update).then(done));
      }),
  );
  page("page-d").getText("t").insert(0, "x");
  window.__aelomiMock?.quit.request();
  await new Promise((done) => setTimeout(done, 10));
  expect(held).toHaveLength(1);
  page("page-e").getText("t").insert(0, "y");
  expect(held).toHaveLength(2);
  held[0]?.();
  await new Promise((done) => setTimeout(done, 10));
  expect(commandsCalled()).toEqual(["docAppend"]);
  held[1]?.();
  await vi.waitFor(() => expect(window.__aelomiMock?.quit.exited).toBe(true));
  expect(commandsCalled()).toEqual(["docAppend", "docAppend", "completeQuit"]);
});

test("open editors stop accepting edits before the drain collects what is in flight", async () => {
  await connect();
  const order: string[] = [];
  const spy = vi.spyOn(commands, "completeQuit");
  stops.push(
    onQuitDrain({
      finish: () => order.push("finish"),
      freeze: () =>
        order.push(`freeze after ${spy.mock.calls.length} completions`),
    }),
  );
  window.__aelomiMock?.quit.request();
  await vi.waitFor(() => expect(window.__aelomiMock?.quit.exited).toBe(true));
  expect(order).toEqual(["finish", "freeze after 0 completions"]);
});
