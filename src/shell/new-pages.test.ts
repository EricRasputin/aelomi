import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands } from "../ipc";
import {
  commandFailures,
  drainCommands,
  setCommandAdmission,
} from "../ipc/pending-commands";
import { usePageTitlesStore } from "../stores/page-titles";
import { usePageTreeStore } from "../stores/page-tree";
import { useQuitStore } from "../stores/quit";
import {
  createNewPage,
  forgetNewPage,
  retryNewPage,
  settleNewPage,
  useNewPagesStore,
} from "./new-pages";

vi.mock("../ipc", async () => ({
  commands: { pageCreate: vi.fn(), pageRename: vi.fn() },
}));
beforeEach(() => {
  setCommandAdmission("open");
  useQuitStore.setState({
    registered: true,
    decision: { kind: "open", revision: 0 },
  });
  usePageTreeStore.setState({ source: { kind: "loading" } });
  vi.mocked(commands.pageCreate).mockImplementation(
    () => new Promise(() => {}),
  );
  vi.mocked(commands.pageRename).mockResolvedValue({
    status: "ok",
    data: null,
  });
});
afterEach(() => {
  for (const id of useNewPagesStore.getState().pages.keys()) forgetNewPage(id);
  usePageTitlesStore.setState({ drafts: new Map() });
  vi.clearAllMocks();
});
test.each([
  [{ status: "ok", data: "n" }, false, "created"],
  [{ status: "error", error: { Internal: "x" } }, false, "failed"],
  [{ status: "error", error: "Abandoned" }, false, "creating"],
  [{ status: "error", error: "AlreadyExists" }, false, "failed"],
  [{ status: "error", error: "AlreadyExists" }, true, "created"],
] satisfies [
  Awaited<ReturnType<typeof commands.pageCreate>>,
  boolean,
  string,
][])(
  "TREE-1 create settlement %j retry %s becomes %s",
  (reply, retry, expected) => {
    useNewPagesStore.setState({ pages: new Map([["n", "creating"]]) });
    settleNewPage("n", reply, retry);
    settleNewPage("unknown", reply, retry);
    expect([...useNewPagesStore.getState().pages]).toEqual([["n", expected]]);
  },
);
test("SHELL-10 a failed page with text retains a quit retry using the same ID", async () => {
  const id = createNewPage();
  expect(usePageTitlesStore.getState().rename(id, "Kept")).toBe(null);
  settleNewPage(id, { status: "error", error: { Internal: "x" } }, false);
  expect(commandFailures()).toEqual(["workspace"]);
  vi.clearAllMocks();
  retryNewPage(id);
  expect(commandFailures()).toEqual([]);
  expect(vi.mocked(commands.pageCreate).mock.calls).toEqual([[null, id]]);
  expect(vi.mocked(commands.pageRename).mock.calls).toEqual([[id, "Kept"]]);
  expect(usePageTitlesStore.getState().drafts.get(id)?.title).toBe("Kept");
  await drainCommands();
});
test("SHELL-10 an empty failed page needs no retained title retry", () => {
  const id = createNewPage();
  settleNewPage(id, { status: "error", error: { Internal: "x" } }, false);
  expect([...useNewPagesStore.getState().pages]).toEqual([[id, "failed"]]);
  expect(commandFailures()).toEqual([]);
});
test("TREE-1 a fresh tree forgets listed settled entries but keeps creating entries", () => {
  useNewPagesStore.setState({
    pages: new Map([
      ["a", "created"],
      ["b", "creating"],
    ]),
  });
  const pages = ["a", "b"].map((id) => ({
    id,
    title: "",
    parent_id: null,
    created_at: null,
    updated_at: null,
  }));
  usePageTreeStore.setState({
    source: {
      kind: "ready",
      tree: { pages, favorites: [], trash: [] },
      byId: new Map(pages.map((p) => [p.id, p])),
    },
  });
  expect([...useNewPagesStore.getState().pages]).toEqual([["b", "creating"]]);
});
