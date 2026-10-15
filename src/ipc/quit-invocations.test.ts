import { afterEach, expect, test, vi } from "vitest";
import { connectQuitDrain } from "../editor/quit-drain";
import { useQuitStore } from "../stores/quit";
import { commands, type Settings } from ".";

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
