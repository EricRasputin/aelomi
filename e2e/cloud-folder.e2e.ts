import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-11 acceptance 7: a cloud folder shows its resolved path and how to move it back", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: new URL(location.href).searchParams.has("refused")
        ? {
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
          }
        : "Ready",
      pages: [{ id: "page-a", title: "A" }],
      bodies: { "page-a": [{ id: "block-a", content: ["Original page"] }] },
    };
  });
  await app.open("/page/page-a");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText(
    "Original page",
  );
  await browser.keyboard.press("Meta+k");
  await expect(screen.getByRole("dialog", "Command palette")).toBeVisible();
  await browser.keyboard.press("Escape");
  await browser.goto(new URL("/?refused=1", await browser.url()).href);
  await expect(
    screen.getByText(
      'Check that "Aelomi" is now in the window from step 1, without an arrow.',
      { exact: true },
    ),
  ).toBeVisible();
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
    screen.getByText("Keep Aelomi open until the last step.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Leave this Finder window open.',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'Move the new window so the window from step 1 shows. Drag "Aelomi Notes" onto an empty area of the window from step 1, not onto a folder. Then rename it "Aelomi".',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText("Quit Aelomi and open it again.", { exact: true }),
  ).toBeVisible();
  await expect(
    screen.getByText("Aelomi hasn't changed anything in your data folder.", {
      exact: true,
    }),
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
      "In Finder, choose Go > Go to Folder (⇧⌘G) and copy and paste:",
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
      "In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and copy and paste:",
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
  await browser.evaluate(() => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.quit.request();
    mock.quit.timeOut();
    return null;
  });
  const dialog = screen.getByRole("alertdialog");
  await expect(dialog).toContainText("Quit with unsaved changes?");
  await expect(screen.getByRole("button", "Keep app open")).toBeFocused();
  await screen.getByRole("button", "Keep app open").click();
  await expect(dialog).not.toBeVisible();
  await browser.keyboard.press("Meta+k");
  await expect(screen.getByRole("dialog", "Command palette")).not.toBeVisible();
  await expect(dialog).not.toBeVisible();
  await browser.evaluate(() => {
    window.__aelomiMock?.menu.activate("openPalette");
    return null;
  });
  await expect(screen.getByRole("dialog", "Command palette")).not.toBeVisible();
  await expect(dialog).not.toBeVisible();
  expect(
    await browser.evaluate(() => ({
      editors: document.querySelectorAll('[data-testid="page-editor"]').length,
      loads:
        window.__aelomiMock?.calls.filter((call) => call.command === "docLoad")
          .length ?? null,
    })),
  ).toEqual({ editors: 0, loads: 0 });
  expect(
    await browser.evaluate(() => ({
      exited: window.__aelomiMock?.quit.exited ?? null,
      phase: window.__aelomiMock?.quit.status.phase ?? null,
    })),
  ).toEqual({ exited: false, phase: "Open" });
  await browser.evaluate(() => {
    window.__aelomiMock?.quit.request();
    window.__aelomiMock?.quit.timeOut();
    return null;
  });
  await expect(dialog).toBeVisible();
  await screen.getByRole("button", "Quit anyway").click();
  expect(
    await browser.evaluate(() => window.__aelomiMock?.quit.exited ?? null),
  ).toBe(true);
});

for (const variant of ["SharedLink", "Override"] as const) {
  test(`SHELL-11 acceptance 7: ${variant} keeps Aelomi open until the separate restart step`, async ({
    app,
    browser,
    screen,
  }) => {
    await browser.addInitScript((recoveryVariant) => {
      window.__aelomiMockSetup = {
        storageStatus: {
          CloudPath: {
            configured: "/Users/test/Library/Application Support/Aelomi",
            resolved: "/Users/test/Library/CloudStorage/Support/Aelomi",
            recovery:
              recoveryVariant === "Override"
                ? "Override"
                : {
                    SharedLink: {
                      link: "/Users/test/Library/Application Support",
                    },
                  },
          },
        },
      };
    }, variant);
    await app.open("/");
    await expect(
      screen.getByText("Keep Aelomi open until the last step.", {
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      screen.getByText("Aelomi hasn't changed anything in your data folder.", {
        exact: true,
      }),
    ).toBeVisible();
    expect(
      await browser.evaluate(() =>
        Array.from(
          document.querySelectorAll(".storage-refusal-recovery > li"),
          (step) => step.textContent,
        ),
      ),
    ).toEqual(
      variant === "Override"
        ? [
            "Set AELOMI_DATA_DIR to a folder outside cloud storage, or unset it.",
            "Quit Aelomi and open it again.",
          ]
        : [
            "This folder is synced to cloud storage:/Users/test/Library/Application SupportOther apps and files may use it. Aelomi can't keep its database there.",
            "In your cloud app's settings, turn off syncing for this folder so it is a regular folder on this Mac.",
            "Quit Aelomi and open it again.",
          ],
    );
  });
}
