import { expect, test } from "./fixtures";

for (const snapshot of ["reject", "stale"] as const) {
  test(`SHELL-11: creation becomes ready before tree refresh and preserves the Ready event with a ${snapshot} snapshot`, async ({
    page,
  }) => {
    await page.addInitScript((snapshot) => {
      window.__aelomiMockSetup = {
        storageStatus: {
          FinishMoving: {
            destination_exists: false,
            configured: "/Local/Aelomi",
            resolved: "/Cloud/Aelomi Notes",
            recovery: "Override",
          },
        },
        recovery: { snapshot, holdTree: true, revealError: "Folder moved" },
      };
    }, snapshot);
    await page.goto("/");
    await expect(
      page.getByRole("heading", { name: "Finish moving your pages" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Show in Finder" }).click();
    await expect(page.getByRole("alert")).toHaveText(
      'Aelomi couldn\'t show that folder. Look inside each folder in "/Local" for "Aelomi Notes" or "Aelomi". Finder search skips Library folders by default.',
    );
    await page
      .getByRole("button", { name: "Start a new empty workspace here" })
      .click();
    const dialog = page.getByRole("alertdialog");
    await expect(
      dialog.getByText("The new empty workspace will be created at:", {
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("textbox", { name: "New workspace location" }),
    ).toHaveValue("/Local/Aelomi");
    await expect(
      dialog.getByRole("textbox", { name: "Where your pages were last seen" }),
    ).toHaveValue("/Cloud/Aelomi Notes");
    await page
      .getByRole("button", { name: "Create empty workspace", exact: true })
      .click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByText("Aelomi", { exact: true })).toBeVisible();
    await page.evaluate(() => window.__aelomiMock?.recovery.releaseTree());
    await page.keyboard.press("Meta+k");
    await page.getByRole("combobox").fill("missing");
    await expect(
      page.getByText("No page content matches.", { exact: true }),
    ).toBeVisible();
    const calls = await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) =>
          call.command === "indexStatus" ||
          call.command === "startEmptyWorkspace",
      ),
    );
    expect(calls).toEqual([
      { command: "indexStatus", args: [] },
      { command: "startEmptyWorkspace", args: [] },
    ]);
  });
}

test("SHELL-11: a lost recovery record still requires explicit empty-workspace confirmation", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: {
        RecoveryRecordUnreadable: { configured: "/Local/Aelomi" },
      },
    };
  });
  await page.goto("/");
  await expect(
    page.getByText(
      "Aelomi lost track of where your pages were. Find the folder you moved before you restart, or explicitly choose a new empty workspace.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("textbox", { name: "Aelomi expects your pages at:" }),
  ).toHaveValue("/Local/Aelomi");
  await expect(
    page.getByText("Quit Aelomi and open it again.", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Start a new empty workspace here" })
    .click();
  await expect(
    page
      .getByRole("alertdialog")
      .getByRole("textbox", { name: "New workspace location" }),
  ).toHaveValue("/Local/Aelomi");
  await page.getByRole("button", { name: "Keep moving my pages" }).click();
  await expect(
    page.getByRole("heading", { name: "Finish moving your pages" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Start a new empty workspace here" })
    .click();
  await page
    .getByRole("button", { name: "Create empty workspace", exact: true })
    .click();
  await expect(page.getByText("Aelomi", { exact: true })).toBeVisible();
});

test("SHELL-11: a non-empty destination keeps moving controls and names the blocking items", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: {
        FinishMoving: {
          destination_exists: true,
          configured: "/Local/Aelomi",
          resolved: "/Cloud/Aelomi Notes",
          recovery: {
            LinkAtDataFolder: {
              configured_parent: "/Local",
              configured_name: "Aelomi",
              resolved_parent: "/Cloud",
              resolved_name: "Aelomi Notes",
            },
          },
        },
      },
      recovery: {
        createError: {
          DestinationNotEmpty: { items: ["notes.txt", "Photos"] },
        },
      },
    };
  });
  await page.goto("/");
  await expect(
    page.getByText(
      'In "/Local", select the existing folder named "Aelomi", press Return, type "Aelomi (old)", and press Return. Keep this folder until your pages are back.',
      { exact: true },
    ),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Start a new empty workspace here" })
    .click();
  await page
    .getByRole("button", { name: "Create empty workspace", exact: true })
    .click();
  await expect(page.getByRole("alert")).toHaveText(
    "This folder already contains items: notes.txt, Photos. Aelomi can't start an empty workspace here. Nothing was changed.",
  );
  await page.getByRole("button", { name: "Keep moving my pages" }).click();
  await expect(
    page.getByRole("heading", { name: "Finish moving your pages" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Show in Finder" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Start a new empty workspace here" }),
  ).toBeVisible();
});
