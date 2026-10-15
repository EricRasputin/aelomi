import { afterEach, expect, test, vi } from "vitest";
import { deliver, drainDeliveries } from "../editor/delivery";
import { connectQuitDrain } from "../editor/quit-drain";
import { useQuitStore } from "../stores/quit";
import { commands, type Settings } from ".";
import { drainCommands } from "./pending-commands";

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

afterEach(() => {
  transport.invoke.mockReset();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
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
