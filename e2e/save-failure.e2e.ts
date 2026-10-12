import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-10 save failure shows why and clears after accepting every write", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await app.open("/");
  const appended = await browser.evaluate(async () => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.failWrites(true, "DiskFull");
    const result = await mock.storage.type("page", "x");
    if (result.status === "error")
      throw new Error(JSON.stringify(result.error));
    return result.data;
  });
  expect(appended).toBe(1);
  await expect(screen.getByRole("alert")).toContainText(
    "Changes aren't being saved.",
    { timeout: 2_000 },
  );
  await expect(screen.getByRole("alert")).toContainText("The disk is full.");
  await expect(screen.getByRole("alert")).toContainText(
    "Your changes are kept in memory. Aelomi will keep trying.",
  );
  await browser.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(false);
    return null;
  });
  await expect(screen.getByRole("alert")).not.toBeVisible();
  expect(
    await browser.evaluate(
      () => window.__aelomiMock?.storage.text("page") ?? null,
    ),
  ).toBe("x");
  expect(nonLocalRequests).toEqual([]);
});

test("SHELL-10 keeping the app open and Escape preserve unsaved changes", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await app.open("/");
  await browser.evaluate(async () => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.failWrites(true);
    await mock.storage.type("page", "x");
    mock.quit.request();
    return null;
  });
  const dialog = screen.getByRole("alertdialog");
  await expect(dialog).toContainText(
    "If you quit now, unsaved changes will be lost.",
  );
  await expect(screen.getByRole("button", "Keep app open")).toBeFocused();
  await screen.getByRole("button", "Keep app open").click();
  await expect(dialog).not.toBeVisible();
  expect(
    await browser.evaluate(() => window.__aelomiMock?.quit.exited ?? null),
  ).toBe(false);
  await browser.evaluate(() => {
    window.__aelomiMock?.quit.request();
    return null;
  });
  await expect(dialog).toBeVisible();
  await browser.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  expect(
    await browser.evaluate(() => ({
      exited: window.__aelomiMock?.quit.exited ?? null,
      text: window.__aelomiMock?.storage.text("page") ?? null,
    })),
  ).toEqual({ exited: false, text: "x" });
  await expect(screen.getByRole("alert")).toBeVisible();
  expect(nonLocalRequests).toEqual([]);
});

test("SHELL-10 confirming quit exits once and keeps pending updates in memory", async ({
  app,
  browser,
  screen,
  nonLocalRequests,
}) => {
  await app.open("/");
  await browser.evaluate(async () => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.failWrites(true);
    await mock.storage.type("page", "x");
    mock.quit.request();
    mock.quit.request();
    return null;
  });
  const dialog = screen.getByRole("alertdialog");
  await expect(dialog).toContainText("Quit with unsaved changes?");
  await expect(dialog).toContainText(
    "If you quit now, unsaved changes will be lost.",
  );
  await expect(screen.getByRole("button", "Keep app open")).toBeFocused();
  await screen.getByRole("button", "Quit anyway").click();
  await expect(screen.getByRole("button", "Keep app open")).toBeDisabled();
  await expect(screen.getByRole("button", "Quit anyway")).toBeDisabled();
  await browser.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(
    await browser.evaluate(() => ({
      exited: window.__aelomiMock?.quit.exited ?? null,
      text: window.__aelomiMock?.storage.text("page") ?? null,
      confirmations:
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "quitAnyway",
        ).length ?? null,
      updates: window.__aelomiMock?.storage.updates("page").length ?? null,
    })),
  ).toEqual({ exited: true, text: "x", confirmations: 1, updates: 1 });
  expect(nonLocalRequests).toEqual([]);
});
