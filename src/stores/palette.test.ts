import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, type SearchReply } from "../ipc";
import { installMock } from "../ipc/mock";
import { useIndexStatusStore } from "./index-status";
import { usePageTreeStore } from "./page-tree";
import { connectPalette, usePaletteStore } from "./palette";

vi.mock("../ipc", async (importOriginal) => {
  const mock = await import("../ipc/mock");
  return {
    ...(await importOriginal<typeof import("../ipc")>()),
    commands: mock.commands,
    events: mock.events,
  };
});

type Reply = Awaited<ReturnType<typeof commands.search>>;

function deferred<T>() {
  let resolve = (_value: T): void => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const ready = (hits: string[]): Reply => ({
  status: "ok",
  data: {
    Ready: {
      hits: hits.map((page_id) => ({
        page_id,
        title: [{ text: page_id, highlighted: false }],
        block: { block_id: "b", text: [{ text: "x", highlighted: true }] },
      })),
      issues: [],
    },
  } satisfies SearchReply,
});

async function seed(pages: { id: string; title: string }[], bodies = {}) {
  window.__aelomiMock?.workspace.seed(pages);
  for (const [id, blocks] of Object.entries(bodies))
    window.__aelomiMock?.bodies.seed(id, blocks as never);
  return usePageTreeStore.getState().refresh();
}

let stop = () => {};
beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  usePaletteStore.setState(usePaletteStore.getInitialState(), true);
  usePageTreeStore.setState(usePageTreeStore.getInitialState(), true);
  useIndexStatusStore.setState(useIndexStatusStore.getInitialState(), true);
  stop = connectPalette();
});
afterEach(() => {
  stop();
  vi.restoreAllMocks();
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("a reply for an older query is dropped", async () => {
  const source = await seed([]);
  const first = deferred<Reply>();
  const second = deferred<Reply>();
  vi.spyOn(commands, "search")
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  const palette = usePaletteStore.getState();
  palette.show(null);
  palette.setQuery("roa", source);
  palette.setQuery("road", source);
  second.resolve(ready(["page-new"]));
  await vi.waitFor(() =>
    expect(usePaletteStore.getState().content.kind).toBe("ready"),
  );
  first.resolve(ready(["page-old"]));
  await new Promise((done) => setTimeout(done, 0));
  const { content } = usePaletteStore.getState();
  expect(
    content.kind === "ready" && content.hits.map((h) => h.page_id),
  ).toEqual(["page-new"]);
});

test("an Indexing reply shows Indexing even after a Ready status, and a later index change reruns the query", async () => {
  const source = await seed([{ id: "page-daily", title: "Daily" }], {
    "page-daily": [{ id: "b1", content: ["quarterly roadmap"] }],
  });
  useIndexStatusStore.getState().receiveStatus("Ready");
  const search = vi
    .spyOn(commands, "search")
    .mockResolvedValueOnce({ status: "ok", data: "Indexing" });
  const palette = usePaletteStore.getState();
  palette.show(null);
  palette.setQuery("roadm", source);
  await vi.waitFor(() =>
    expect(usePaletteStore.getState().content).toEqual({ kind: "indexing" }),
  );
  useIndexStatusStore.getState().receiveChange();
  await vi.waitFor(() =>
    expect(usePaletteStore.getState().content.kind).toBe("ready"),
  );
  expect(search).toHaveBeenCalledTimes(2);
  const { content } = usePaletteStore.getState();
  expect(
    content.kind === "ready" &&
      content.hits.map((hit) => [hit.page_id, hit.block?.text]),
  ).toEqual([
    [
      "page-daily",
      [
        { text: "quarterly ", highlighted: false },
        { text: "roadmap", highlighted: true },
      ],
    ],
  ]);
});

test("content results never repeat a page from the capped title group, which is excluded whole", async () => {
  const pages = Array.from({ length: 230 }, (_, i) => ({
    id: `page-${String(i).padStart(3, "0")}`,
    title: `Roadmap ${i}`,
  }));
  const bodies = Object.fromEntries(
    pages.map((page) => [
      page.id,
      [{ id: `${page.id}-b`, content: ["roadmap notes"] }],
    ]),
  );
  const source = await seed(pages, bodies);
  const search = vi.spyOn(commands, "search");
  const palette = usePaletteStore.getState();
  palette.show(null);
  palette.setQuery("roadmap", source);
  await vi.waitFor(() =>
    expect(usePaletteStore.getState().content.kind).toBe("ready"),
  );
  const { titles, content } = usePaletteStore.getState();
  expect(titles).toHaveLength(200);
  expect(search.mock.calls[0]?.[2]).toEqual({
    Content: { exclude: titles.map((title) => title.id) },
  });
  const contentIds =
    content.kind === "ready" ? content.hits.map((hit) => hit.page_id) : [];
  expect(contentIds).toHaveLength(20);
  const titleIds = new Set(titles.map((title) => title.id));
  expect(contentIds.filter((id) => titleIds.has(id))).toEqual([]);
  expect(contentIds.every((id) => id >= "page-200")).toBe(true);
});

test("a closed palette ignores replies and status changes", async () => {
  const source = await seed([{ id: "page-daily", title: "Daily" }]);
  const pending = deferred<Reply>();
  const search = vi.spyOn(commands, "search").mockReturnValue(pending.promise);
  const palette = usePaletteStore.getState();
  palette.show(null);
  palette.setQuery("dai", source);
  palette.hide();
  pending.resolve(ready(["page-daily"]));
  useIndexStatusStore.getState().receiveStatus("Ready");
  await new Promise((done) => setTimeout(done, 0));
  expect(search).toHaveBeenCalledOnce();
  expect(usePaletteStore.getState().content).toEqual({ kind: "idle" });
});
