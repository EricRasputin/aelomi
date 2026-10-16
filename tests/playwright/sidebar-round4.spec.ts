import { expect, type Page, test } from "./fixtures";

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
const titleOf = (page: Page) =>
  page.getByRole("textbox", { name: "Page title", exact: true });
const rowsOf = (page: Page) => page.getByRole("navigation", { name: "Pages" });
async function tree(page: Page) {
  return page.evaluate(() =>
    window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
  );
}
async function createIds(page: Page) {
  return page.evaluate(() => [
    ...new Set(
      window.__aelomiMock?.calls
        .filter((c) => c.command === "pageCreate")
        .map((c) => String(c.args[1])),
    ),
  ]);
}
async function hold(page: Page) {
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Io: "held create" }),
  );
}
async function release(page: Page) {
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
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
async function newId(page: Page) {
  const id = (await createIds(page))[0];
  if (!id) throw new Error("No create ID");
  return id;
}
async function trash(page: Page) {
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Move to trash");
  await page.getByRole("option", { name: "Move to trash" }).click();
}

test("TREE-1 Command N focuses the real title synchronously and leaves the old body unchanged", async ({
  page,
}) => {
  await page.goto("/page/a");
  await page.getByRole("textbox", { name: "Page body" }).click();
  await hold(page);
  await page.evaluate(() => {
    const log: (string | undefined)[] = [];
    document.addEventListener("focusin", (e) => {
      if (e.target instanceof HTMLElement) log.push(e.target.dataset.testid);
    });
    Object.assign(window, { focusLog: log });
  });
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Q3 plan");
  const id = await newId(page);
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(titleOf(page)).toHaveValue("Q3 plan");
  expect(await page.evaluate(() => Reflect.get(window, "focusLog"))).toEqual([
    "page-title",
  ]);
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Original body",
  );
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Q3 plan"]);
});

test("SHELL-4 first Command N accepts typing before the body chunk loads", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [] };
  });
  await page.goto("/");
  await expect(
    page.getByRole("region", { name: "No page open" }),
  ).toBeVisible();
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("First");
  await expect(titleOf(page)).toBeFocused();
  await expect(titleOf(page)).toHaveValue("First");
  await expect.poll(() => tree(page)).toEqual(["First"]);
});

test("TREE-1 held page reopens by row palette and route with no body read or write", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Held");
  const id = await newId(page);
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(
    rowsOf(page).getByRole("button", { name: "Held", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "Changes aren't being saved",
  );
  await titleOf(page).press("Enter");
  await expect(titleOf(page)).toBeFocused();
  expect(await bodyCalls(page, id)).toEqual([]);
  await rowsOf(page)
    .getByRole("button", { name: "Alpha", exact: true })
    .click();
  await rowsOf(page).getByRole("button", { name: "Held", exact: true }).click();
  expect(await bodyCalls(page, id)).toEqual([]);
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Held");
  await page.getByRole("option", { name: "Held", exact: true }).click();
  expect(await bodyCalls(page, id)).toEqual([]);
  await page.evaluate((id) => {
    history.pushState({}, "", `/page/${id}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, id);
  await expect(page).toHaveURL(`/page/${id}`);
  expect(await bodyCalls(page, id)).toEqual([]);
  await titleOf(page).focus();
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Held"]);
  await expect.poll(() => bodyCalls(page, id)).toEqual(["docLoad"]);
  await expect(titleOf(page)).toBeFocused();
});

test("TREE-1 create acknowledgement preserves the caret and every title undo step", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Bufered");
  for (let i = 0; i < 4; i++) await page.keyboard.press("ArrowLeft");
  await page.keyboard.type("f");
  await expect(titleOf(page)).toHaveValue("Buffered");
  await expect(titleOf(page)).toHaveJSProperty("selectionStart", 4);
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Buffered"]);
  await expect(titleOf(page)).toHaveJSProperty("selectionStart", 4);
  const undone = ["Bufered", "Bufere", "Bufer", "Bufe", "Buf", "Bu", "B", ""];
  for (const value of undone) {
    await page.keyboard.press("Meta+z");
    await expect(titleOf(page)).toHaveValue(value);
  }
  for (let i = 0; i < 8; i++) await page.keyboard.press("Meta+Shift+z");
  await expect(titleOf(page)).toHaveValue("Buffered");
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Buffered"]);
});

test("TREE-1 two titled activations own distinct IDs through an outage", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("One");
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Two");
  const ids = await createIds(page);
  expect(ids).toHaveLength(1);
  const second = new URL(page.url()).pathname.split("/").at(-1);
  expect(second).not.toBe(ids[0]);
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "One", "Two"]);
  expect(await createIds(page)).toHaveLength(2);
  await expect(page).toHaveURL(`/page/${second}`);
});

test("TREE-1 repeated empty activations reuse one ID with stable New page controls", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [] };
  });
  await page.goto("/");
  await expect(
    page.getByRole("region", { name: "No page open" }),
  ).toBeVisible();
  await hold(page);
  await page
    .getByRole("region", { name: "No page open" })
    .getByRole("button", { name: "New page" })
    .click();
  for (let i = 0; i < 3; i++) {
    await page.getByRole("button", { name: "New page", exact: true }).click();
    await page.keyboard.press("Meta+n");
    await page.evaluate(() => window.__aelomiMock?.menu.activate("newPage"));
  }
  expect(await createIds(page)).toHaveLength(1);
  await expect(titleOf(page)).toBeFocused();
  await expect(page.getByText("Creating page…", { exact: true })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("button", { name: "New page", exact: true }),
  ).toBeEnabled();
  await release(page);
  await expect.poll(() => tree(page)).toEqual([""]);
});

test("TREE-1 stale reads keep the new title editable until one exact record is listed", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(true));
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Offline read");
  const id = await newId(page);
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(titleOf(page)).toBeEditable();
  await expect(
    rowsOf(page).getByRole("button", { name: "Offline read", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "New page", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(false));
  await expect
    .poll(() => tree(page))
    .toEqual(["Alpha", "Beta", "Offline read"]);
  await page.keyboard.press("Meta+n");
  expect(await createIds(page)).toHaveLength(2);
});

test("SHELL-10 held create asks immediately and retains Q during recovery in Asking", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Q");
  const id = await newId(page);
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page
    .locator(".sidebar-page")
    .filter({ hasText: "Beta" })
    .evaluate((el) => {
      if (el instanceof HTMLElement) el.click();
    });
  await expect(page).toHaveURL(`/page/${id}`);
  await page.keyboard.press("Meta+k");
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await expect(page).toHaveURL(`/page/${id}`);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls
        .filter((c) => c.command === "quitDrainFailed")
        .map((c) => c.args),
    ),
  ).toEqual([[1, ["workspace"]]]);
  expect(await page.evaluate(() => window.__aelomiMock?.quit.exited)).toBe(
    false,
  );
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Q"]);
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(titleOf(page)).toHaveValue("Q");
  await expect(
    rowsOf(page).getByRole("button", { name: "Q", exact: true }),
  ).toBeVisible();
});

test("SHELL-10 terminal create keeps its title read-only and retries the same ID by keyboard", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Kept");
  const id = await newId(page);
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" }),
  );
  const notice = page.getByText(
    "This page couldn't be created. Its title is kept here.",
    { exact: true },
  );
  await expect(notice).toBeVisible();
  await expect(titleOf(page)).toHaveValue("Kept");
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await expect(
    rowsOf(page).locator('.sidebar-row[data-state="failed"]'),
  ).toHaveCount(1);
  const description = await titleOf(page).getAttribute("aria-describedby");
  expect(description).toBe(await notice.getAttribute("id"));
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await expect(page.getByRole("alertdialog")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        window.__aelomiMock?.calls
          .filter((c) => c.command === "quitDrainFailed")
          .at(-1)?.args[1],
    ),
  ).toEqual(["workspace"]);
  await page.getByRole("button", { name: "Keep app open" }).click();
  await release(page);
  await titleOf(page).focus();
  await page.keyboard.press("Alt+Tab");
  await expect(
    page.getByRole("button", { name: "Try again", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Space");
  await expect(titleOf(page)).toBeFocused();
  await page.keyboard.type(" now");
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Kept now"]);
  expect(await createIds(page)).toEqual([id]);
});

test("TREE-1 leaving an empty failed page removes it and the next Command N owns focus", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" }),
  );
  await page.keyboard.press("Meta+n");
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await release(page);
  await rowsOf(page)
    .getByRole("button", { name: "Alpha", exact: true })
    .click();
  await page.getByRole("textbox", { name: "Page body" }).click();
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Retry");
  await expect(titleOf(page)).toBeFocused();
  expect(await page.evaluate(() => window.__aelomiMock?.bodies.text("a"))).toBe(
    "Original body",
  );
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Retry"]);
  await expect(rowsOf(page).locator('[data-state="failed"]')).toHaveCount(0);
});

test("CMD-6 held new page cannot be trashed behind the palette", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("New");
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("move");
  await expect(page.getByRole("option", { name: "Move to trash" })).toHaveCount(
    0,
  );
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "New"]);
  await expect(page.getByRole("combobox")).toBeFocused();
});

test("TREE-9 trash completion cannot move focus off a new title", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await trash(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("T");
  const path = new URL(page.url()).pathname;
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Beta", "T"]);
  await expect(page).toHaveURL(path);
  await expect(titleOf(page)).toBeFocused();
  await expect(titleOf(page)).toHaveValue("T");
});

test("TREE-1 a body mounted after a gated create never steals row focus", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Held");
  const row = rowsOf(page).getByRole("button", { name: "Beta", exact: true });
  await row.focus();
  await release(page);
  await expect(page.locator('[contenteditable="true"]')).toBeVisible();
  await expect(row).toBeFocused();
});

test("TREE-9 successful trash removes stale rows and disables stale palette results", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.evaluate(() => window.__aelomiMock?.workspace.failReads(true));
  await trash(page);
  await expect(page).toHaveURL("/page/b", { timeout: 2000 });
  await expect(
    rowsOf(page).getByRole("button", { name: "Alpha", exact: true }),
  ).toHaveCount(0);
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Alpha");
  await expect(
    page.getByRole("option", { name: "Alpha", exact: true }),
  ).toHaveAttribute("aria-disabled", "true");
});

test("SHELL-9 draining disables New page visibly with no press transform and keeps the status node", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  expect(await page.locator(".shell-notice").textContent()).toBe("");
  await page.evaluate(() => {
    Object.assign(window, {
      statusNode: document.querySelector(".shell-notice"),
    });
  });
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  const button = page.getByRole("button", { name: "New page", exact: true });
  await expect(button).toHaveAttribute("disabled", "");
  await expect(button).toHaveCSS("opacity", "0.5");
  const box = await button.boundingBox();
  if (!box) throw new Error("button missing");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await expect(button).toHaveCSS("transform", "none");
  await page.mouse.up();
  expect(
    await page.evaluate(
      () =>
        Reflect.get(window, "statusNode") ===
        document.querySelector(".shell-notice"),
    ),
  ).toBe(true);
  await expect(page.locator(".shell-notice")).toContainText(
    "Aelomi is closing",
  );
});

test("TREE-7 full truncated title is on the hovered row button", async ({
  page,
}) => {
  const full =
    "Quarterly planning notes for platform infrastructure and product design teams";
  await page.addInitScript((full) => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: full }] };
  }, full);
  await page.goto("/page/a");
  const row = rowsOf(page).getByRole("button", { name: full, exact: true });
  await row.hover();
  await expect(row).toHaveAttribute("title", full);
});

test("TREE-1 a long outage on A keeps Held gated through acceptance and durability", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "A" }] };
  });
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("A");
  await page.clock.install();
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Held");
  const id = await newId(page);
  await page.clock.runFor(15000);
  await expect(page).toHaveURL(`/page/${id}`);
  expect(await bodyCalls(page, id)).toEqual([]);
  await page.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(true);
    window.__aelomiMock?.storage.refuseRecords(null);
  });
  await page.clock.runFor(6000);
  expect(await bodyCalls(page, id)).toEqual([]);
  await expect(titleOf(page)).toHaveValue("Held");
  await page.evaluate(() => window.__aelomiMock?.storage.failWrites(false));
  await page.clock.runFor(6000);
  await expect.poll(() => tree(page)).toEqual(["A", "Held"]);
  await expect.poll(() => bodyCalls(page, id)).toEqual(["docLoad"]);
  await expect(titleOf(page)).toBeFocused();
});

test("CMD-6 final create and trash failures keep their own pages and retry targets", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords({ Internal: "terminal" }),
  );
  await page.keyboard.press("Meta+n");
  const id = await newId(page);
  await expect(page).toHaveURL(`/page/${id}`);
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await expect(page.locator(".shell-notice")).toHaveText(
    "A new page couldn't be created.",
  );
  await rowsOf(page)
    .getByRole("button", { name: "Alpha", exact: true })
    .click();
  await trash(page);
  await expect(page.locator(".shell-notice")).toHaveText(
    "This page couldn't be moved to trash. Try again.",
  );
  await expect(page).toHaveURL("/page/a");
  expect(await tree(page)).toEqual(["Alpha", "Beta"]);
  await hold(page);
  await trash(page);
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await release(page);
  await expect.poll(() => tree(page)).toEqual(["Beta"]);
  await expect(page).toHaveURL("/page/b");
});

test("SHELL-9 each sidebar and direct-route activation stays on the frozen page during Draining", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "Original body",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  const before = await page.evaluate(() =>
    window.__aelomiMock?.calls
      .filter((c) => c.command === "docLoad")
      .map((c) => c.args[0]),
  );
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  const beta = rowsOf(page).getByRole("button", { name: "Beta", exact: true });
  await expect(beta).toHaveAttribute("aria-disabled", "true");
  await beta.click({ force: true });
  await expect(page).toHaveURL("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls
        .filter((c) => c.command === "docLoad")
        .map((c) => c.args[0]),
    ),
  ).toEqual(before);
  await page.evaluate(() => {
    history.pushState({}, "", "/page/b");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page).toHaveURL("/page/a");
  await page.clock.runFor(0);
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
});

test("SHELL-9 palette activation stays blocked during Draining independently of the sidebar", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox").fill("Beta");
  const beta = page.getByRole("option", { name: "Beta", exact: true });
  await expect(beta).toBeVisible();
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await expect(beta).toHaveAttribute("aria-disabled", "true");
  await beta.click({ force: true });
  await expect(page).toHaveURL("/page/a");
  await expect(page.getByRole("combobox")).toBeVisible();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (c) => c.command === "docLoad" && c.args[0] === "b",
      ),
    ),
  ).toEqual([]);
});

test("SHELL-9 a title mounted during an existing drain starts frozen", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "Original body",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() => {
    history.pushState({}, "", "/page/b");
    window.dispatchEvent(new PopStateEvent("popstate"));
    queueMicrotask(() => window.__aelomiMock?.quit.request());
  });
  await expect(titleOf(page)).toHaveValue("Beta");
  expect(
    await page.evaluate(() => window.__aelomiMock?.quit.status.phase),
  ).toBe("Draining");
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await expect(page.locator(".page-notice")).toHaveText(
    "Aelomi is closing. New actions are paused while pending changes finish.",
  );
  await expect(titleOf(page)).toHaveAttribute(
    "aria-describedby",
    (await page.locator(".page-notice").getAttribute("id")) ?? "missing",
  );
  await titleOf(page).focus();
  await page.keyboard.type("Refused");
  expect(await tree(page)).toEqual(["Alpha", "Beta"]);
});

test("TREE-1 an accepted record cannot unlock a terminally failed create through a fresh tree", async ({
  page,
}) => {
  await page.goto("/page/a");
  await expect(titleOf(page)).toHaveValue("Alpha");
  await page.clock.install();
  await hold(page);
  await page.keyboard.press("Meta+n");
  await page.keyboard.type("Kept");
  const id = await newId(page);
  await page.evaluate(() => {
    window.__aelomiMock?.storage.failWrites(true);
    window.__aelomiMock?.storage.refuseRecords(null);
  });
  await page.clock.runFor(6000);
  expect(await tree(page)).toEqual(["Alpha", "Beta", ""]);
  await page.evaluate(() => window.__aelomiMock?.storage.setStatus("Locked"));
  await page.clock.runFor(6000);
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  await expect(titleOf(page)).toHaveValue("Kept");
  await page.evaluate(() => {
    window.__aelomiMock?.storage.setStatus("Ready");
    window.__aelomiMock?.storage.failWrites(false);
    window.__aelomiMock?.workspace.failReads(false);
  });
  await page.clock.runFor(6000);
  await expect(
    page.getByText("This page couldn't be created. Its title is kept here.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(titleOf(page)).toHaveJSProperty("readOnly", true);
  expect(await bodyCalls(page, id)).toEqual([]);
  await page.getByRole("button", { name: "Try again", exact: true }).click();
  await page.clock.runFor(1000);
  await expect.poll(() => tree(page)).toEqual(["Alpha", "Beta", "Kept"]);
  await expect.poll(() => bodyCalls(page, id)).toEqual(["docLoad"]);
});
