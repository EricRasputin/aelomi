import { expect, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "a", title: "Alpha" },
        { id: "b", title: "Beta" },
      ],
      bodies: {
        a: [{ id: "one", content: ["Original body"] }],
        b: [{ id: "two", content: ["Other body"] }],
      },
    };
  });
});

test("EDIT-11 title Undo and Redo survive A to B to A", async ({ page }) => {
  await page.goto("/page/a");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await title.fill("Alpha revised");
  const rows = page.getByRole("navigation", { name: "Pages" });
  await rows.getByRole("button", { name: "Beta", exact: true }).click();
  await rows
    .getByRole("button", { name: "Alpha revised", exact: true })
    .click();
  await title.focus();
  await title.press("Meta+z");
  await expect(title).toHaveValue("Alpha");
  await title.press("Meta+Shift+z");
  await expect(title).toHaveValue("Alpha revised");
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
      ),
    )
    .toEqual(["Alpha revised", "Beta"]);
});

test("CMD-3 Move to trash requires explicit selection after to and content replies", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(
    page.getByRole("textbox", { name: "Page title", exact: true }),
  ).toHaveValue("Alpha");
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("to");
  const trash = page.getByRole("option", { name: "Move to trash" });
  await expect(trash).toHaveAttribute("aria-selected", "false");
  await expect(page.getByRole("listbox")).toHaveAttribute(
    "data-settled",
    "true",
  );
  await page.keyboard.press("Enter");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Alpha", "Beta"]);
  await page.keyboard.press("ArrowDown");
  await expect(trash).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.workspace.tree().trash.map((p) => p.title),
      ),
    )
    .toEqual(["Alpha"]);
});

test("TREE-7 refresh error keeps rows and title with a retry notice then recovers", async ({
  page,
}) => {
  await page.goto("/page/a");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Alpha");
  await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
  await page.clock.pauseAt(new Date("2026-01-01T00:00:01Z"));
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(true));
  const rows = page.getByRole("navigation", { name: "Pages" });
  await expect(
    rows.getByRole("button", { name: "Beta", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Pages couldn't be refreshed. Showing the last available list.",
    ),
  ).toBeVisible();
  await expect(title).toHaveValue("Alpha");
  await expect(title).toHaveJSProperty("readOnly", true);
  await page.evaluate(() =>
    window.__aelomiMock?.workspace.failReads(false, false),
  );
  const before = await page.evaluate(
    () =>
      window.__aelomiMock?.calls.filter((c) => c.command === "pageTree")
        .length ?? 0,
  );
  await page.getByRole("button", { name: "Try again" }).click();
  expect(
    await page.evaluate(
      () =>
        window.__aelomiMock?.calls.filter((c) => c.command === "pageTree")
          .length,
    ),
  ).toBe(before + 1);
  await expect(
    page.getByText(
      "Pages couldn't be refreshed. Showing the last available list.",
    ),
  ).toHaveCount(0);
  await expect(title).toBeEditable();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Alpha", "Beta"]);
});

test("TREE-9 pending trash freezes the target title and its descendants", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "p", title: "Parent" },
        { id: "c", title: "Child", parentId: "p" },
      ],
      settings: { expanded_rows: ["p"] },
    };
  });
  await page.goto("/page/p");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Parent");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held trash" }),
  );
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("trash");
  await page.getByRole("option", { name: "Move to trash" }).click();
  await expect(title).toHaveJSProperty("readOnly", true);
  await expect(
    page.locator(".page-notice").getByText("Moving to trash…", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("button", { name: "Child", exact: true })
    .click();
  await expect(title).toHaveValue("Child");
  await expect(title).toHaveJSProperty("readOnly", true);
  await expect(
    page.locator(".page-notice").getByText("Moving to trash…", { exact: true }),
  ).toBeVisible();
  await title.focus();
  await page.keyboard.type("lost");
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  await expect(
    page.getByRole("region", { name: "No page open" }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().trash.map((p) => p.title),
    ),
  ).toEqual(["Parent"]);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls
        .filter((c) => c.command === "pageRename")
        .map((c) => c.args),
    ),
  ).toEqual([]);
});
