import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-11 acceptance 7: a cloud folder shows its resolved path and how to move it back", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: {
        CloudPath: {
          configured: "/Users/test/Library/Application Support/Aelomi Dev",
          resolved: "~/Library/Mobile Documents/com~apple~CloudDocs/Aelomi",
        },
      },
      pages: [{ id: "page-a", title: "A" }],
    };
  });
  await app.open("/");
  await expect(
    screen.getByRole("heading", "Move your data folder out of cloud storage"),
  ).toBeVisible();
  await expect(
    screen.getByText("~/Library/Mobile Documents/com~apple~CloudDocs/Aelomi", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    screen.getByText("Cloud sync corrupts the database.", { exact: false }),
  ).toBeVisible();
  await expect(
    screen.getByText("/Users/test/Library/Application Support/Aelomi Dev", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    screen.getByText("Quit Aelomi (⌘Q).", { exact: true }),
  ).toBeVisible();
  await expect(
    screen.getByText("Reopen Aelomi.", { exact: true }),
  ).toBeVisible();
  await expect(
    screen.getByText(
      "In Finder, choose Go > Go to Folder (⇧⌘G) and paste a path.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      "Remove any symbolic link at the configured path or its parents first.",
      { exact: false },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText("Do not overwrite an existing folder.", { exact: false }),
  ).toBeVisible();
  expect(await browser.evaluate(() => document.activeElement?.id ?? null)).toBe(
    "storage-refusal-title",
  );
  expect(
    await browser.evaluate(() => ({
      editors: document.querySelectorAll('[data-testid="page-editor"]').length,
      loads:
        window.__aelomiMock?.calls.filter((call) => call.command === "docLoad")
          .length ?? null,
    })),
  ).toEqual({ editors: 0, loads: 0 });
});
