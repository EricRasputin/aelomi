// @vitest-environment happy-dom
import { afterEach, expect, test, vi } from "vitest";
import { commands } from "../ipc";
import { useQuitStore } from "./quit";
import { useShellStore } from "./shell";
import { useTreeExpansionStore } from "./tree-expansion";

afterEach(() => {
  vi.restoreAllMocks();
  useQuitStore.setState(useQuitStore.getInitialState(), true);
  useShellStore.setState(useShellStore.getInitialState(), true);
});

test("TREE-6 expansion loads persisted IDs and writes the next set without touching other settings", async () => {
  vi.spyOn(commands, "settingsGet").mockResolvedValue({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: ["parent"],
    },
  });
  const patches: unknown[] = [];
  vi.spyOn(commands, "settingsUpdate").mockImplementation(async (patch) => {
    patches.push(patch);
    return {
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: patch.expanded_rows ?? [],
      },
    };
  });
  await useTreeExpansionStore.getState().load();
  await useTreeExpansionStore.getState().toggle("parent");
  await useTreeExpansionStore.getState().toggle("child");
  expect(patches).toEqual([
    { expanded_rows: [] },
    { expanded_rows: ["child"] },
  ]);
  expect(useTreeExpansionStore.getState().expansion).toEqual({
    kind: "ready",
    rows: new Set(["child"]),
  });
});

test("TREE-6 a disclosure change during Draining is refused with feedback", async () => {
  const patches: unknown[] = [];
  vi.spyOn(commands, "settingsUpdate").mockImplementation(async (patch) => {
    patches.push(patch);
    return {
      status: "ok",
      data: {
        theme: "System",
        open_page: null,
        recent_pages: [],
        expanded_rows: patch.expanded_rows ?? [],
      },
    };
  });
  useTreeExpansionStore.setState({
    expansion: { kind: "ready", rows: new Set(["parent"]) },
  });
  useQuitStore.setState({ decision: { kind: "draining", revision: 1 } });
  await useTreeExpansionStore.getState().toggle("parent");
  expect([
    patches,
    useShellStore.getState().message,
    useTreeExpansionStore.getState().expansion,
  ]).toEqual([
    [],
    "Aelomi is closing. Keep the app open to change the sidebar.",
    { kind: "ready", rows: new Set(["parent"]) },
  ]);
  useQuitStore.setState({ decision: { kind: "open", revision: 2 } });
  await useTreeExpansionStore.getState().toggle("parent");
  expect(patches).toEqual([{ expanded_rows: [] }]);
});
