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

test.each([{ Io: "Settings read denied" }, "Internal", "Quitting"] as const)(
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
    expect(report.mock.calls).toEqual([[error]]);
    await drainCommands();
    expect(update.mock.calls).toEqual([
      [{ theme: "Dark" }],
      [{ theme: "Dark" }],
    ]);
    expect(report.mock.calls).toEqual([[error], [null]]);
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
