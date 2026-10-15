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
          configured: "/Users/test/Library/Application Support/Aelomi",
          resolved:
            "/Users/test/Library/Mobile Documents/com~apple~CloudDocs/Aelomi Notes",
          recovery: {
            LinkAtDataFolder: {
              configured_parent: "/Users/test/Library/Application Support",
              configured_name: "Aelomi",
              resolved_parent:
                "/Users/test/Library/Mobile Documents/com~apple~CloudDocs",
              resolved_name: "Aelomi Notes",
            },
          },
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
    screen.getByText("Cloud sync corrupts the database.", { exact: true }),
  ).toBeVisible();
  await expect(
    screen.getByRole("textbox", "Where your data really is"),
  ).toHaveValue(
    "/Users/test/Library/Mobile Documents/com~apple~CloudDocs/Aelomi Notes",
  );
  await expect(
    screen.getByText("Keep this window open until the last step.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages.',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'Drag "Aelomi Notes" into the window from step 1. Then rename it "Aelomi".',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText("Quit Aelomi and open it again.", { exact: true }),
  ).toBeVisible();
  await expect(
    screen.getByText("Nothing in this folder was changed.", { exact: true }),
  ).toBeVisible();
  expect(await browser.evaluate(() => document.activeElement?.id ?? null)).toBe(
    "storage-refusal-title",
  );
  expect(
    await browser.evaluate(() =>
      Array.from(document.querySelectorAll(".storage-refusal-path"), (chip) =>
        chip instanceof HTMLTextAreaElement ? chip.value : null,
      ),
    ),
  ).toEqual([
    "/Users/test/Library/Mobile Documents/com~apple~CloudDocs/Aelomi Notes",
    "/Users/test/Library/Application Support",
    "/Users/test/Library/Mobile Documents/com~apple~CloudDocs",
  ]);
  await screen
    .getByRole(
      "textbox",
      "In Finder, choose Go > Go to Folder (⇧⌘G) and paste:",
    )
    .click();
  expect(
    await browser.evaluate(() => {
      const chip = document.activeElement;
      return chip instanceof HTMLTextAreaElement
        ? chip.value.slice(chip.selectionStart, chip.selectionEnd)
        : null;
    }),
  ).toBe("/Users/test/Library/Application Support");
  await screen
    .getByRole(
      "textbox",
      "In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste:",
    )
    .focus();
  expect(
    await browser.evaluate(() => {
      const chip = document.activeElement;
      return chip instanceof HTMLTextAreaElement
        ? chip.value.slice(chip.selectionStart, chip.selectionEnd)
        : null;
    }),
  ).toBe("/Users/test/Library/Mobile Documents/com~apple~CloudDocs");
  expect(
    await browser.evaluate(() => ({
      editors: document.querySelectorAll('[data-testid="page-editor"]').length,
      loads:
        window.__aelomiMock?.calls.filter((call) => call.command === "docLoad")
          .length ?? null,
    })),
  ).toEqual({ editors: 0, loads: 0 });
});
