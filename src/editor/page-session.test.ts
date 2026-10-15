// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { installMock } from "../ipc/mock";
import { drainDeliveries } from "./delivery";
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
afterEach(() => backend().reset());

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
