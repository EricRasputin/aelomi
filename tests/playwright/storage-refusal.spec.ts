import { expect, test } from "./fixtures";

for (const status of [
  "CloudPath",
  "FinishMoving",
  "RecoveryRecordUnreadable",
] as const) {
  test(`SHELL-5 SHELL-11 ${status} uses explicit Dark tokens on a light system`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await page.addInitScript((status) => {
      window.__aelomiInitialTheme = "Dark";
      const location = {
        configured: "/synthetic/local/Aelomi",
        resolved: "/synthetic/cloud/Aelomi",
        recovery: "Override" as const,
      };
      window.__aelomiMockSetup = {
        appearance: "light",
        storageStatus:
          status === "CloudPath"
            ? { CloudPath: location }
            : status === "FinishMoving"
              ? { FinishMoving: { ...location, destination_exists: false } }
              : {
                  RecoveryRecordUnreadable: { configured: location.configured },
                },
      };
    }, status);
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1 })).toHaveText(
      status === "CloudPath"
        ? "Move your data folder out of cloud storage"
        : "Finish moving your pages",
    );
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await expect(page.locator("html")).toHaveCSS(
      "background-color",
      "rgb(24, 24, 27)",
    );
    const chip = page.locator(".storage-refusal-path").first();
    await expect(chip).toHaveCSS("background-color", "rgb(31, 31, 35)");
    await expect(chip).toHaveCSS("color", "rgb(237, 237, 240)");
    await expect(chip).toHaveCSS("border-color", "rgb(48, 48, 54)");
    if (status !== "CloudPath") {
      await page
        .getByRole("button", { name: "Start a new empty workspace here" })
        .click();
      const dialog = page.getByRole("alertdialog");
      await expect(dialog).toBeVisible();
      await expect(dialog).toHaveCSS("background-color", "rgb(24, 24, 27)");
      await expect(dialog).toHaveCSS("color", "rgb(237, 237, 240)");
      await page.getByRole("button", { name: "Keep moving my pages" }).click();
      await expect(dialog).toBeHidden();
    }
  });
}

test("SHELL-5 SHELL-11 recovery loads stored Dark after the System fallback", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      appearance: "light",
      settings: { theme: "Dark" },
      storageStatus: {
        FinishMoving: {
          configured: "/synthetic/local/Aelomi",
          resolved: "/synthetic/cloud/Aelomi",
          recovery: "Override",
          destination_exists: false,
        },
      },
    };
  });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Finish moving your pages" }),
  ).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({ status: "error", error: "Unavailable" });
  await page
    .getByRole("button", { name: "Start a new empty workspace here" })
    .click();
  await page
    .getByRole("button", { name: "Create empty workspace", exact: true })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.keyboard.press("Meta+,");
  await expect(
    page.getByRole("dialog", { name: "Settings", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("radio", { name: "Dark", exact: true }),
  ).toBeChecked();
});

for (const colorScheme of ["light", "dark"] as const) {
  test(`SHELL-11 ${colorScheme} recovery chips wrap and select without automatic field sizing`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    await page.setViewportSize({ width: 320, height: 640 });
    await page.addInitScript(() => {
      window.__aelomiMockSetup = {
        storageStatus: {
          CloudPath: {
            configured: "/Users/test/Library/Application Support/Aelomi",
            resolved:
              "/Users/test/Library/CloudStorage/Dropbox/International-Collaboration-Archive/研究とメモ🦊/Aelomi Notes",
            recovery: {
              LinkAtDataFolder: {
                configured_parent: "/Users/test/Library/Application Support",
                configured_name: "Aelomi",
                resolved_parent:
                  "/Users/test/Library/CloudStorage/Dropbox/International-Collaboration-Archive/研究とメモ🦊",
                resolved_name: "Aelomi Notes",
              },
            },
          },
        },
      };
    });
    await page.goto("/");
    await expect(
      page.getByRole("heading", {
        name: "Move your data folder out of cloud storage",
      }),
    ).toBeVisible();
    await expect(
      page.getByText("Keep Aelomi open until the last step.", { exact: true }),
    ).toBeVisible();
    await page.addStyleTag({
      content: ".storage-refusal-path { field-sizing: fixed !important; }",
    });
    for (const fontSize of ["100%", "200%"]) {
      await page.evaluate((size) => {
        document.documentElement.style.fontSize = size;
      }, fontSize);
      expect(
        await page.evaluate(() => ({
          overflow:
            document.documentElement.scrollWidth >
            document.documentElement.clientWidth,
          clipped: Array.from(document.querySelectorAll("textarea"))
            .filter(
              (chip) =>
                chip.scrollHeight > chip.clientHeight + 1 ||
                chip.scrollWidth > chip.clientWidth + 1,
            )
            .map((chip) => chip.value),
        })),
      ).toEqual({ overflow: false, clipped: [] });
      const chip = page.getByRole("textbox", {
        name: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
      });
      await chip.click();
      expect(
        await chip.evaluate((element) =>
          element instanceof HTMLTextAreaElement
            ? element.value.slice(element.selectionStart, element.selectionEnd)
            : null,
        ),
      ).toBe(
        "/Users/test/Library/CloudStorage/Dropbox/International-Collaboration-Archive/研究とメモ🦊",
      );
      await expect(
        page.getByRole("textbox", {
          name: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
          exact: true,
        }),
      ).toHaveValue("/Users/test/Library/Application Support/Aelomi");
      await page
        .getByText("Aelomi hasn't changed anything in your data folder.", {
          exact: true,
        })
        .scrollIntoViewIfNeeded();
      await expect(
        page.getByText("Quit Aelomi and open it again.", { exact: true }),
      ).toBeVisible();
      await expect(
        page.getByText(
          'Finder must open "Aelomi" directly inside "/Users/test/Library/Application Support", with format.json and docs. Continue to step 4 only after this exact path opens with those files.',
          { exact: true },
        ),
      ).toBeVisible();
    }
  });
}
