import { afterEach, beforeEach, expect, test, vi } from "vitest";
import * as Y from "yjs";
import { commands } from "../ipc";
import { fromBase64, toBase64 } from "../ipc/base64";
import { installMock } from "../ipc/mock";
import { drainDeliveries, useDeliveryStore } from "./delivery";
import { loadOrigin, syncPage } from "./page-sync";

vi.mock("../ipc", async () => {
  const mock = await import("../ipc/mock");
  return { commands: mock.commands, events: mock.events };
});

function loaded(docId: string, text: string) {
  const source = new Y.Doc();
  source.getText("t").insert(0, text);
  const base = Y.encodeStateAsUpdate(source);
  window.__aelomiMock?.bodies.raw(docId, toBase64(base));
  const ydoc = new Y.Doc();
  Y.applyUpdate(ydoc, base, loadOrigin);
  return { ydoc, base };
}

function stored(docId: string) {
  const doc = new Y.Doc();
  Y.applyUpdate(
    doc,
    fromBase64(window.__aelomiMock?.bodies.state(docId) ?? ""),
  );
  return doc.getText("t").toString();
}

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
});
afterEach(async () => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  expect(await drainDeliveries()).toEqual([]);
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("each update is handed to docAppend before its transaction returns, with no timer and no echo of the load", () => {
  const { ydoc, base } = loaded("page-a", "plan");
  const sent: Uint8Array[] = [];
  vi.spyOn(commands, "docAppend").mockImplementation(async (_doc, update) => {
    sent.push(fromBase64(update));
    return { status: "ok", data: sent.length };
  });
  vi.useFakeTimers();
  syncPage({
    docId: "page-a",
    ydoc,
    hasUserInput: () => true,
    onTrip: () => {},
  });
  expect(sent).toEqual([]);
  ydoc.getText("t").insert(4, " x");
  expect(sent).toHaveLength(1);
  expect(vi.getTimerCount()).toBe(0);
  const replay = new Y.Doc();
  Y.applyUpdate(replay, base);
  for (const update of sent) Y.applyUpdate(replay, update);
  expect(replay.getText("t").toString()).toBe("plan x");
});

test("every update gets its own invoke while earlier ones are still in flight", async () => {
  const { ydoc } = loaded("page-b", "");
  const held: (() => void)[] = [];
  const docAppend = commands.docAppend;
  const append = vi
    .spyOn(commands, "docAppend")
    .mockImplementation(
      (docId, update) =>
        new Promise((done) =>
          held.push(() => void docAppend(docId, update).then(done)),
        ),
    );
  syncPage({
    docId: "page-b",
    ydoc,
    hasUserInput: () => true,
    onTrip: () => {},
  });
  for (const char of "abc") ydoc.getText("t").insert(0, char);
  expect(append.mock.calls.map(([doc]) => doc)).toEqual([
    "page-b",
    "page-b",
    "page-b",
  ]);
  for (const release of held) release();
  expect(await drainDeliveries()).toEqual([]);
  expect(stored("page-b")).toBe("cba");
});

test("before any user input, an update that deletes is held back and trips once; an additive one is sent", () => {
  const { ydoc } = loaded("page-c", "plan");
  const append = vi.spyOn(commands, "docAppend");
  const onTrip = vi.fn();
  let input = false;
  syncPage({ docId: "page-c", ydoc, hasUserInput: () => input, onTrip });
  ydoc.getText("t").insert(4, "!");
  ydoc.getText("t").delete(0, 1);
  ydoc.getText("t").insert(0, "P");
  input = true;
  ydoc.getText("t").delete(0, 1);
  expect(onTrip).toHaveBeenCalledOnce();
  expect(append).toHaveBeenCalledOnce();
  expect(stored("page-c")).toBe("plan!");
});

test("after user input, deletions are ordinary edits", () => {
  const { ydoc } = loaded("page-d", "plan");
  const onTrip = vi.fn();
  syncPage({ docId: "page-d", ydoc, hasUserInput: () => true, onTrip });
  ydoc.getText("t").delete(0, 2);
  expect(onTrip).not.toHaveBeenCalled();
  expect(stored("page-d")).toBe("an");
});

test("a refused update is kept, shown, retried with the next one, and reported by a drain until the store takes it", async () => {
  const { ydoc } = loaded("page-e", "");
  syncPage({
    docId: "page-e",
    ydoc,
    hasUserInput: () => true,
    onTrip: () => {},
  });
  window.__aelomiMock?.storage.setStatus("Locked");
  ydoc.getText("t").insert(0, "x");
  await vi.waitFor(() =>
    expect([...useDeliveryStore.getState().failing]).toEqual([
      ["page-e", "Unavailable"],
    ]),
  );
  expect(await drainDeliveries()).toEqual(["page-e"]);
  window.__aelomiMock?.storage.setStatus("Ready");
  ydoc.getText("t").insert(1, "y");
  expect(await drainDeliveries()).toEqual([]);
  expect([...useDeliveryStore.getState().failing]).toEqual([]);
  expect(stored("page-e")).toBe("xy");
});

test("a drain waits for every invoke still in flight, on pages already closed too", async () => {
  const { ydoc } = loaded("page-f", "");
  let accept = () => {};
  const docAppend = commands.docAppend;
  vi.spyOn(commands, "docAppend").mockImplementationOnce(
    (docId, update) =>
      new Promise((done) => {
        accept = () => void docAppend(docId, update).then(done);
      }),
  );
  const stop = syncPage({
    docId: "page-f",
    ydoc,
    hasUserInput: () => true,
    onTrip: () => {},
  });
  ydoc.getText("t").insert(0, "z");
  stop();
  ydoc.destroy();
  let drained = false;
  const drain = drainDeliveries().then((failed) => {
    drained = true;
    return failed;
  });
  await new Promise((done) => setTimeout(done, 10));
  expect(drained).toBe(false);
  accept();
  expect(await drain).toEqual([]);
  expect(stored("page-f")).toBe("z");
});
