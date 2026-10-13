import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { IndexStatus } from "./bindings";
import { commands, events, installMock } from "./mock";

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  window.__aelomiMock?.reset();
});

afterEach(() => {
  window.__aelomiMock?.reset();
  vi.unstubAllGlobals();
});

test("rebuilding keeps documents available and publishes ready", async () => {
  const seen: IndexStatus[] = [];
  const unlisten = await events.indexStatusChanged.listen((event) => {
    seen.push(event.payload);
  });
  try {
    window.__aelomiMock?.index.setStatus("Rebuilding");
    expect(await commands.indexStatus()).toBe("Rebuilding");
    expect(await commands.search("still", 20)).toEqual({
      status: "ok",
      data: "Indexing",
    });
    expect(await commands.backlinks("page")).toEqual({
      status: "ok",
      data: "Indexing",
    });
    expect(
      await window.__aelomiMock?.storage.type("page", "still editable"),
    ).toEqual({
      status: "ok",
      data: 1,
    });
    expect(window.__aelomiMock?.storage.text("page")).toBe("still editable");
    window.__aelomiMock?.index.setStatus("Ready");
    expect(await commands.indexStatus()).toBe("Ready");
    expect(await commands.search("still", 20)).toEqual({
      status: "ok",
      data: { Ready: [] },
    });
    expect(seen).toEqual(["Rebuilding", "Ready"]);
  } finally {
    unlisten();
  }
});

test("failed status preserves its reason and reset restores ready", async () => {
  const failed: IndexStatus = { Failed: { reason: "read-only index folder" } };
  window.__aelomiMock?.index.setStatus(failed);
  expect(await commands.indexStatus()).toEqual(failed);
  expect(await commands.search("anything", 20)).toEqual({
    status: "error",
    error: { Failed: { reason: "read-only index folder" } },
  });
  expect(await commands.pageTree()).toEqual({
    status: "ok",
    data: { pages: [], favorites: [], trash: [] },
  });
  failed.Failed.reason = "changed caller value";
  expect(await commands.indexStatus()).toEqual({
    Failed: { reason: "read-only index folder" },
  });
  window.__aelomiMock?.reset();
  expect(await commands.indexStatus()).toBe("Ready");
});

test("backlinks rejects the reserved workspace ID while indexing", async () => {
  window.__aelomiMock?.index.setStatus("Rebuilding");
  expect(await commands.backlinks("workspace")).toEqual({
    status: "error",
    error: "InvalidPageId",
  });
});

test("unopened storage reports the same index failure as Rust", async () => {
  window.__aelomiMock?.storage.setStatus("Locked");
  expect(await commands.indexStatus()).toEqual({
    Failed: { reason: "the data folder is not open" },
  });
  expect(await commands.search("word", 20)).toEqual({
    status: "error",
    error: "Unavailable",
  });
  expect(await commands.backlinks("workspace")).toEqual({
    status: "error",
    error: "InvalidPageId",
  });
});
