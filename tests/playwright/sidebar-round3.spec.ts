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

test("TREE-1 typing immediately after Command N reaches only the new title", async ({
  page,
}) => {
  await page.goto("/page/a");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("Original body");
  await body.click();
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Immediate title");
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Immediate title");
  await expect(title).toBeFocused();
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Original body",
  );
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
      ),
    )
    .toEqual(["Alpha", "Beta", "Immediate title"]);
});

test("TREE-1 accessible New page activation keeps focus and buffers typing", async ({
  page,
}) => {
  await page.goto("/page/a");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("Original body");
  await body.focus();
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
  await page
    .getByRole("button", { name: "New page", exact: true })
    .dispatchEvent("click");
  await expect(
    page.getByRole("button", { name: "Creating page…", exact: true }),
  ).toBeFocused();
  await page.keyboard.type("Accessible title");
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Accessible title");
  await expect(title).toBeFocused();
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Original body",
  );
});

test("TREE-1 buffered title keeps its insertion point and Undo after creation", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "Original body",
  );
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Buffered");
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Buffered");
  await expect(title).toBeFocused();
  await page.keyboard.type(" end");
  await expect(title).toHaveValue("Buffered end");
  for (let i = 0; i < 4; i++) await title.press("Meta+z");
  await expect(title).toHaveValue("Buffered");
  await title.press("Meta+z");
  await expect(title).toHaveValue("");
  await title.press("Meta+Shift+z");
  await expect(title).toHaveValue("Buffered");
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Original body",
  );
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

test("TREE-1 late create reveals its row without navigating or renaming after editor focus", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(
    page.getByRole("textbox", { name: "Page title", exact: true }),
  ).toHaveValue("Alpha");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
  const button = page.getByRole("button", { name: "New page", exact: true });
  await button.click();
  await expect(
    page.getByRole("button", { name: "Creating page…", exact: true }),
  ).toBeFocused();
  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("button", { name: "Beta", exact: true })
    .click();
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toBeFocused();
  await page.keyboard.press("Meta+ArrowRight");
  await page.keyboard.type(" kept typing");
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  await expect(
    page.getByText("New page created.", { exact: true }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/page\/b$/);
  await expect(body).toBeFocused();
  await expect(body).toHaveText("Other body kept typing");
  await expect(
    page
      .getByRole("navigation", { name: "Pages" })
      .getByRole("button", { name: "Untitled", exact: true }),
  ).toBeInViewport();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Alpha", "Beta", ""]);
});

test("TREE-7 refresh error keeps rows and title with a retry notice then recovers", async ({
  page,
}) => {
  await page.goto("/page/a");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Alpha");
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
  await page.getByRole("button", { name: "Try again" }).click();
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(false));
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
    page.getByText("Moving to Trash…", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("button", { name: "Child", exact: true })
    .click();
  await expect(title).toHaveValue("Child");
  await expect(title).toHaveJSProperty("readOnly", true);
  await expect(
    page.getByText("Moving to Trash…", { exact: true }),
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

test("TREE-1 pending create cannot open from a row, palette, or route before durable ok", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(
    page.getByRole("textbox", { name: "Page title", exact: true }),
  ).toHaveValue("Alpha");
  await page.clock.install();
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
  await page.getByRole("button", { name: "New page", exact: true }).click();
  await page.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(true);
    window.__aelomiMock?.storage.refuseRecords(null);
  });
  await page.clock.runFor(6000);
  const created = await page.evaluate(
    () =>
      window.__aelomiMock?.workspace
        .tree()
        .pages.find((p) => p.id !== "a" && p.id !== "b")?.id,
  );
  if (!created) throw new Error("missing accepted create");
  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("button", { name: "Untitled", exact: true })
    .click({ force: true });
  await expect(page).toHaveURL(/\/page\/a$/);
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Untitled");
  await page
    .getByRole("option", { name: "Untitled", exact: true })
    .click({ force: true });
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(/\/page\/a$/);
  await page.evaluate((id) => {
    history.pushState({}, "", `/page/${id}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, created);
  await page.clock.runFor(100);
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveCount(0);
  expect(
    await page.evaluate(
      (id) =>
        window.__aelomiMock?.calls.filter(
          (c) =>
            (c.command === "docLoad" || c.command === "docAppend") &&
            c.args[0] === id,
        ).length,
      created,
    ),
  ).toBe(0);
  await page.evaluate(() => window.__aelomiMock?.storage.failWrites(false));
  await page.clock.runFor(6000);
  await expect(page.getByRole("textbox", { name: "Page body" })).toBeVisible();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Alpha", "Beta", ""]);
});

test("SHELL-9 draining blocks sidebar and palette navigation and preserves the exact title", async ({
  page,
}) => {
  await page.goto("/page/a");
  const title = page.getByRole("textbox", { name: "Page title", exact: true });
  await expect(title).toHaveValue("Alpha");
  await title.fill("Exact title before quit");
  await expect
    .poll(() =>
      page.evaluate(
        () => window.__aelomiMock?.workspace.tree().pages[0]?.title,
      ),
    )
    .toBe("Exact title before quit");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held delivery" }),
  );
  await page.getByRole("button", { name: "New page", exact: true }).click();
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Beta");
  await expect(
    page.getByRole("option", { name: "Beta", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await expect(page.locator('[aria-label="Page title"]')).toHaveJSProperty(
    "readOnly",
    true,
  );
  await page
    .getByRole("option", { name: "Beta", exact: true })
    .click({ force: true });
  await expect(page).toHaveURL(/\/page\/a$/);
  await page.keyboard.press("Meta+n");
  await page.keyboard.press("Escape");
  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("button", { name: "Beta", exact: true })
    .click({ force: true });
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toHaveCount(0);
  await expect(page).toHaveURL(/\/page\/a$/);
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited))
    .toBe(true);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Exact title before quit", "Beta", ""]);
});
