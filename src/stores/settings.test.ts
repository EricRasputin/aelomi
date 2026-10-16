import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { commands, installMock } from "../ipc/mock";
import {
  commandFailures,
  drainCommands,
  setCommandAdmission,
} from "../ipc/pending-commands";
import { createSettingsWriteJob } from "./settings";

beforeEach(() => {
  vi.stubGlobal("window", {});
  installMock();
  window.__aelomiMock?.reset();
  setCommandAdmission("open");
  vi.useFakeTimers();
});
afterEach(async () => {
  vi.restoreAllMocks();
  await drainCommands();
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test("SHELL-8 latest field values survive an outage with one write in flight", async () => {
  let release = () => {};
  const original = commands.settingsUpdate;
  const update = vi.spyOn(commands, "settingsUpdate");
  update.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        release = () =>
          resolve({ status: "error", error: { Io: "Settings read denied" } });
      }),
  );
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    () => {},
  );
  job({ theme: "Dark", expanded_rows: ["page-a"] });
  job({ theme: "System", recent_pages: ["page-b"] });
  job({ theme: "Light", expanded_rows: ["page-c"], recent_pages: null });
  expect(update.mock.calls).toEqual([
    [{ theme: "Dark", expanded_rows: ["page-a"] }],
  ]);
  release();
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: [], settings: true });
  update.mockImplementation(original);
  await drainCommands();
  expect(update.mock.calls).toEqual([
    [{ theme: "Dark", expanded_rows: ["page-a"] }],
    [{ theme: "Light", expanded_rows: ["page-c"], recent_pages: ["page-b"] }],
  ]);
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Light",
      open_page: null,
      recent_pages: ["page-b"],
      expanded_rows: ["page-c"],
    },
  });
});

test.each([{ Io: "Settings read denied" }] as const)(
  "SHELL-9 drain retries a retained settings failure %j without waiting for backoff",
  async (error) => {
    const update = vi
      .spyOn(commands, "settingsUpdate")
      .mockResolvedValueOnce({ status: "error", error });
    const report = vi.fn();
    const job = createSettingsWriteJob(
      (patch) => commands.settingsUpdate(patch),
      report,
    );
    job({ theme: "Dark" });
    await drainCommands();
    expect(commandFailures()).toEqual({ docs: [], settings: true });
    expect(report.mock.calls).toEqual([[{ kind: "retrying", error }]]);
    await drainCommands();
    expect(update.mock.calls).toEqual([
      [{ theme: "Dark" }],
      [{ theme: "Dark" }],
    ]);
    expect(report.mock.calls).toEqual([[{ kind: "retrying", error }], [null]]);
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: [],
      },
    });
  },
);

test.each(["InvalidPageId", "Unavailable", "Internal", "Quitting"] as const)(
  "SHELL-8 a terminal settings rejection %s settles without retry",
  async (error) => {
    const update = vi
      .spyOn(commands, "settingsUpdate")
      .mockResolvedValueOnce({ status: "error", error });
    const report = vi.fn();
    const job = createSettingsWriteJob(
      (patch) => commands.settingsUpdate(patch),
      report,
    );
    job({ expanded_rows: ["Bad ID"] });
    await drainCommands();
    expect(commandFailures()).toEqual({ docs: [], settings: false });
    expect(report.mock.calls).toEqual([[{ kind: "failed", error }]]);
    await vi.advanceTimersByTimeAsync(10_000);
    job({ theme: "Dark" });
    await drainCommands();
    expect(update.mock.calls).toEqual([
      [{ expanded_rows: ["Bad ID"] }],
      [{ theme: "Dark" }],
    ]);
    expect(await commands.settingsGet()).toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: [],
      },
    });
  },
);

test("SHELL-8 InvalidPageId drops the rejected fields and saves the pending theme", async () => {
  let release = () => {};
  const update = vi.spyOn(commands, "settingsUpdate").mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        release = () => resolve({ status: "error", error: "InvalidPageId" });
      }),
  );
  const report = vi.fn();
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    report,
  );
  job({ expanded_rows: ["Bad ID"] });
  job({ theme: "Dark" });
  release();
  await drainCommands();
  expect(report.mock.calls).toEqual([
    [{ kind: "failed", error: "InvalidPageId" }],
    [null],
  ]);
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(update.mock.calls).toEqual([
    [{ expanded_rows: ["Bad ID"] }],
    [{ theme: "Dark" }],
  ]);
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-8 InvalidPageId preserves the theme when local page-ID validation disagrees", async () => {
  const update = vi
    .spyOn(commands, "settingsUpdate")
    .mockResolvedValueOnce({ status: "error", error: "InvalidPageId" });
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    () => {},
  );
  job({
    theme: "Dark",
    expanded_rows: ["page-a"],
    recent_pages: ["page-b"],
    open_page: { Set: { page_id: "page-c", top_visible_block_id: null } },
  });
  await drainCommands();
  expect(update.mock.calls).toEqual([
    [
      {
        theme: "Dark",
        expanded_rows: ["page-a"],
        recent_pages: ["page-b"],
        open_page: { Set: { page_id: "page-c", top_visible_block_id: null } },
      },
    ],
    [{ theme: "Dark" }],
  ]);
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-9 an expanded-rows patch queued with closed admission is reported and retried on reopening", async () => {
  const update = vi.spyOn(commands, "settingsUpdate");
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    () => {},
  );
  setCommandAdmission("completing");
  job({ expanded_rows: ["page-parent"] });
  await drainCommands();
  const waiting = commandFailures();
  const attempts = update.mock.calls.length;
  setCommandAdmission("open");
  await drainCommands();
  expect({ waiting, attempts }).toEqual({
    waiting: { docs: [], settings: true },
    attempts: 0,
  });
  expect(update.mock.calls).toEqual([[{ expanded_rows: ["page-parent"] }]]);
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "System",
      open_page: null,
      recent_pages: [],
      expanded_rows: ["page-parent"],
    },
  });
});

test("SHELL-10 closed admission preserves the Io reason and reopening retries immediately", async () => {
  const update = vi
    .spyOn(commands, "settingsUpdate")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read denied" } });
  const report = vi.fn();
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    report,
  );
  job({ theme: "Dark" });
  await drainCommands();
  setCommandAdmission("completing");
  await vi.advanceTimersByTimeAsync(20_000);
  expect(update.mock.calls).toEqual([[{ theme: "Dark" }]]);
  expect(report.mock.calls).toEqual([
    [{ kind: "retrying", error: { Io: "read denied" } }],
  ]);
  setCommandAdmission("open");
  await drainCommands();
  expect(update.mock.calls).toEqual([[{ theme: "Dark" }], [{ theme: "Dark" }]]);
  expect(report.mock.calls).toEqual([
    [{ kind: "retrying", error: { Io: "read denied" } }],
    [null],
  ]);
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-8 a merged invalid field cannot discard valid settings after a retained outage", async () => {
  const update = vi
    .spyOn(commands, "settingsUpdate")
    .mockResolvedValueOnce({ status: "error", error: { Io: "read denied" } });
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    () => {},
  );
  job({ expanded_rows: ["Bad ID"] });
  await drainCommands();
  job({ theme: "Dark", recent_pages: ["page-a"] });
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(update.mock.calls).toEqual([
    [{ expanded_rows: ["Bad ID"] }],
    [{ expanded_rows: ["Bad ID"], theme: "Dark", recent_pages: ["page-a"] }],
    [{ theme: "Dark", recent_pages: ["page-a"] }],
  ]);
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: ["page-a"],
      expanded_rows: [],
    },
  });
});

test("SHELL-10 a retained transport failure survives a quit refusal until storage accepts it", async () => {
  const update = vi
    .spyOn(commands, "settingsUpdate")
    .mockRejectedValueOnce(new Error("invoke disconnected"))
    .mockResolvedValueOnce({ status: "error", error: "Quitting" });
  const report = vi.fn();
  const job = createSettingsWriteJob(
    (patch) => commands.settingsUpdate(patch),
    report,
  );
  job({ theme: "Dark" });
  await drainCommands();
  await drainCommands();
  expect(commandFailures()).toEqual({ docs: [], settings: true });
  expect(report.mock.calls).toEqual([
    [{ kind: "retrying", error: { Transport: "invoke disconnected" } }],
  ]);
  await drainCommands();
  expect(update.mock.calls).toEqual([
    [{ theme: "Dark" }],
    [{ theme: "Dark" }],
    [{ theme: "Dark" }],
  ]);
  expect(commandFailures()).toEqual({ docs: [], settings: false });
  expect(await commands.settingsGet()).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});
