import { expect, test } from "vitest";
import { defaultSettings, patchSettings } from "./mock-settings";

test("DATA-4 merges typed patches and keeps unrelated session fields", () => {
  const initial = patchSettings(defaultSettings(), {
    theme: "Dark",
    openPage: {
      Set: { pageId: "page-a", topVisibleBlockId: "block-7" },
    },
    recentPages: ["page-b", "page-a", "page-b"],
    expandedRows: ["page-b", "page-a", "page-b"],
  });
  expect(initial).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      openPage: { pageId: "page-a", topVisibleBlockId: "block-7" },
      recentPages: ["page-b", "page-a"],
      expandedRows: ["page-a", "page-b"],
    },
  });
  if (initial.status === "error") throw new Error(initial.error.toString());
  expect(patchSettings(initial.data, { theme: "Light" })).toEqual({
    status: "ok",
    data: { ...initial.data, theme: "Light" },
  });
  expect(patchSettings(initial.data, { openPage: "Clear" })).toEqual({
    status: "ok",
    data: { ...initial.data, openPage: null },
  });
});

test("DATA-4 recent pages retain newest-first input order and the first fifty distinct IDs", () => {
  const pages = Array.from({ length: 60 }, (_, i) => `page-${i}`);
  const result = patchSettings(defaultSettings(), {
    recentPages: pages.flatMap((id) => [id, id]),
  });
  expect(result).toEqual({
    status: "ok",
    data: { ...defaultSettings(), recentPages: pages.slice(0, 50) },
  });
});

test("DATA-4 refuses invalid page IDs before applying any patch", () => {
  const initial = defaultSettings();
  for (const patch of [
    { theme: "Dark", recentPages: ["workspace"] },
    { expandedRows: ["../page"] },
    { openPage: { Set: { pageId: "", topVisibleBlockId: null } } },
  ] satisfies Parameters<typeof patchSettings>[1][]) {
    expect(patchSettings(initial, patch)).toEqual({
      status: "error",
      error: "InvalidPageId",
    });
  }
  expect(initial).toEqual({
    theme: "System",
    openPage: null,
    recentPages: [],
    expandedRows: [],
  });
});

test("DATA-4 mock boot seeds settings and concurrent command patches preserve each other", async () => {
  const { Window } = await import("happy-dom");
  const { vi } = await import("vitest");
  const browser = new Window();
  vi.stubGlobal("window", browser);
  try {
    window.__aelomiMockSetup = { settings: { theme: "Dark" } };
    const { installMock } = await import("./mock");
    const { commands } = installMock();
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        openPage: null,
        recentPages: [],
        expandedRows: [],
      },
    });
    await Promise.all([
      commands.settingsUpdate({ theme: "Light" }),
      commands.settingsUpdate({ expandedRows: ["page-a"] }),
    ]);
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Light",
        openPage: null,
        recentPages: [],
        expandedRows: ["page-a"],
      },
    });
    window.__aelomiMock?.storage.setStatus("Locked");
    expect(await commands.settingsGet()).toEqual({
      status: "error",
      error: "Unavailable",
    });
    expect(await commands.settingsUpdate({ theme: "System" })).toEqual({
      status: "error",
      error: "Unavailable",
    });
    window.__aelomiMock?.reset();
  } finally {
    vi.unstubAllGlobals();
    await browser.happyDOM.close();
  }
});
