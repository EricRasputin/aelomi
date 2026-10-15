import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-7 CMD-1 ⌘K opens the palette through the shared dispatcher and returns editor focus", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "page-daily", title: "Daily" }],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await app.open("/page/page-daily");
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toHaveText("plan");
  await body.click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.press("Meta+k");
  const palette = screen.getByRole("dialog", "Command palette");
  await expect(palette).toBeVisible();
  await browser.keyboard.type("Daily");
  await expect(palette.getByRole("option", "Daily")).toBeVisible();
  await browser.keyboard.press("Escape");
  await expect(palette).not.toBeVisible();
  await browser.keyboard.type("x");
  await expect(body).toHaveText("planx");
});

test("SHELL-7 TREE-15 editor undo and redo never undo workspace record edits", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-other", title: "Other" },
      ],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await app.open("/page/page-daily");
  const body = screen.getByRole("textbox", "Page body");
  await expect(body).toHaveText("plan");
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Other");
  await expect(screen.getByRole("option", "Other")).toBeVisible();
  await browser.keyboard.press("Escape");
  await body.click();
  await browser.keyboard.press("Meta+ArrowRight");
  await browser.keyboard.type(" token");
  await expect(body).toHaveText("plan token");
  await browser.evaluate(() => {
    window.__aelomiMock?.workspace.trash("page-other", "batch-shortcut");
    return null;
  });
  await browser.keyboard.press("Meta+z");
  await expect(body).toHaveText("plan");
  await browser.keyboard.press("Meta+Shift+z");
  await expect(body).toHaveText("plan token");
  await browser.keyboard.press("Meta+k");
  await browser.keyboard.type("Other");
  await expect(screen.getByRole("option", "Other")).not.toBeVisible();
  await expect(screen.getByText("No page content matches.")).toBeVisible();
});
