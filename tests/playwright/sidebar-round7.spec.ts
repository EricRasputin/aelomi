import { expect, type Page, test } from "./fixtures";

const title = (page: Page) =>
  page.getByRole("textbox", { name: "Page title", exact: true });
const body = (page: Page) => page.getByRole("textbox", { name: "Page body" });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "a", title: "Alpha" },
        { id: "b", title: "Beta" },
      ],
      bodies: {
        a: [{ id: "one", content: ["Alpha body"] }],
        b: [
          ...Array.from({ length: 40 }, (_, i) => ({
            id: `context-${i}`,
            content: ["Context"],
          })),
          { id: "two", content: ["Beta needle body"] },
        ],
      },
    };
  });
  await page.goto("/page/a");
  await expect(body(page)).toHaveText("Alpha body");
  await page.evaluate(() => window.__aelomiMock?.bodies.holdLoad("b"));
});

for (const path of ["content hit", "Return during load"]) {
  test(`TREE-7 ${path} defers body focus and preserves title typing`, async ({
    page,
  }) => {
    const renderedBody = `${"Context".repeat(40)}Beta needle body`;
    const storedBody = `${"Context\n".repeat(40)}Beta needle body`;
    if (path === "content hit") {
      await page.addStyleTag({
        content: ".block-flash { animation-play-state: paused; }",
      });
      await page.keyboard.press("Meta+k");
      await page.getByRole("combobox").fill("needle");
      await page
        .getByRole("option")
        .filter({ hasText: "Beta needle body" })
        .click();
      await expect(page).toHaveURL(/\/page\/b\?block=two$/);
    } else {
      await page
        .getByRole("navigation", { name: "Pages" })
        .getByRole("button", { name: "Beta", exact: true })
        .click();
    }
    await expect(title(page)).toHaveValue("Beta");
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__aelomiMock?.calls.filter(
              (call) => call.command === "docLoad" && call.args[0] === "b",
            ).length,
        ),
      )
      .toBe(1);
    await title(page).click();
    await title(page).press("End");
    if (path === "Return during load") await page.keyboard.press("Enter");
    await page.keyboard.type(" draft");
    await expect(title(page)).toHaveValue("Beta draft");
    await expect(title(page)).toBeFocused();
    await page.evaluate(() => window.__aelomiMock?.bodies.releaseLoad("b"));
    await expect(body(page)).toHaveText(renderedBody);
    if (path === "content hit") {
      await expect(
        page.locator('[data-node-type="blockOuter"][data-id="two"]'),
      ).toBeInViewport({ ratio: 1 });
      await expect(page.locator(".block-flash")).toHaveCount(1);
    }
    await page.keyboard.type(" complete");
    await expect(title(page)).toHaveValue("Beta draft complete");
    await expect(title(page)).toBeFocused();
    await expect(body(page)).toHaveText(renderedBody);
    expect(
      await page.evaluate(() => window.__aelomiMock?.bodies.text("b")),
    ).toBe(storedBody);
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__aelomiMock?.workspace
              .tree()
              .pages.find((record) => record.id === "b")?.title,
        ),
      )
      .toBe("Beta draft complete");
  });
}
