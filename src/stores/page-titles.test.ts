import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands } from "../ipc";
import { installMock, commands as transport } from "../ipc/mock";
import { drainCommands, setCommandAdmission } from "../ipc/pending-commands";
import { connectPageTreeEvents, usePageTreeStore } from "./page-tree";
import { usePageTitlesStore } from "./page-titles";
import { useQuitStore } from "./quit";

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
  useQuitStore.setState({ registered: true, decision: { kind: "open", revision: 0 } });
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

test("TREE-7 a failed rename's retained retry clears its error and draft after the source catches up", async () => {
  vi.spyOn(transport, "pageRename").mockResolvedValueOnce({ status: "error", error: { Io: "read failed" } });
  await usePageTitlesStore.getState().rename("a", "Retried title");
  expect(usePageTitlesStore.getState().drafts.get("a")?.error).toBe("This title couldn't be saved. Aelomi will retry.");
  await vi.waitFor(() => expect(usePageTitlesStore.getState().drafts.size).toBe(0));
  const stored = await commands.pageTree();
  expect(stored.status === "ok" && stored.data.pages.map((p) => p.title)).toEqual(["Retried title"]);
});

test("TREE-7 an unavailable title cannot submit a rename against an empty baseline", async () => {
  usePageTreeStore.setState({ source: { kind: "error", message: "refresh failed" } });
  await usePageTitlesStore.getState().rename("a", "x");
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.title)).toEqual(["Original"]);
  await usePageTreeStore.getState().refresh();
  await usePageTitlesStore.getState().rename("a", "Known title");
  expect(window.__aelomiMock?.workspace.tree().pages.map((p) => p.title)).toEqual(["Known title"]);
});
