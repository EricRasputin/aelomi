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
          configured: "/Users/test/Library/Support/Aelomi Dev",
          resolved: "~/Library/Mobile Documents/com~apple~CloudDocs/Aelomi",
          link: "/Users/test/Library/Support",
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
    screen.getByText("/Users/test/Library/Support/Aelomi Dev", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    screen.getByText(
      "Keep this screen open while you move the folder in Finder.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'In Finder, open /Users/test/Library with Go > Go to Folder (⇧⌘G). Drag "Support" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      "If a folder already exists at the destination, Aelomi may have created it when it last opened. Move it aside, then move your data in.",
      { exact: true },
    ),
  ).toBeVisible();
  expect(
    await browser.evaluate(() =>
      Array.from(
        document.querySelectorAll(".storage-refusal li"),
        (item) => item.textContent,
      ),
    ),
  ).toEqual([
    'In Finder, open /Users/test/Library with Go > Go to Folder (⇧⌘G). Drag "Support" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.',
    'Create a regular folder named "Support" in /Users/test/Library. Recreate any other folders needed to reach /Users/test/Library/Support.',
    "Move ~/Library/Mobile Documents/com~apple~CloudDocs/Aelomi so its new path is /Users/test/Library/Support/Aelomi Dev.",
    "Quit Aelomi and reopen it.",
  ]);
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
