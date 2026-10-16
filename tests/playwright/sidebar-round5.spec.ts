import { expect, type Page, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "a", title: "Alpha" },
        { id: "b", title: "Beta" },
      ],
      bodies: { a: [{ id: "one", content: ["Original body"] }] },
    };
  });
});
const title = (page: Page) =>
  page.getByRole("textbox", { name: "Page title", exact: true });
const rows = (page: Page) => page.getByRole("navigation", { name: "Pages" });

test("SHELL-1 cold page navigation preloads the body before the first frame mounts", async ({
  page,
}) => {
  let release = () => {};
  const held = new Promise<void>((done) => {
    release = done;
  });
  let requested = () => {};
  const started = new Promise<void>((done) => {
    requested = done;
  });
  await page.route(/\/assets\/page-view-[^/]+\.js$/, async (route) => {
    requested();
    await held;
    await route.continue();
  });
  await page.goto("/page/a");
  await started;
  try {
    await expect(title(page)).toHaveCount(0);
  } finally {
    release();
  }
  await expect(title(page)).toHaveValue("Alpha");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "Original body",
  );
});
async function creates(page: Page) {
  return page.evaluate(() => [
    ...new Set(
      window.__aelomiMock?.calls
        .filter((c) => c.command === "pageCreate")
        .map((c) => String(c.args[1])),
    ),
  ]);
}
async function bodyCalls(page: Page, id: string) {
  return page.evaluate(
    (id) =>
      window.__aelomiMock?.calls
        .filter(
          (c) =>
            (c.command === "docLoad" || c.command === "docAppend") &&
            c.args[0] === id,
        )
        .map((c) => c.command),
    id,
  );
}
async function hold(page: Page) {
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held" }),
  );
}
async function release(page: Page) {
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
}
async function trash(page: Page) {
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Move to trash");
  await page
    .getByRole("option", { name: "Move to trash", exact: true })
    .click();
}

test("SHELL-6 hiding the sidebar during a held create cannot defer body focus", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Plan");
  const [id] = await creates(page);
  if (!id) throw new Error("Missing create");
  await rows(page).getByRole("button", { name: "Beta", exact: true }).focus();
  await page.keyboard.press("Meta+Backslash");
  await expect(
    page.getByRole("complementary", { name: "Sidebar" }),
  ).toBeHidden();
  await title(page).focus();
  await page.keyboard.type(" for Q3");
  await release(page);
  await expect.poll(() => bodyCalls(page, id)).toEqual(["docLoad"]);
  await expect(title(page)).toBeFocused();
  await page.keyboard.type(" review");
  await expect(title(page)).toHaveValue("Plan for Q3 review");
});

test("TREE-1 a forgotten failed ID under stale reads never loads an orphan body", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await page.evaluate(() => {
    window.__aelomiMock?.workspace.failReads(true);
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" });
  });
  await page.keyboard.press("Meta+n");
  const [id] = await creates(page);
  if (!id) throw new Error("Missing create");
  await expect(title(page)).toHaveJSProperty("readOnly", true);
  await rows(page).getByRole("button", { name: "Alpha", exact: true }).click();
  await page.evaluate((id) => {
    history.pushState({}, "", `/page/${id}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, id);
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveCount(0);
  expect(await bodyCalls(page, id)).toEqual([]);
  const notice = page.getByText(
    "Titles can't be edited until pages can be refreshed.",
    { exact: true },
  );
  await expect(notice).toBeVisible();
  await expect(title(page)).toHaveAttribute(
    "aria-describedby",
    (await notice.getAttribute("id")) ?? "missing",
  );
});

test("TREE-1 an untitled created page with body text is never reused under stale reads", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(true));
  await page.keyboard.press("Meta+n");
  const [first] = await creates(page);
  if (!first) throw new Error("Missing create");
  await expect(page.getByRole("textbox", { name: "Page body" })).toBeVisible();
  await title(page).press("Enter");
  await page.keyboard.type("Meeting notes");
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Second page");
  const ids = await creates(page);
  expect(ids).toHaveLength(2);
  await expect(page).toHaveURL(`/page/${ids[1]}`);
  expect(
    await page.evaluate((id) => window.__aelomiMock?.bodies.text(id), first),
  ).toBe("Meeting notes");
  await expect(title(page)).toHaveValue("Second page");
});

test("TREE-1 leaving an accepted untitled failed create keeps its body gate", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await page.clock.install();
  await hold(page);
  await page.keyboard.press("Meta+n");
  const [id] = await creates(page);
  if (!id) throw new Error("Missing create");
  await page.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(true);
    window.__aelomiMock?.storage.refuseRecords(null);
  });
  await page.clock.runFor(6000);
  await page.evaluate(() => window.__aelomiMock?.storage.setStatus("Locked"));
  await page.clock.runFor(6000);
  await expect(title(page)).toHaveJSProperty("readOnly", true);
  await page.evaluate(() => {
    window.__aelomiMock?.storage.setStatus("Ready");
    window.__aelomiMock?.storage.failWrites(false);
    window.__aelomiMock?.workspace.failReads(false);
  });
  await page.clock.runFor(6000);
  await rows(page).getByRole("button", { name: "Alpha", exact: true }).click();
  await rows(page)
    .getByRole("button", { name: "Untitled", exact: true })
    .click();
  await expect(title(page)).toHaveJSProperty("readOnly", true);
  expect(await bodyCalls(page, id)).toEqual([]);
  await page.getByRole("button", { name: "Try again", exact: true }).click();
  await page.clock.runFor(1000);
  await expect.poll(() => bodyCalls(page, id)).toEqual(["docLoad"]);
});

test("TREE-9 open-page trash fallback waits for the palette to close", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await hold(page);
  await trash(page);
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("move");
  await release(page);
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
      ),
    )
    .toEqual(["Beta"]);
  await expect(page).toHaveURL("/page/a");
  await expect(page.getByRole("combobox")).toBeFocused();
  await expect(
    page.getByRole("option", { name: "Move to trash", exact: true }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL("/page/b");
  await expect(title(page)).toHaveValue("Beta");
});

test("SHELL-1 status messages never move sidebar rows and failed rows stay distinct", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  const alpha = rows(page).getByRole("button", { name: "Alpha", exact: true });
  const before = await alpha.boundingBox();
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" }),
  );
  await page.keyboard.press("Meta+n");
  await expect(page.locator(".shell-notice")).toHaveText(
    "A new page couldn't be created.",
  );
  expect(await alpha.boundingBox()).toEqual(before);
  const failed = rows(page).locator('[data-state="failed"]');
  await expect(failed.getByText("Not created", { exact: true })).toBeVisible();
  await expect(failed.getByRole("button").last()).toHaveCSS(
    "color",
    await alpha.evaluate((el) => getComputedStyle(el).color),
  );
});

test("TREE-7 a long failed palette title leaves its failure label fully visible", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(title(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  const full =
    "Quarterly planning https://example.com/platform-infrastructure/research/王秀英-👩🏽‍💻-نور";
  await title(page).fill(full);
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" }),
  );
  await expect(title(page)).toHaveJSProperty("readOnly", true);
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Quarterly");
  const option = page.getByRole("option", { name: full, exact: true });
  const suffix = option.getByText("Not created", { exact: true });
  await expect(suffix).toBeVisible();
  const box = await suffix.boundingBox();
  const row = await option.boundingBox();
  if (!box || !row) throw new Error("Missing palette result");
  expect(box.x + box.width).toBeLessThanOrEqual(row.x + row.width - 10);
});
