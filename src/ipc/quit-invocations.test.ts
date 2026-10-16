import { afterEach, expect, test, vi } from "vitest";
import { deliver, drainDeliveries } from "../editor/delivery";
import { connectQuitDrain } from "../editor/quit-drain";
import {
  createNewPage,
  forgetNewPage,
  useNewPagesStore,
} from "../shell/new-pages";
import { usePageTitlesStore } from "../stores/page-titles";
import { useQuitStore } from "../stores/quit";
import { commands, type PageError, type Settings } from ".";
import { commandFailures, drainCommands } from "./pending-commands";

const transport = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
}));
vi.mock("@tauri-apps/api/core", () => transport);

function dispatch() {
  let admit = (_settings: Settings) => {};
  const promise = new Promise<Settings>((done) => {
    admit = done;
  });
  return { promise, admit };
}

const snapshot: Settings = {
  theme: "Dark",
  open_page: null,
  recent_pages: [],
  expanded_rows: [],
};

afterEach(async () => {
  transport.invoke.mockImplementation((name) =>
    Promise.resolve(name === "storage_flush" ? true : null),
  );
  await drainCommands();
  for (const id of useNewPagesStore.getState().pages.keys()) forgetNewPage(id);
  usePageTitlesStore.setState({ drafts: new Map() });
  vi.useRealTimers();
  transport.invoke.mockReset();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
});

function failingRecords(error: PageError = { Io: "read failed" }) {
  let failing = true;
  let title = "Before";
  transport.invoke.mockImplementation((command, args) => {
    if (command === "storage_flush") return Promise.resolve(true);
    if (command === "keep_open")
      return Promise.resolve({ revision: 3, phase: "Open" });
    if (command === "page_create") {
      if (failing) return Promise.reject(error);
      return Promise.resolve(
        typeof args === "object" && args !== null && "pageId" in args
          ? args.pageId
          : null,
      );
    }
    if (command === "page_rename") {
      if (failing) return Promise.reject(error);
      if (typeof args === "object" && args !== null && "title" in args)
        title = String(args.title);
      return Promise.resolve(null);
    }
    if (typeof args === "object" && args !== null && "revision" in args) {
      const revision = Number(args.revision) + 1;
      if (command === "quit_drain_failed")
        return Promise.resolve({
          revision,
          phase: {
            Asking: { pending_docs: ["workspace"], pending_settings: false },
          },
        });
      if (command === "complete_quit")
        return Promise.resolve({ revision, phase: "Confirmed" });
    }
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
  return {
    recover: () => {
      failing = false;
    },
    title: () => title,
  };
}

test("SHELL-10 a record failure during the drain's initial yield is reported", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  const stop = connectQuitDrain();
  try {
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    void commands.pageRename("page-a", "During yield");
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      pendingDocs: ["workspace"],
    });
    expect(
      transport.invoke.mock.calls.some(([name]) => name === "complete_quit"),
    ).toBe(false);
  } finally {
    stop();
    storage.recover();
  }
});

test("SHELL-10 a record failure after command collection while deliveries are held is reported", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  const invoke = transport.invoke.getMockImplementation();
  let release = (_seq: number) => {};
  const document = new Promise<number>((done) => {
    release = done;
  });
  transport.invoke.mockImplementation((name, args) =>
    name === "doc_append"
      ? document
      : (invoke?.(name, args) ?? Promise.resolve(null)),
  );
  const stop = connectQuitDrain();
  try {
    deliver("late-record-page", new Uint8Array([1]));
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    await drainCommands();
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    void commands.pageRename("page-a", "While held");
    await vi.advanceTimersByTimeAsync(0);
    release(1);
    await drainDeliveries();
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      pendingDocs: ["workspace"],
    });
    expect(
      transport.invoke.mock.calls.some(([name]) => name === "complete_quit"),
    ).toBe(false);
  } finally {
    release(1);
    stop();
    storage.recover();
  }
});

test("SHELL-10 a second normal quit retains a refused rename until recovery saves it", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  const stop = connectQuitDrain();
  try {
    void commands.pageRename("page-a", "Recovered title");
    await vi.advanceTimersByTimeAsync(0);
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision.kind).toBe("asking");
    useQuitStore.getState().receiveQuit({ revision: 3, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      revision: 4,
      pendingDocs: ["workspace"],
    });
    expect(storage.title()).toBe("Before");
    storage.recover();
    await vi.advanceTimersByTimeAsync(5000);
    expect(storage.title()).toBe("Recovered title");
    useQuitStore.getState().receiveQuit({ revision: 5, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "quitting",
      revision: 6,
      origin: "drained",
    });
    expect(
      transport.invoke.mock.calls.filter(([name]) => name === "complete_quit"),
    ).toEqual([["complete_quit", { revision: 5 }]]);
  } finally {
    stop();
    storage.recover();
  }
});

test("SHELL-10 a drain superseded by the five second timer retains a failure that settles afterward", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  const invoke = transport.invoke.getMockImplementation();
  let reject = (_error: unknown) => {};
  const held = new Promise<unknown>((_done, fail) => {
    reject = fail;
  });
  transport.invoke.mockImplementationOnce(() => held);
  const stop = connectQuitDrain();
  try {
    const rename = commands.pageRename("page-a", "After timeout");
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    setTimeout(
      () =>
        useQuitStore.getState().receiveQuit({
          revision: 2,
          phase: { Asking: { pending_docs: [], pending_settings: false } },
        }),
      5000,
    );
    await vi.advanceTimersByTimeAsync(5000);
    expect(useQuitStore.getState().decision.kind).toBe("asking");
    reject({ Io: "late failure" });
    void rename;
    await vi.advanceTimersByTimeAsync(0);
    useQuitStore.getState().receiveQuit({ revision: 3, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      revision: 4,
      pendingDocs: ["workspace"],
    });
    expect(
      transport.invoke.mock.calls.some(([name]) => name === "complete_quit"),
    ).toBe(false);
  } finally {
    reject({ Io: "late failure" });
    stop();
    storage.recover();
    if (invoke) transport.invoke.mockImplementation(invoke);
  }
});

test("SHELL-9 the real settings command boundary drains invokes held before native admission", async () => {
  const first = dispatch();
  const second = dispatch();
  const waiting = [first.promise, second.promise];
  transport.invoke.mockImplementation((command) => {
    if (command === "settings_update")
      return waiting.shift() ?? Promise.reject(new Error("unexpected update"));
    if (command === "complete_quit")
      return Promise.resolve({ revision: 2, phase: "Confirmed" });
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
  const stop = connectQuitDrain();
  try {
    const a = commands.settingsUpdate({ theme: "Dark" });
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await new Promise((done) => setTimeout(done, 10));
    const beforeAdmission = transport.invoke.mock.calls.map(([name]) => name);
    const b = commands.settingsUpdate({ recent_pages: ["page-b"] });
    first.admit(snapshot);
    await a;
    await new Promise((done) => setTimeout(done, 10));
    const beforeSecondAdmission = transport.invoke.mock.calls.map(
      ([name]) => name,
    );
    second.admit({ ...snapshot, recent_pages: ["page-b"] });
    await b;
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("quitting"),
    );
    expect(beforeAdmission).toEqual(["settings_update"]);
    expect(beforeSecondAdmission).toEqual([
      "settings_update",
      "settings_update",
    ]);
    expect(transport.invoke.mock.calls).toEqual([
      ["settings_update", { patch: { theme: "Dark" } }],
      ["settings_update", { patch: { recent_pages: ["page-b"] } }],
      ["complete_quit", { revision: 1 }],
    ]);
    expect(await b).toEqual({
      status: "ok",
      data: { ...snapshot, recent_pages: ["page-b"] },
    });
  } finally {
    first.admit(snapshot);
    second.admit(snapshot);
    stop();
  }
});

test("SHELL-9 quit awaits a settings invoke started after the command drain while document delivery is held", async () => {
  const settings = dispatch();
  let acceptDocument = (_seq: number) => {};
  const document = new Promise<number>((done) => {
    acceptDocument = done;
  });
  transport.invoke.mockImplementation((command) => {
    if (command === "doc_append") return document;
    if (command === "settings_update") return settings.promise;
    if (command === "complete_quit")
      return Promise.resolve({ revision: 2, phase: "Confirmed" });
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
  const stop = connectQuitDrain();
  try {
    deliver("held-page", new Uint8Array([1, 2, 3]));
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await new Promise((done) => setTimeout(done, 10));
    await drainCommands();
    const update = commands.settingsUpdate({ theme: "Dark" });
    acceptDocument(1);
    await drainDeliveries();
    await new Promise((done) => setTimeout(done, 10));
    const beforeAdmission = transport.invoke.mock.calls.map(([name]) => name);
    settings.admit(snapshot);
    expect(await update).toEqual({ status: "ok", data: snapshot });
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("quitting"),
    );
    expect(beforeAdmission).toEqual(["doc_append", "settings_update"]);
    expect(transport.invoke.mock.calls.map(([name]) => name)).toEqual([
      "doc_append",
      "settings_update",
      "complete_quit",
    ]);
  } finally {
    acceptDocument(1);
    settings.admit(snapshot);
    stop();
  }
});

test("SHELL-9 settings writes are refused with Quitting once completion starts and resume after Keep open", async () => {
  let finishQuit = (_status: unknown) => {};
  const completion = new Promise<unknown>((done) => {
    finishQuit = done;
  });
  transport.invoke.mockImplementation((command) => {
    if (command === "complete_quit") return completion;
    if (command === "settings_update") return Promise.resolve(snapshot);
    if (command === "keep_open")
      return Promise.resolve({ revision: 3, phase: "Open" });
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
  const stop = connectQuitDrain();
  try {
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.waitFor(() =>
      expect(transport.invoke.mock.calls).toEqual([
        ["complete_quit", { revision: 1 }],
      ]),
    );
    const refused = await commands.settingsUpdate({ theme: "Dark" });
    const beforeKeepOpen = transport.invoke.mock.calls.map(([name]) => name);
    finishQuit({
      revision: 2,
      phase: { Asking: { pending_docs: [], pending_settings: true } },
    });
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("asking"),
    );
    await useQuitStore.getState().keepOpen();
    expect(await commands.settingsUpdate({ theme: "Dark" })).toEqual({
      status: "ok",
      data: snapshot,
    });
    expect(refused).toEqual({ status: "error", error: "Quitting" });
    expect(beforeKeepOpen).toEqual(["complete_quit"]);
    expect(transport.invoke.mock.calls.map(([name]) => name)).toEqual([
      "complete_quit",
      "keep_open",
      "settings_update",
    ]);
  } finally {
    finishQuit({ revision: 3, phase: "Open" });
    stop();
  }
});

test("SHELL-9 one command drain waits for both a rename and settings before both land at quit", async () => {
  let acceptRename = () => {};
  let acceptSettings = () => {};
  let title = "Before";
  let settings: Settings = { ...snapshot, theme: "System" };
  transport.invoke.mockImplementation((command) => {
    if (command === "page_rename")
      return new Promise((done) => {
        acceptRename = () => {
          title = "At quit";
          done(null);
        };
      });
    if (command === "settings_update")
      return new Promise((done) => {
        acceptSettings = () => {
          settings = snapshot;
          done(settings);
        };
      });
    if (command === "complete_quit")
      return Promise.resolve({ revision: 2, phase: "Confirmed" });
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
  const stop = connectQuitDrain();
  try {
    const rename = commands.pageRename("page-a", "At quit");
    const update = commands.settingsUpdate({ theme: "Dark" });
    let drained = false;
    const drain = drainCommands().then((failed) => {
      drained = true;
      return failed;
    });
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    acceptSettings();
    expect(await update).toEqual({ status: "ok", data: snapshot });
    await new Promise((done) => setTimeout(done, 10));
    expect(drained).toBe(false);
    expect(transport.invoke.mock.calls.map(([name]) => name)).toEqual([
      "page_rename",
      "settings_update",
    ]);
    acceptRename();
    expect(await rename).toEqual({ status: "ok", data: null });
    await drain;
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("quitting"),
    );
    expect({ title, settings }).toEqual({
      title: "At quit",
      settings: snapshot,
    });
    expect(transport.invoke.mock.calls.map(([name]) => name)).toEqual([
      "page_rename",
      "settings_update",
      "complete_quit",
    ]);
  } finally {
    acceptRename();
    acceptSettings();
    stop();
  }
});

test("SHELL-9 a new drain after Asking admits both rename and settings before completion", async () => {
  let first = true;
  const settings = dispatch();
  transport.invoke.mockImplementation((name) => {
    if (name === "complete_quit") {
      if (first) {
        first = false;
        return Promise.resolve({
          revision: 2,
          phase: {
            Asking: { pending_docs: ["workspace"], pending_settings: true },
          },
        });
      }
      return Promise.resolve({ revision: 4, phase: "Confirmed" });
    }
    if (name === "page_rename") return Promise.resolve(null);
    if (name === "settings_update") return settings.promise;
    return Promise.reject(new Error(`unexpected command ${name}`));
  });
  const stop = connectQuitDrain();
  try {
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("asking"),
    );
    useQuitStore.getState().receiveQuit({ revision: 3, phase: "Draining" });
    const rename = commands.pageRename("page-a", "New drain");
    const update = commands.settingsUpdate({ theme: "Dark" });
    expect(await rename).toEqual({ status: "ok", data: null });
    await new Promise((done) => setTimeout(done, 10));
    expect(useQuitStore.getState().decision.kind).toBe("draining");
    settings.admit(snapshot);
    expect(await update).toEqual({ status: "ok", data: snapshot });
    await vi.waitFor(() =>
      expect(useQuitStore.getState().decision.kind).toBe("quitting"),
    );
    expect(transport.invoke.mock.calls).toEqual([
      ["complete_quit", { revision: 1 }],
      ["page_rename", { pageId: "page-a", title: "New drain" }],
      ["settings_update", { patch: { theme: "Dark" } }],
      ["complete_quit", { revision: 3 }],
    ]);
  } finally {
    settings.admit(snapshot);
    stop();
  }
});

test("SHELL-10 Keep open preserves a failed head and queued edit until both recover in order", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  const invoke = transport.invoke.getMockImplementation();
  const titles: string[] = [];
  transport.invoke.mockImplementation((name, args) => {
    if (name === "keep_open")
      return Promise.resolve({ revision: 3, phase: "Open" });
    if (
      name === "page_rename" &&
      typeof args === "object" &&
      args !== null &&
      "title" in args
    )
      titles.push(String(args.title));
    return invoke?.(name, args) ?? Promise.resolve(null);
  });
  const stop = connectQuitDrain();
  try {
    void commands.pageRename("page-a", "First");
    await vi.advanceTimersByTimeAsync(0);
    const next = commands.pageRename("page-a", "Second");
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "asking",
      pendingDocs: ["workspace"],
    });
    await useQuitStore.getState().keepOpen();
    expect(useQuitStore.getState().decision.kind).toBe("open");
    expect(storage.title()).toBe("Before");
    storage.recover();
    await vi.advanceTimersByTimeAsync(5000);
    expect(await next).toEqual({ status: "ok", data: null });
    expect(titles).toEqual(["First", "First", "First", "First", "Second"]);
    expect(storage.title()).toBe("Second");
    useQuitStore.getState().receiveQuit({ revision: 4, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(useQuitStore.getState().decision).toMatchObject({
      kind: "quitting",
      origin: "drained",
    });
  } finally {
    stop();
    storage.recover();
  }
});

test("SHELL-10 a retained new page asks immediately and saves its exact title after Keep open", async () => {
  vi.useFakeTimers();
  const storage = failingRecords();
  useQuitStore.setState({
    registered: true,
    decision: { kind: "open", revision: 0 },
  });
  const stop = connectQuitDrain();
  try {
    const id = createNewPage();
    expect(usePageTitlesStore.getState().rename(id, "Q3")).toBe(null);
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(
      transport.invoke.mock.calls.filter(
        ([name]) => name === "quit_drain_failed",
      ),
    ).toEqual([["quit_drain_failed", { revision: 1, docIds: ["workspace"] }]]);
    expect(
      transport.invoke.mock.calls.filter(([name]) => name === "complete_quit"),
    ).toEqual([]);
    expect(useQuitStore.getState().decision.kind).toBe("asking");
    storage.recover();
    await useQuitStore.getState().keepOpen();
    await vi.advanceTimersByTimeAsync(5000);
    const calls = transport.invoke.mock.calls.filter(
      ([name]) => name === "page_create" || name === "page_rename",
    );
    expect(calls).toEqual([
      ["page_create", { parentId: null, pageId: id }],
      ["page_create", { parentId: null, pageId: id }],
      ["page_create", { parentId: null, pageId: id }],
      ["page_rename", { pageId: id, title: "Q3" }],
    ]);
    expect(storage.title()).toBe("Q3");
  } finally {
    stop();
    storage.recover();
  }
});

test("SHELL-10 a terminal new-page failure retains its exact title for the quit drain", async () => {
  vi.useFakeTimers();
  const storage = failingRecords({ Internal: "terminal" });
  useQuitStore.setState({
    registered: true,
    decision: { kind: "open", revision: 0 },
  });
  const stop = connectQuitDrain();
  try {
    const id = createNewPage();
    expect(usePageTitlesStore.getState().rename(id, "Terminal Q3")).toBe(null);
    useQuitStore.getState().receiveQuit({ revision: 1, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(
      transport.invoke.mock.calls.filter(
        ([name]) => name === "quit_drain_failed",
      ),
    ).toEqual([["quit_drain_failed", { revision: 1, docIds: ["workspace"] }]]);
    expect(usePageTitlesStore.getState().drafts.get(id)?.title).toBe(
      "Terminal Q3",
    );
    expect(useQuitStore.getState().decision.kind).toBe("asking");
    storage.recover();
    await useQuitStore.getState().keepOpen();
    useQuitStore.getState().receiveQuit({ revision: 4, phase: "Draining" });
    await vi.advanceTimersByTimeAsync(0);
    expect(storage.title()).toBe("Terminal Q3");
  } finally {
    stop();
    storage.recover();
  }
});
