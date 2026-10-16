import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands } from "../ipc";
import { installMock, commands as transport } from "../ipc/mock";
import { drainCommands, setCommandAdmission } from "../ipc/pending-commands";
import { usePageTitlesStore } from "./page-titles";
import { connectPageTreeEvents, usePageTreeStore } from "./page-tree";
import { useQuitStore } from "./quit";
import { useRecordCommandStore } from "./record-commands";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  const { withRecordCommands } = await import("../ipc/record-commands");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: withRecordCommands(mock.commands),
    events: mock.events,
  };
});

let connection: ReturnType<typeof connectPageTreeEvents>;
beforeEach(async () => {
  vi.stubGlobal("window", {});
  installMock();
  setCommandAdmission("open");
  useQuitStore.setState({
    registered: true,
    decision: { kind: "open", revision: 0 },
  });
  window.__aelomiMock?.workspace.seed([{ id: "a", title: "Original" }]);
  connection = connectPageTreeEvents();
  await connection.ready;
});
afterEach(async () => {
  vi.restoreAllMocks();
  await drainCommands();
  connection.dispose();
  usePageTitlesStore.setState({ drafts: new Map() });
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("TREE-7 a retained rename stays a draft without an error until retry acknowledgement", async () => {
  vi.spyOn(transport, "pageRename").mockResolvedValueOnce({
    status: "error",
    error: { Io: "read failed" },
  });
  await usePageTitlesStore.getState().rename("a", "Retried title");
  expect(usePageTitlesStore.getState().drafts.get("a")?.error).toBe(null);
  expect([...useRecordCommandStore.getState().retained.values()]).toMatchObject(
    [{ op: "rename", pageId: "a", attempts: 1 }],
  );
  expect(usePageTitlesStore.getState().drafts.get("a")?.title).toBe(
    "Retried title",
  );
  await vi.waitFor(() =>
    expect(usePageTitlesStore.getState().drafts.size).toBe(0),
  );
  const stored = await commands.pageTree();
  expect(
    stored.status === "ok" && stored.data.pages.map((p) => p.title),
  ).toEqual(["Retried title"]);
});

test("TREE-7 an unavailable title cannot submit a rename against an empty baseline", async () => {
  usePageTreeStore.setState({
    source: { kind: "error", message: "refresh failed" },
  });
  await usePageTitlesStore.getState().rename("a", "x");
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Original"]);
  await usePageTreeStore.getState().refresh();
  await usePageTitlesStore.getState().rename("a", "Known title");
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Known title"]);
});

test("TREE-7 a matching old snapshot cannot acknowledge a newer queued draft", async () => {
  const native = transport.pageRename;
  let release = () => {};
  vi.spyOn(transport, "pageRename").mockImplementationOnce(
    (id, title) =>
      new Promise((done) => {
        release = () => void native(id, title).then(done);
      }),
  );
  const first = usePageTitlesStore.getState().rename("a", "Intermediate");
  const last = usePageTitlesStore.getState().rename("a", "Original");
  expect(usePageTitlesStore.getState().drafts.get("a")?.title).toBe("Original");
  release();
  await Promise.all([first, last]);
  await vi.waitFor(() =>
    expect(usePageTitlesStore.getState().drafts.size).toBe(0),
  );
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Original"]);
});

test("TREE-7 a final rename refusal keeps the draft and shows a failure", async () => {
  vi.spyOn(transport, "pageRename").mockResolvedValueOnce({
    status: "error",
    error: "NotActive",
  });
  await usePageTitlesStore.getState().rename("a", "Unsaved title");
  expect(usePageTitlesStore.getState().drafts.get("a")).toMatchObject({
    title: "Unsaved title",
    error: "This title couldn't be changed. Your text is kept here.",
  });
  expect(
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  ).toEqual(["Original"]);
});

test("TREE-7 a matching source title stays a draft until the retained operation is acknowledged", async () => {
  vi.spyOn(transport, "pageRename").mockResolvedValueOnce({
    status: "error",
    error: { Io: "read failed" },
  });
  let finish = (
    _reply: Awaited<ReturnType<typeof transport.storageFlush>>,
  ) => {};
  vi.spyOn(transport, "storageFlush").mockImplementationOnce(
    () =>
      new Promise((done) => {
        finish = done;
      }),
  );
  await usePageTitlesStore.getState().rename("a", "Durable title");
  try {
    await vi.waitFor(() =>
      expect(
        window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
      ).toEqual(["Durable title"]),
    );
    expect(usePageTitlesStore.getState().drafts.get("a")?.title).toBe(
      "Durable title",
    );
    expect(useRecordCommandStore.getState().retained.size).toBe(1);
  } finally {
    finish({ status: "ok", data: true });
  }
  await vi.waitFor(() =>
    expect(usePageTitlesStore.getState().drafts.size).toBe(0),
  );
  expect(useRecordCommandStore.getState().retained.size).toBe(0);
});
