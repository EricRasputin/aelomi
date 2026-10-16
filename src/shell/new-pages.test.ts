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
  activateNewPage,
  createNewPage,
  forgetEmptyFailuresExcept,
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
    useNewPagesStore.setState({
      pages: new Map([["n", { stage: "creating", accepted: false }]]),
    });
    settleNewPage("n", reply, retry);
    settleNewPage("unknown", reply, retry);
    expect([...useNewPagesStore.getState().pages]).toEqual([
      [
        "n",
        expected === "failed"
          ? { stage: expected, accepted: false }
          : expected === "creating"
            ? { stage: expected, accepted: false }
            : { stage: expected },
      ],
    ]);
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
  expect([...useNewPagesStore.getState().pages]).toEqual([
    [id, { stage: "failed", accepted: false }],
  ]);
  expect(commandFailures()).toEqual([]);
});
test("TREE-1 a fresh tree forgets listed settled entries but keeps creating and failed entries", () => {
  useNewPagesStore.setState({
    pages: new Map([
      ["a", { stage: "created" }],
      ["b", { stage: "creating", accepted: false }],
      ["failed", { stage: "failed", accepted: false }],
    ]),
  });
  const pages = ["a", "b", "failed"].map((id) => ({
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
  expect([...useNewPagesStore.getState().pages]).toEqual([
    ["b", { stage: "creating", accepted: false }],
    ["failed", { stage: "failed", accepted: false }],
  ]);
});

test("TREE-1 only an uncreated empty activation can be reused", () => {
  const first = activateNewPage(null);
  expect(activateNewPage(first)).toBe(first);
  settleNewPage(first, { status: "ok", data: first }, false);
  const second = activateNewPage(first);
  expect(vi.mocked(commands.pageCreate).mock.calls).toEqual([
    [null, first],
    [null, second],
  ]);
  expect(second).not.toBe(first);
});

test("TREE-1 acceptance survives a later retry failure and prevents forgetting under stale reads", () => {
  const id = createNewPage();
  settleNewPage(
    id,
    { status: "error", error: "Unavailable", accepted: true },
    false,
  );
  retryNewPage(id);
  settleNewPage(
    id,
    { status: "error", error: { Internal: "failed retry" } },
    true,
  );
  forgetEmptyFailuresExcept("another-page");
  expect([...useNewPagesStore.getState().pages]).toEqual([
    [id, { stage: "failed", accepted: true }],
  ]);
});
