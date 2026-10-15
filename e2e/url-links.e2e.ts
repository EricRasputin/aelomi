import { expect } from "e2e";
import { test } from "./fixtures";

test.beforeEach(async ({ browser }) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "page-a", title: "A" }],
      bodies: {
        "page-a": [
          {
            id: "block-a",
            content: [{ text: "Example", url: "https://example.com" }],
          },
        ],
      },
    };
  });
});

test("SHELL-13 acceptance 9: a URL opens in the default browser and the page stays shown", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
  await expect(browser.locator('[data-testid="page-editor"]')).toHaveAttribute(
    "contenteditable",
    "true",
  );
  await screen.getByRole("link", "Example", { exact: true }).click();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "openUrl",
        ) ?? null,
    ),
  ).toEqual([{ command: "openUrl", args: ["https://example.com"] }]);
  await expect(screen.getByRole("heading", "A", { exact: true })).toBeVisible();
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
});

test("SHELL-13 double-click selects link text with only one browser handoff", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await expect(browser.locator('[data-testid="page-editor"]')).toHaveAttribute(
    "contenteditable",
    "true",
  );
  await screen.getByRole("link", "Example", { exact: true }).dblclick();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "openUrl",
        ) ?? null,
    ),
  ).toEqual([{ command: "openUrl", args: ["https://example.com"] }]);
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
});

test("SHELL-13 drag-select and a link context menu make no additional handoff", async ({
  app,
  browser,
  screen,
}) => {
  await app.open("/");
  await expect(browser.locator('[data-testid="page-editor"]')).toHaveAttribute(
    "contenteditable",
    "true",
  );
  const link = screen.getByRole("link", "Example", { exact: true });
  await link.click();
  const box = await browser.evaluate(() => {
    const rect = document
      .querySelector(".page-body a")
      ?.getBoundingClientRect();
    if (!rect) throw new Error("Missing link");
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
  });
  await browser.mouse.move(box.x + 1, box.y + box.height / 2);
  await browser.mouse.down();
  for (let x = 3; x < box.width - 1; x += 3)
    await browser.mouse.move(box.x + x, box.y + box.height / 2);
  await browser.mouse.up();
  expect(
    await browser.evaluate(() => window.getSelection()?.isCollapsed ?? true),
  ).toBe(false);
  await link.secondaryTap();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "openUrl",
        ) ?? null,
    ),
  ).toEqual([{ command: "openUrl", args: ["https://example.com"] }]);
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
});
