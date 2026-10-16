import { expect, test } from "./fixtures";

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
        name: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it:",
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
          name: "Copy this path. In Finder, choose Go > Go to Folder and paste it:",
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
          'Finder should open a folder named "Aelomi" with your Aelomi files. If it can\'t find it, close Go to Folder. If the folder is in the window from step 1, rename it "Aelomi" again. If you moved it elsewhere, undo any rename first, then choose Edit > Undo Move of "Aelomi Notes" in Finder and repeat step 2. If you haven\'t moved it, repeat step 2.',
          { exact: true },
        ),
      ).toBeVisible();
    }
  });
}
