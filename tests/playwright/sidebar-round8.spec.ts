import { expect, type Page, test } from "./fixtures";

const title = (page: Page) =>
  page.getByRole("textbox", { name: "Page title", exact: true });
const body = (page: Page) => page.getByRole("textbox", { name: "Page body" });
const block = (page: Page, id: string) =>
  page.locator(`[data-node-type="blockOuter"][data-id="${id}"]`);

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "a", title: "Alpha" }],
      bodies: {
        a: [
          { id: "one", content: ["Other body"] },
          { id: "two", content: ["Alpha needle body"] },
        ],
      },
    };
  });
});

test("TREE-7 a same-page content hit from the title types at the block start", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(body(page)).toHaveText("Other bodyAlpha needle body");
  await title(page).focus();
  await expect(title(page)).toBeFocused();
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("needle");
  await page
    .getByRole("option")
    .filter({ hasText: "Alpha needle body" })
    .click();
  await expect(page).toHaveURL(/\/page\/a\?block=two$/);
  await expect(body(page)).toBeFocused();
  await page.keyboard.type("Found ");
  await expect(block(page, "two")).toHaveText("Found Alpha needle body");
  await expect(block(page, "one")).toHaveText("Other body");
  await expect(title(page)).toHaveValue("Alpha");
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Other body\nFound Alpha needle body",
  );
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((record) => record.title),
    ),
  ).toEqual(["Alpha"]);
});

test("SHELL-10 Keep app open preserves the moved caret after a block mount", async ({
  page,
}) => {
  await page.goto("/page/a?block=two");
  await expect(body(page)).toBeFocused();
  await page.keyboard.type("Hit ");
  await expect(block(page, "two")).toHaveText("Hit Alpha needle body");
  await block(page, "one").click();
  await page.keyboard.press("Meta+ArrowRight");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseAppends({ Io: "Rebuild read failed" }),
  );
  await page.keyboard.type(" draft");
  await expect(block(page, "one")).toHaveText("Other body draft");
  await page.keyboard.press("Meta+ArrowLeft");
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toHaveAccessibleName("Quit with unsaved changes?");
  await expect(page.getByTestId("page-editor")).toHaveAttribute(
    "contenteditable",
    "false",
  );
  await page.evaluate(() => window.__aelomiMock?.storage.refuseAppends(null));
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(body(page)).toHaveAttribute("contenteditable", "true");
  await expect(body(page)).toBeFocused();
  await page.keyboard.type("Complete ");
  await expect(block(page, "one")).toHaveText("Complete Other body draft");
  await expect(block(page, "two")).toHaveText("Hit Alpha needle body");
  await expect(title(page)).toHaveValue("Alpha");
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.bodies.text("a")))
    .toBe("Complete Other body draft\nHit Alpha needle body");
});
