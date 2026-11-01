import { expect, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "page-daily", title: "Daily" }],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
  await page.goto("/page/page-daily");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("plan");
  await body.click();
  await page.keyboard.press("Meta+ArrowRight");
  await page.keyboard.insertText(" token");
  await expect(body).toHaveText("plan token");
});

test("SHELL-7 menu-command events undo and redo the focused editor", async ({
  page,
}) => {
  const body = page.getByRole("textbox", { name: "Page body" });
  await page.evaluate(() => window.__aelomiMock?.menu.activate("undo"));
  await expect(body).toHaveText("plan");
  await page.evaluate(() => window.__aelomiMock?.menu.activate("redo"));
  await expect(body).toHaveText("plan token");
});

test("SHELL-7 palette open with focus off its input leaves page history unchanged", async ({
  page,
}) => {
  const body = page.getByRole("textbox", { name: "Page body" });
  await page.keyboard.press("Meta+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  const input = page.getByRole("combobox", { name: "Search pages" });
  await expect(input).toBeFocused();
  await input.fill("Daily");
  await input.evaluate((element) => element.blur());
  await expect(input).not.toBeFocused();
  await page.keyboard.press("Meta+z");
  await expect(body).toHaveText("plan token");
  await page.evaluate(() => window.__aelomiMock?.menu.activate("undo"));
  await expect(body).toHaveText("plan token");
  await expect(palette).toBeVisible();
  await input.fill("Daily");
  const row = palette.getByRole("option", { name: "Daily" });
  await expect(row).toBeVisible();
  await row.focus();
  await expect(row).toBeFocused();
  await page.keyboard.press("Meta+z");
  await input.fill("Daily");
  await row.focus();
  await page.evaluate(() => window.__aelomiMock?.menu.activate("undo"));
  await expect(body).toHaveText("plan token");
  await input.focus();
  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
  await page.keyboard.press("Meta+z");
  await expect(body).toHaveText("plan");
});

test("SHELL-7 composition suppresses keys and menu events until composition ends", async ({
  page,
}) => {
  const body = page.getByRole("textbox", { name: "Page body" });
  await body.dispatchEvent("compositionstart");
  for (const command of ["undo", "redo", "openPalette"]) {
    await page.evaluate(
      (id) => window.__aelomiMock?.menu.activate(id),
      command,
    );
    await expect(body).toHaveText("plan token");
    await expect(
      page.getByRole("dialog", { name: "Command palette" }),
    ).toBeHidden();
  }
  await body.dispatchEvent("keydown", {
    key: "k",
    metaKey: true,
    isComposing: true,
  });
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toBeHidden();
  await body.dispatchEvent("compositionend");
  await page.evaluate(() => window.__aelomiMock?.menu.activate("undo"));
  await expect(body).toHaveText("plan");
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toBeVisible();
});

test("SHELL-7 the quit confirmation blocks keyboard and menu commands", async ({
  page,
}) => {
  await page.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(true);
    window.__aelomiMock?.quit.request();
    window.__aelomiMock?.quit.timeOut();
  });
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Meta+k");
  await page.evaluate(() => window.__aelomiMock?.menu.activate("openPalette"));
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toBeHidden();
  for (const command of ["undo", "redo"]) {
    await page.evaluate(
      (id) => window.__aelomiMock?.menu.activate(id),
      command,
    );
  }
  await expect(
    page.getByRole("textbox", { name: "Page body", includeHidden: true }),
  ).toHaveText("plan token");
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(dialog).toBeHidden();
  await page.evaluate(() => window.__aelomiMock?.menu.activate("openPalette"));
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toBeVisible();
});
