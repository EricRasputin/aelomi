import { expect, test } from "./fixtures";

test("SHELL-1 SHELL-6 hidden sidebar gives its width to the editor", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "A" }] };
  });
  await page.goto("/");
  const sidebar = page.getByRole("complementary", { name: "Sidebar" });
  await expect(sidebar).toBeVisible();
  const before = await page.locator("main").boundingBox();
  const width = await sidebar.boundingBox();
  await page.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeHidden();
  const hidden = await page.locator("main").boundingBox();
  expect(Math.round((hidden?.width ?? 0) - (before?.width ?? 0))).toBe(
    Math.round(width?.width ?? 0),
  );
  await page.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeVisible();
});

test("SHELL-4 TREE-1 CMD-6 trash the only page then create from the empty state", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "A" }] };
  });
  await page.goto("/");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("A");
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("trash");
  await page.getByRole("option", { name: "Move to trash" }).click();
  const empty = page.getByRole("region", { name: "No page open" });
  await expect(empty).toBeVisible();
  await expect(
    page
      .getByRole("navigation", { name: "Pages" })
      .getByRole("button", { name: "A", exact: true }),
  ).toHaveCount(0);
  await page.keyboard.press("Meta+k");
  await expect(page.getByRole("option", { name: "Move to trash" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("option", { name: "New page" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await empty.getByRole("button", { name: "New page" }).click();
  await expect(title).toHaveValue("");
  await expect(title).toHaveAttribute("placeholder", "Untitled");
  await expect(title).toBeFocused();
  await expect(
    page
      .getByRole("navigation", { name: "Pages" })
      .getByRole("button", { name: "Untitled", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls
        .filter(
          (call) =>
            call.command === "pageTrash" || call.command === "pageCreate",
        )
        .map((call) => [call.command, call.args]),
    ),
  ).toEqual([
    ["pageTrash", ["a"]],
    ["pageCreate", [null]],
  ]);
});

test("TREE-1 TREE-7 SHELL-8 EDIT-1 title input persists immediately and Return starts the body", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Meta+n");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toBeFocused();
  await page.keyboard.type("Projects");
  await expect(title).toHaveValue("Projects");
  await expect(
    page
      .getByRole("navigation", { name: "Pages" })
      .getByRole("button", { name: "Projects", exact: true }),
  ).toBeVisible();
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toBeVisible();
  await title.press("Enter");
  await expect(body).toBeFocused();
  await page.keyboard.type("body token");
  await expect(body).toHaveText("body token");
  await title.focus();
  const delivered = await page.evaluate(async () => {
    const field = document.querySelector<HTMLTextAreaElement>(
      '[aria-label="Page title"]',
    );
    if (!field) throw new Error("missing title");
    const start = performance.now();
    document.execCommand("insertText", false, "x");
    while (
      !window.__aelomiMock?.calls.some(
        (call) => call.command === "pageRename" && call.args[1] === field.value,
      )
    ) {
      if (performance.now() - start > 200)
        return { elapsed: 201, value: field.value };
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    return { elapsed: performance.now() - start, value: field.value };
  });
  expect(delivered.value).toBe("Projectsx");
  expect(delivered.elapsed).toBeLessThan(100);
});

test("TREE-7 TREE-15 title history never consumes body history after repeated Undo", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "a", title: "Inbox" }],
      bodies: { a: [{ id: "block", content: ["body"] }] },
    };
  });
  await page.goto("/page/a");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("body");
  await body.click();
  await page.keyboard.press("Meta+ArrowRight");
  await page.keyboard.type(" token");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await title.focus();
  await title.press("Meta+ArrowRight");
  await page.keyboard.type("xy");
  await expect(title).toHaveValue("Inboxxy");
  for (let i = 0; i < 5; i++) await title.press("Meta+z");
  await page.evaluate(() => window.__aelomiMock?.menu.activate("undo"));
  await expect(title).toHaveValue("Inbox");
  await expect(body).toHaveText("body token");
  await title.press("Meta+Shift+z");
  await expect(title).toHaveValue("Inboxx");
  await expect(
    page
      .getByRole("navigation", { name: "Pages" })
      .getByRole("button", { name: "Inboxx", exact: true }),
  ).toBeVisible();
});

test("TREE-2 TREE-6 disclosure persists settings and a row click focuses the editor", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "p", title: "Projects" },
        { id: "a", title: "A", parentId: "p" },
      ],
      settings: { expanded_rows: ["p"] },
    };
  });
  await page.goto("/page/p");
  const rows = page.getByRole("navigation", { name: "Pages" });
  await expect(
    rows.getByRole("button", { name: "A", exact: true }),
  ).toBeVisible();
  await rows.getByRole("button", { name: "Collapse Projects" }).click();
  await expect(
    rows.getByRole("button", { name: "A", exact: true }),
  ).toHaveCount(0);
  await rows.getByRole("button", { name: "Expand Projects" }).click();
  await rows.getByRole("button", { name: "A", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Page title", exact: true }),
  ).toHaveValue("A");
  await expect(page.getByRole("textbox", { name: "Page body" })).toBeFocused();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.calls
          .filter((call) => call.command === "settingsUpdate")
          .map((call) => call.args),
      ),
    )
    .toEqual([[{ expanded_rows: [] }], [{ expanded_rows: ["p"] }]]);
});
