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
  expect(
    await browser.evaluate(() =>
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "k",
          code: "KeyK",
          metaKey: true,
          bubbles: true,
          cancelable: true,
        }),
      ),
    ),
  ).toBe(false);
  await expect(screen.getByRole("dialog", "Command palette")).toBeVisible();
  await browser.keyboard.press("Escape");
  await browser.goto(new URL("/?refused=1", await browser.url()).href);
  await expect(
    screen.getByText(
      'Finder must open "Aelomi" directly inside "/Users/test/Library/Application Support", with format.json and docs. Continue to step 4 only after this exact path opens with those files.',
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
      'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    screen.getByText(
      'Move the new window aside so you can see the window from step 1. Drag "Aelomi Notes" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac. Then rename it "Aelomi".',
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
    "/Users/test/Library/Application Support/Aelomi",
    "/Users/test/Library/Application Support",
  ]);
  await screen
    .getByRole(
      "textbox",
      "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it. Press Return:",
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
      "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
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
    await browser.evaluate(() =>
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "k",
          code: "KeyK",
          metaKey: true,
          bubbles: true,
          cancelable: true,
        }),
      ),
    ),
  ).toBe(false);
  const dialog = screen.getByRole("alertdialog");
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
      await browser.evaluate(() => {
        const steps = document.querySelectorAll(
          ".storage-refusal-recovery > li",
        );
        return {
          count: steps.length,
          restart: steps[steps.length - 1]?.textContent ?? null,
        };
      }),
    ).toEqual({
      count: variant === "Override" ? 2 : 3,
      restart: "Quit Aelomi and open it again.",
    });
    if (variant === "SharedLink") {
      await expect(
        screen.getByRole(
          "textbox",
          "This folder is a link into cloud storage:",
        ),
      ).toHaveValue("/Users/test/Library/Application Support");
      await expect(
        screen.getByText(
          "Aelomi can't keep its database there and hasn't changed anything.",
          { exact: true },
        ),
      ).toBeVisible();
      await expect(
        screen.getByText(
          "For Aelomi to open it, this folder must be a regular folder on this Mac. Turning off syncing for it in a cloud app can remove its files from this Mac, including other apps' files.",
          { exact: true },
        ),
      ).toBeVisible();
    }
  });
}

test("SHELL-9/SHELL-10: a ready workspace asks on drain timeout but a refused folder quits immediately", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      storageStatus: new URL(location.href).searchParams.has("refused")
        ? "Locked"
        : "Ready",
      pages: [{ id: "page-a", title: "A" }],
      bodies: { "page-a": [{ id: "block-a", content: ["Original page"] }] },
    };
  });
  await app.open("/page/page-a");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText(
    "Original page",
  );
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
  await browser.evaluate(() => {
    window.__aelomiMock?.quit.request();
    window.__aelomiMock?.quit.timeOut();
    return null;
  });
  await expect(dialog).toBeVisible();
  await screen.getByRole("button", "Quit anyway").click();
  expect(
    await browser.evaluate(() => ({
      exited: window.__aelomiMock?.quit.exited ?? null,
      confirmations:
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "quitAnyway",
        ).length ?? null,
    })),
  ).toEqual({ exited: true, confirmations: 1 });
  await browser.goto(new URL("/?refused=1", await browser.url()).href);
  await expect(
    screen.getByRole("heading", "This data folder is already open"),
  ).toBeVisible();
  await expect(screen.getByRole("status")).toContainText("IPC ok");
  expect(
    await browser.evaluate(() => {
      const mock = window.__aelomiMock;
      if (!mock) throw new Error("Missing IPC mock");
      const registrations = mock.calls.filter(
        (call) => call.command === "frontendReady",
      ).length;
      mock.quit.request();
      mock.quit.timeOut();
      return {
        registrations,
        exited: mock.quit.exited,
        phase: mock.quit.status.phase,
        completions: mock.calls.filter(
          (call) => call.command === "completeQuit",
        ).length,
      };
    }),
  ).toEqual({ registrations: 1, exited: true, phase: "Open", completions: 0 });
  await expect(dialog).not.toBeVisible();
});
