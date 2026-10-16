import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-5 Theme applies Dark to the editor, Settings and command palette", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      appearance: "light",
      pages: [{ id: "page-daily", title: "Daily" }],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await app.open("/page/page-daily");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("plan");
  await browser.keyboard.press("Meta+,");
  const settings = screen.getByRole("dialog", "Settings");
  await expect(settings).toBeVisible();
  await settings.getByRole("radio", "Dark").click();
  expect(
    await browser.evaluate(() => ({
      theme: document.documentElement.dataset.theme ?? null,
      editor: getComputedStyle(
        document.querySelector('[role="textbox"][aria-label="Page body"]') ??
          document.body,
      ).color,
      settings: getComputedStyle(
        document.querySelector(".settings-dialog") ?? document.body,
      ).backgroundColor,
    })),
  ).toEqual({
    theme: "dark",
    editor: "rgb(237, 237, 240)",
    settings: "rgb(31, 31, 35)",
  });
  await browser.keyboard.press("Escape");
  await browser.keyboard.press("Meta+k");
  await expect(screen.getByRole("dialog", "Command palette")).toBeVisible();
  expect(
    await browser.evaluate(
      () =>
        getComputedStyle(document.querySelector(".palette") ?? document.body)
          .backgroundColor,
    ),
  ).toBe("rgb(31, 31, 35)");
});

test("SHELL-6 Settings shortcut opens the modal and Escape closes it", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await browser.keyboard.press("Meta+,");
  const settings = screen.getByRole("dialog", "Settings");
  await expect(settings).toBeVisible();
  await expect(settings.getByRole("radio", "System")).toBeVisible();
  await browser.keyboard.press("Escape");
  await expect(settings).not.toBeVisible();
  await expect(screen.getByRole("button", "Ping again")).toBeVisible();
});

test("SHELL-5 System theme follows changes in the mock macOS appearance", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      settings: { theme: "System" },
      appearance: "dark",
    };
  });
  await app.open("/");
  await expect(screen.getByRole("button", "Ping again")).toBeVisible();
  expect(
    await browser.evaluate(() => ({
      theme: document.documentElement.dataset.theme ?? null,
      background: getComputedStyle(document.documentElement).backgroundColor,
    })),
  ).toEqual({ theme: "dark", background: "rgb(24, 24, 27)" });
  await browser.evaluate(() => {
    window.__aelomiMock?.appearance.set("light");
    return null;
  });
  expect(
    await browser.evaluate(() => ({
      theme: document.documentElement.dataset.theme ?? null,
      background: getComputedStyle(document.documentElement).backgroundColor,
    })),
  ).toEqual({ theme: "light", background: "rgb(250, 250, 250)" });
});
