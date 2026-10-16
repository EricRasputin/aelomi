import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-11 issue 92: finish moving keeps pages gated until empty-workspace confirmation", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: {
        FinishMoving: {
          destination_exists: false,
          configured: "/Users/test/Local/Aelomi",
          resolved: "/Users/test/Cloud/Aelomi Notes",
          recovery: {
            LinkAtDataFolder: {
              configured_parent: "/Users/test/Local",
              configured_name: "Aelomi",
              resolved_parent: "/Users/test/Cloud",
              resolved_name: "Aelomi Notes",
            },
          },
        },
      },
    };
  });
  await app.open("/");
  await expect(
    screen.getByRole("heading", "Finish moving your pages"),
  ).toBeVisible();
  await expect(
    screen.getByRole("textbox", "Where your pages were last seen"),
  ).toHaveValue("/Users/test/Cloud/Aelomi Notes");
  await screen.getByRole("button", "Show in Finder").click();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "showRecoveryFolder",
        ) ?? null,
    ),
  ).toEqual([{ command: "showRecoveryFolder", args: [] }]);
  await screen.getByRole("button", "Start a new empty workspace here").click();
  const dialog = screen.getByRole(
    "alertdialog",
    "Start a new empty workspace?",
  );
  await expect(dialog).toBeVisible();
  await expect(
    screen.getByText(
      "Your pages stay where they are. Aelomi won't move or delete them. The new workspace won't contain those pages.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByRole("textbox", "New workspace location"),
  ).toHaveValue("/Users/test/Local/Aelomi");
  expect(
    await browser.evaluate(() => document.activeElement?.textContent ?? null),
  ).toBe("Keep moving my pages");
  await screen.getByRole("button", "Keep moving my pages").click();
  await expect(dialog).not.toBeVisible();
  await expect(
    screen.getByRole("heading", "Finish moving your pages"),
  ).toBeVisible();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "startEmptyWorkspace",
        ).length ?? null,
    ),
  ).toBe(0);
  await screen.getByRole("button", "Start a new empty workspace here").click();
  await expect(dialog).toBeVisible();
  await screen.getByRole("button", "Create empty workspace").click();
  await expect(dialog).not.toBeVisible();
  await expect(
    screen.getByRole("heading", "Finish moving your pages"),
  ).not.toBeVisible();
  await expect(screen.getByText("Aelomi", { exact: true })).toBeVisible();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "startEmptyWorkspace",
        ) ?? null,
    ),
  ).toEqual([{ command: "startEmptyWorkspace", args: [] }]);
  await expect(screen.getByRole("heading", "No page open")).toBeVisible();
  expect(
    await browser.evaluate(
      () => document.querySelectorAll('[data-testid="page-editor"]').length,
    ),
  ).toBe(0);
});
