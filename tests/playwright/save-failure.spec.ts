import { expect, test } from "./fixtures";

for (const colorScheme of ["light", "dark"] as const) {
  test(`SHELL-10 ${colorScheme} warnings wrap and preserve the quit decision after recovery`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    await page.setViewportSize({ width: 480, height: 720 });
    await page.goto("/");
    const reason =
      "A storage write failed. ".repeat(20) + "unbreakable".repeat(50);
    await page.evaluate(async (reason) => {
      const mock = window.__aelomiMock;
      if (!mock) throw new Error("Missing IPC mock");
      mock.storage.failWrites(true, { Io: reason });
      await mock.storage.type("page", "x");
    }, reason);
    const banner = page.getByRole("alert");
    await expect(banner).toContainText(reason);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    expect(
      await banner.evaluate(
        (element) => getComputedStyle(element).backgroundColor,
      ),
    ).toBe(colorScheme === "light" ? "rgb(255, 248, 235)" : "rgb(43, 37, 27)");
    await page.getByRole("button", { name: "Ping again" }).focus();
    await page.evaluate(() => window.__aelomiMock?.quit.request());
    const dialog = page.getByRole("alertdialog");
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAccessibleName("Quit with unsaved changes?");
    await expect(dialog).toHaveAccessibleDescription(
      "Some changes haven't been saved to disk. If you quit now, unsaved changes will be lost. Keep Aelomi open to keep trying.",
    );
    await expect(
      page.getByRole("button", { name: "Keep app open" }),
    ).toBeFocused();
    await page.mouse.click(4, 4);
    await expect(dialog).toBeVisible();
    await page.evaluate(() => window.__aelomiMock?.storage.failWrites(false));
    await expect(banner).not.toBeVisible();
    await expect(dialog).toContainText(
      "If you quit now, unsaved changes will be lost.",
    );
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(
      page.getByRole("button", { name: "Ping again" }),
    ).toBeFocused();
    expect(
      await page.evaluate(() => window.__aelomiMock?.storage.text("page")),
    ).toBe("x");
  });
}

test("SHELL-10 double confirmation sends one exit and preserves pending text", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(async () => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.failWrites(true);
    await mock.storage.type("page", "x");
    mock.quit.request();
    mock.quit.request();
  });
  const confirm = page.getByRole("button", { name: "Quit anyway" });
  await confirm.dblclick();
  await expect(confirm).toBeDisabled();
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  expect(
    await page.evaluate(() => ({
      exited: window.__aelomiMock?.quit.exited,
      confirmations: window.__aelomiMock?.calls.filter(
        (call) => call.command === "quitAnyway",
      ).length,
      text: window.__aelomiMock?.storage.text("page"),
    })),
  ).toEqual({ exited: true, confirmations: 1, text: "x" });
});
