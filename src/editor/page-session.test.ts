// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands } from "../ipc";
import { installMock } from "../ipc/mock";
import { drainDeliveries, useDeliveryStore } from "./delivery";
import { openPage, type PageSession } from "./page-session";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: mock.commands,
    events: mock.events,
  };
});

function backend() {
  const mock = window.__aelomiMock;
  if (!mock) throw new Error("Missing IPC mock");
  return mock;
}

async function open(): Promise<Extract<PageSession, { kind: "editable" }>> {
  const session = await openPage({
    pageId: "page-daily",
    onPickerKey: () => false,
    onTrip: () => {},
  });
  if (session.kind !== "editable") throw new Error(session.kind);
  return session;
}

async function typed(
  session: Extract<PageSession, { kind: "editable" }>,
  text: string,
) {
  session.editor.insertInlineContent(text);
  await drainDeliveries();
  return backend().bodies.text("page-daily");
}

beforeEach(() => {
  installMock();
  backend().workspace.seed([{ id: "page-daily", title: "Daily" }]);
  backend().bodies.seed("page-daily", [
    { id: "block-plan", content: ["plan"] },
  ]);
});
afterEach(async () => {
  backend().storage.refuseAppends(null);
  vi.restoreAllMocks();
  await drainDeliveries();
  vi.useRealTimers();
  backend().reset();
});

test("closing a mounted page keeps its doc syncing until the editor unmounts", async () => {
  const session = await open();
  const host = document.createElement("div");
  document.body.append(host);
  const unmount = session.mount(host);
  session.editor.setTextCursorPosition("block-plan", "end");
  session.close();
  expect(await typed(session, "x")).toBe("planx");
  unmount();
  unmount();
  expect(host.childElementCount).toBe(0);
  const appends = backend().storage.updates("page-daily").length;
  session.close();
  expect(backend().storage.updates("page-daily")).toHaveLength(appends);
});

test("unmounting first, then closing, ends the page once", async () => {
  const session = await open();
  const host = document.createElement("div");
  document.body.append(host);
  const unmount = session.mount(host);
  session.editor.setTextCursorPosition("block-plan", "end");
  expect(await typed(session, "y")).toBe("plany");
  unmount();
  session.close();
  session.close();
  expect(backend().bodies.text("page-daily")).toBe("plany");
});

test("a page reopened before recovery shows every refused edit and subsequent typing persists both", async () => {
  const first = await open();
  const unmountFirst = first.mount(document.createElement("div"));
  vi.useFakeTimers();
  backend().storage.refuseAppends({ Io: "Read failed" });
  first.editor.setTextCursorPosition("block-plan", "end");
  first.editor.insertInlineContent(" x");
  await vi.advanceTimersByTimeAsync(0);
  first.close();
  unmountFirst();
  expect(backend().bodies.text("page-daily")).toBe("plan");
  const reopened = await open();
  const unmountReopened = reopened.mount(document.createElement("div"));
  expect(reopened.editor.document[0]?.content).toEqual([
    { type: "text", text: "plan x", styles: {} },
  ]);
  backend().storage.refuseAppends(null);
  await vi.advanceTimersByTimeAsync(100);
  expect([...useDeliveryStore.getState().failing]).toEqual([]);
  expect(backend().bodies.text("page-daily")).toBe("plan x");
  expect(reopened.editor.document[0]?.content).toEqual([
    { type: "text", text: "plan x", styles: {} },
  ]);
  reopened.editor.setTextCursorPosition("block-plan", "end");
  expect(await typed(reopened, " y")).toBe("plan x y");
  reopened.close();
  unmountReopened();
});

test("a reopened load includes an in-flight edit accepted after its stored snapshot was read", async () => {
  const first = await open();
  const unmountFirst = first.mount(document.createElement("div"));
  const append = commands.docAppend;
  let release = async () => {};
  vi.spyOn(commands, "docAppend").mockImplementationOnce(
    (docId, update) =>
      new Promise((done) => {
        release = async () => {
          done(await append(docId, update));
        };
      }),
  );
  first.editor.setTextCursorPosition("block-plan", "end");
  first.editor.insertInlineContent(" x");
  first.close();
  unmountFirst();
  const load = commands.docLoad;
  vi.spyOn(commands, "docLoad").mockImplementationOnce(async (docId) => {
    const snapshot = await load(docId);
    await release();
    return snapshot;
  });
  const reopened = await open();
  const unmountReopened = reopened.mount(document.createElement("div"));
  expect(reopened.editor.document[0]?.content).toEqual([
    { type: "text", text: "plan x", styles: {} },
  ]);
  expect(backend().bodies.text("page-daily")).toBe("plan x");
  reopened.close();
  unmountReopened();
});
