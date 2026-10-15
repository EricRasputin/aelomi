import { expect, test } from "vitest";
import { defaultSettings, patchSettings, seedSettings } from "./mock-settings";

test("DATA-4 merges typed patches and keeps unrelated session fields", () => {
  const initial = patchSettings(defaultSettings(), {
    theme: "Dark",
    open_page: {
      Set: { page_id: "page-a", top_visible_block_id: "block-7" },
    },
    recent_pages: ["page-b", "page-a", "page-b"],
    expanded_rows: ["page-b", "page-a", "page-b"],
  });
  expect(initial).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: { page_id: "page-a", top_visible_block_id: "block-7" },
      recent_pages: ["page-b", "page-a"],
      expanded_rows: ["page-a", "page-b"],
    },
  });
  if (initial.status === "error") throw new Error(initial.error.toString());
  expect(patchSettings(initial.data, { theme: "Light" })).toEqual({
    status: "ok",
    data: { ...initial.data, theme: "Light" },
  });
  expect(patchSettings(initial.data, { open_page: "Clear" })).toEqual({
    status: "ok",
    data: { ...initial.data, open_page: null },
  });
});

test("DATA-4 recent pages retain newest-first input order and the first fifty distinct IDs", () => {
  const pages = Array.from({ length: 60 }, (_, i) => `page-${i}`);
  const result = patchSettings(defaultSettings(), {
    recent_pages: pages.flatMap((id) => [id, id]),
  });
  expect(result).toEqual({
    status: "ok",
    data: { ...defaultSettings(), recent_pages: pages.slice(0, 50) },
  });
});

test("DATA-4 refuses invalid page IDs before applying any patch", () => {
  const initial = defaultSettings();
  for (const patch of [
    { theme: "Dark", recent_pages: ["workspace"] },
    { expanded_rows: ["../page"] },
    { open_page: { Set: { page_id: "", top_visible_block_id: null } } },
  ] satisfies Parameters<typeof patchSettings>[1][]) {
    expect(patchSettings(initial, patch)).toEqual({
      status: "error",
      error: "InvalidPageId",
    });
  }
  expect(initial).toEqual({
    theme: "System",
    open_page: null,
    recent_pages: [],
    expanded_rows: [],
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
        open_page: null,
        recent_pages: [],
        expanded_rows: [],
      },
    });
    await Promise.all([
      commands.settingsUpdate({ theme: "Light" }),
      commands.settingsUpdate({ expanded_rows: ["page-a"] }),
    ]);
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Light",
        open_page: null,
        recent_pages: [],
        expanded_rows: ["page-a"],
      },
    });
    window.__aelomiMock?.storage.failWrites(true);
    expect(await commands.settingsUpdate({ theme: "Dark" })).toEqual({
      status: "error",
      error: { Write: { Io: "mock write failure" } },
    });
    expect(await commands.storageHealth()).toEqual({
      Failing: {
        reason: { Io: "mock write failure" },
        pending_docs: [],
        pending_settings: true,
      },
    });
    window.__aelomiMock?.storage.failWrites(false);
    expect(await commands.storageHealth()).toEqual("Healthy");
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: ["page-a"],
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

test("DATA-4 seeded settings normalize IDs, sets, and the recent-page cap", () => {
  const pages = Array.from({ length: 60 }, (_, i) => `page-${i}`);
  expect(
    seedSettings({
      theme: "Dark",
      open_page: { page_id: "workspace", top_visible_block_id: null },
      recent_pages: ["workspace", ...pages, ...pages],
      expanded_rows: ["page-b", "workspace", "page-a", "page-b"],
    }),
  ).toEqual({
    theme: "Dark",
    open_page: null,
    recent_pages: pages.slice(0, 50),
    expanded_rows: ["page-a", "page-b"],
  });
});

test("SHELL-10 a failed frontend drain retains both refused documents and settings", async () => {
  const { vi } = await import("vitest");
  vi.stubGlobal("window", {});
  const { installMock } = await import("./mock");
  const { commands } = installMock();
  try {
    await commands.frontendReady();
    window.__aelomiMock?.storage.failWrites(true);
    await commands.settingsUpdate({ theme: "Dark" });
    window.__aelomiMock?.quit.request();
    expect(
      await commands.quitDrainFailed(1, ["page-b", "page-a", "page-b"]),
    ).toEqual({
      status: "ok",
      data: {
        revision: 2,
        phase: {
          Asking: {
            pending_docs: ["page-a", "page-b"],
            pending_settings: true,
          },
        },
      },
    });
    expect(window.__aelomiMock?.quit.exited).toBe(false);
  } finally {
    window.__aelomiMock?.reset();
    vi.unstubAllGlobals();
  }
});
