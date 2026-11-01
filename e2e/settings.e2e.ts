import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-5 acceptance 1: Theme applies Dark to the sidebar, editor, Settings and command palette", async ({
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
    await browser.evaluate(() => {
      const editor = document.querySelector(".page-body.bn-editor");
      const settings = document.querySelector(".settings-dialog");
      const sidebar = document.querySelector(".sidebar");
      if (!editor || !settings || !sidebar)
        throw new Error("Sidebar, editor or Settings missing");
      return {
        theme: document.documentElement.dataset.theme ?? null,
        sidebar: getComputedStyle(sidebar).backgroundColor,
        editor: getComputedStyle(editor).color,
        settings: getComputedStyle(settings).backgroundColor,
      };
    }),
  ).toEqual({
    theme: "dark",
    sidebar: "rgb(36, 36, 40)",
    editor: "rgb(237, 237, 240)",
    settings: "rgb(31, 31, 35)",
  });
  await browser.keyboard.press("Escape");
  await browser.keyboard.press("Meta+k");
  await expect(screen.getByRole("dialog", "Command palette")).toBeVisible();
  expect(
    await browser.evaluate(() => {
      const palette = document.querySelector(".palette");
      if (!palette) throw new Error("Palette missing");
      return getComputedStyle(palette).backgroundColor;
    }),
  ).toBe("rgb(31, 31, 35)");
});

test("SHELL-6 acceptance 3: Settings shortcut opens the modal and Escape closes it", async ({
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
  await expect(screen.getByRole("navigation", "Pages")).toBeVisible();
});

test("SHELL-5 acceptance 6: System theme follows changes in the mock macOS appearance", async ({
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
  await expect(screen.getByRole("navigation", "Pages")).toBeVisible();
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
