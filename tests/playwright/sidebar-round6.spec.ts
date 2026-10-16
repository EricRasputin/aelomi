import { expect, type Page, test } from "./fixtures";

const title = (page: Page) =>
  page.getByRole("textbox", { name: "Page title", exact: true });
const body = (page: Page) => page.getByRole("textbox", { name: "Page body" });
const rows = (page: Page) => page.getByRole("navigation", { name: "Pages" });

test.beforeEach(async ({ page }, info) => {
  await page.addInitScript((scrolling) => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "a", title: "Alpha" },
        { id: "b", title: "Beta" },
        ...(scrolling
          ? Array.from({ length: 80 }, (_, i) => ({
              id: `p-${i}`,
              title: `Page ${i + 1}`,
            }))
          : []),
      ],
      bodies: {
        a: [{ id: "one", content: ["Alpha body"] }],
        b: [{ id: "two", content: ["Beta body"] }],
      },
    };
  }, info.title.startsWith("SHELL-1"));
  await page.goto("/page/a");
  await expect(body(page)).toHaveText("Alpha body");
});

for (const path of ["row click", "trash fallback"]) {
  test(`TREE-7 ${path} yields pending body focus to title typing`, async ({
    page,
  }) => {
    await page.evaluate(() => window.__aelomiMock?.bodies.holdLoad("b"));
    if (path === "row click") {
      await rows(page)
        .getByRole("button", { name: "Beta", exact: true })
        .click();
    } else {
      await page.keyboard.press("Meta+k");
      await page.getByRole("combobox").fill("Move to trash");
      await page
        .getByRole("option", { name: "Move to trash", exact: true })
        .click();
    }
    await expect(title(page)).toHaveValue("Beta");
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__aelomiMock?.calls.filter(
              (c) => c.command === "docLoad" && c.args[0] === "b",
            ).length,
        ),
      )
      .toBe(1);
    await title(page).click();
    await title(page).press("End");
    await page.keyboard.type(" draft");
    await page.evaluate(() => window.__aelomiMock?.bodies.releaseLoad("b"));
    await expect(body(page)).toHaveText("Beta body");
    await page.keyboard.type(" complete");
    await expect(title(page)).toHaveValue("Beta draft complete");
    await expect(title(page)).toBeFocused();
    await expect(body(page)).toHaveText("Beta body");
    expect(
      await page.evaluate(() => window.__aelomiMock?.bodies.text("b")),
    ).toBe("Beta body");
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__aelomiMock?.workspace
              .tree()
              .pages.find((p) => p.id === "b")?.title,
        ),
      )
      .toBe("Beta draft complete");
  });
}

test("TREE-1 Return in a held title preserves its middle caret through recovery", async ({
  page,
}) => {
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held" }),
  );
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Plan review");
  await title(page).evaluate((field) => {
    if (!(field instanceof HTMLTextAreaElement))
      throw new Error("Missing title");
    field.setSelectionRange(4, 4);
  });
  await page.keyboard.press("Enter");
  await page.keyboard.type(" Q3");
  await expect(title(page)).toHaveValue("Plan Q3 review");
  await expect(title(page)).toHaveJSProperty("selectionStart", 7);
  await expect(title(page)).toHaveJSProperty("selectionEnd", 7);
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  await expect(body(page)).toHaveText("");
  await expect(title(page)).toBeFocused();
  await expect(title(page)).toHaveJSProperty("selectionStart", 7);
  await page.keyboard.type(" final");
  await expect(title(page)).toHaveValue("Plan Q3 final review");
  await expect(body(page)).toHaveText("");
});

test("SHELL-1 the status strip leaves the last failed row clear in a scrolling tree", async ({
  page,
}) => {
  await page.evaluate(() => {
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" });
  });
  await page.keyboard.press("Meta+n");
  await expect(page.getByRole("status").first()).toHaveAccessibleName(
    "Connection status",
  );
  const notice = page.locator(".shell-notice");
  await expect(notice).toHaveText("A new page couldn't be created.");
  const failed = rows(page).locator('[data-state="failed"]');
  await expect(failed).toBeInViewport();
  const geometry = await page.evaluate(() => {
    const row = document
      .querySelector('[data-state="failed"]')
      ?.getBoundingClientRect();
    const notice = document
      .querySelector(".shell-notice")
      ?.getBoundingClientRect();
    const scroll = document.querySelector(".sidebar-scroll");
    if (!row || !notice || !scroll)
      throw new Error("Missing scrolling sidebar");
    return {
      scrolls: scroll.scrollHeight > scroll.clientHeight,
      overlaps:
        row.left < notice.right &&
        row.right > notice.left &&
        row.top < notice.bottom &&
        row.bottom > notice.top,
    };
  });
  expect(geometry).toEqual({ scrolls: true, overlaps: false });
  const optionButton = failed.getByRole("button").last();
  await expect(optionButton).toHaveAccessibleDescription("Not created");
  await optionButton.click();
  await expect(title(page)).toHaveJSProperty("readOnly", true);
});
