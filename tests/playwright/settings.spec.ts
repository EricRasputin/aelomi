import { expect, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      appearance: "light",
      pages: [{ id: "page-daily", title: "Daily" }],
      bodies: { "page-daily": [{ id: "block-plan", content: ["plan"] }] },
    };
  });
});

test("SHELL-6 SHELL-7 menu Settings replaces the palette and restores editor focus", async ({
  page,
}) => {
  await page.goto("/page/page-daily");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("plan");
  await body.click();
  await page.keyboard.press("Meta+ArrowRight");
  await page.keyboard.press("Meta+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await page.evaluate(() => window.__aelomiMock?.menu.activate("openSettings"));
  const settings = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(settings).toBeVisible();
  await expect(palette).toBeHidden();
  await page.keyboard.press("Meta+,");
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await page.keyboard.press("Meta+k");
  await expect(palette).toBeHidden();
  await settings.getByRole("button", { name: "Close Settings" }).click();
  await expect(settings).toBeHidden();
  await expect(body).toBeFocused();
  await page.keyboard.insertText("x");
  await expect(body).toHaveText("planx");
  await page.keyboard.press("Meta+,");
  await expect(settings).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();
  await expect(body).toBeFocused();
  await page.keyboard.insertText("y");
  await expect(body).toHaveText("planxy");
});

test("SHELL-5 SHELL-8 keyboard theme picks apply at once and send only theme patches", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  const system = page.getByRole("radio", { name: "System", exact: true });
  await expect(system).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("radio", { name: "Light", exact: true }),
  ).toBeChecked();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("radio", { name: "Dark", exact: true }),
  ).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "settingsUpdate",
        ),
      ),
    )
    .toEqual([
      { command: "settingsUpdate", args: [{ theme: "Light" }] },
      { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    ]);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Meta+,");
  await expect(
    page.getByRole("radio", { name: "Dark", exact: true }),
  ).toBeChecked();
});

test("SHELL-5 unknown theme follows System and is preserved until an explicit pick", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      appearance: "dark",
      settings: { theme: { Unknown: "Sepia" } },
    };
  });
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  await expect(
    page.getByRole("radio", { name: "System", exact: true }),
  ).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([]);
  await page.getByRole("radio", { name: "System", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "settingsUpdate",
        ),
      ),
    )
    .toEqual([{ command: "settingsUpdate", args: [{ theme: "System" }] }]);
});

test("SHELL-8 SHELL-10 failed settings write uses the storage banner and recovers", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      failWrites: { Io: "Settings disk unavailable" },
    };
  });
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alert")).toContainText(
    "Settings disk unavailable",
  );
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([{ command: "settingsUpdate", args: [{ theme: "Dark" }] }]);
  await page.evaluate(() => window.__aelomiMock?.storage.failWrites(false));
  await expect(page.getByRole("alert")).toBeHidden();
  await page.keyboard.press("Meta+,");
  await expect(
    page.getByRole("radio", { name: "Dark", exact: true }),
  ).toBeChecked();
});

test("SHELL-8 SHELL-10 retains theme picks across transport failures in order", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => window.__aelomiMock?.settings.failTransport(100));
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alert")).toContainText(
    "Settings transport unavailable",
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Light", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.evaluate(() => window.__aelomiMock?.settings.failTransport(0));
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alert")).toBeHidden();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__aelomiMock?.calls
          .filter((call) => call.command === "settingsUpdate")
          .slice(-2),
      ),
    )
    .toEqual([
      { command: "settingsUpdate", args: [{ theme: "Dark" }] },
      { command: "settingsUpdate", args: [{ theme: "Light" }] },
    ]);
});

test("SHELL-5 explicit choices ignore system changes and System resumes live media changes", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.emulateMedia({ colorScheme: "dark" });
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("radio", { name: "System", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

test("SHELL-6 Settings keeps focus when the initial page finishes loading", async ({
  page,
}) => {
  let releaseChunk = () => {};
  const chunkReady = new Promise<void>((resolve) => {
    releaseChunk = resolve;
  });
  await page.route("**/assets/page-view-*.js", async (route) => {
    await chunkReady;
    await route.continue();
  });
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.keyboard.press("Meta+,");
  const system = page.getByRole("radio", { name: "System", exact: true });
  await expect(system).toBeFocused();
  releaseChunk();
  await expect(page.locator('[aria-label="Page body"]')).toHaveText("plan");
  await expect(system).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("radio", { name: "Light", exact: true }),
  ).toBeChecked();
});
