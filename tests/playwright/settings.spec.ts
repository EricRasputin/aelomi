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
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-8 SHELL-10 latest theme pick wins after a transport outage", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "plan",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() => window.__aelomiMock?.settings.failTransport(100));
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alert")).toContainText(
    "Settings transport unavailable",
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Light", exact: true }).click();
  await page.evaluate(() => window.__aelomiMock?.settings.failTransport(0));
  await page.keyboard.press("Escape");
  await page.clock.runFor(200);
  await expect(page.getByRole("alert")).toBeHidden();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Light" }] },
    { command: "settingsUpdate", args: [{ theme: "Light" }] },
  ]);
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "Light",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
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

test("SHELL-8 Appearance waits for quit registration before accepting a pick", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { holdRegistration: true };
  });
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  const dark = page.getByRole("radio", { name: "Dark", exact: true });
  await expect(dark).toBeDisabled();
  await expect(
    page.getByText("Editing pauses while Aelomi connects to its shell."),
  ).toBeVisible();
  const choices = page.locator(".theme-choices");
  await expect(choices).toHaveCSS("opacity", "0.5");
  const label = page.locator(".theme-choice").filter({ has: dark });
  const idle = await label.evaluate((element) => ({
    color: getComputedStyle(element).color,
    background: getComputedStyle(element).backgroundColor,
  }));
  await label.hover();
  await page.mouse.down();
  expect(
    await label.evaluate((element) => ({
      color: getComputedStyle(element).color,
      background: getComputedStyle(element).backgroundColor,
    })),
  ).toEqual(idle);
  await page.mouse.up();
  await dark.click({ force: true });
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([]);
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "System",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
  await page.evaluate(() => window.__aelomiMock?.quit.releaseRegistration());
  await expect(dark).toBeEnabled();
  await expect(
    page.getByRole("radio", { name: "System", exact: true }),
  ).toBeFocused();
  await expect(choices).toHaveCSS("opacity", "1");
  await dark.click();
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.settings.get()))
    .toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: [],
      },
    });
});

test("SHELL-9 SHELL-10 an Io settings failure asks promptly with settings and retries through the next drain", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "plan",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() =>
    window.__aelomiMock?.settings.refuseUpdates({
      Io: "Settings read permission denied",
    }),
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alert")).toContainText(
    "Settings read permission denied",
  );
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await page.clock.runFor(1);
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("Settings changes have not been saved.");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "quitDrainFailed",
      ),
    ),
  ).toEqual([{ command: "quitDrainFailed", args: [1, [], true] }]);
  expect(await page.evaluate(() => window.__aelomiMock?.quit.exited)).toBe(
    false,
  );
  await page.getByRole("button", { name: "Keep app open" }).click();
  await page.evaluate(() => window.__aelomiMock?.settings.refuseUpdates(null));
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await page.clock.runFor(1);
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited))
    .toBe(true);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
  ]);
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-6 Close Window dismisses Settings and the palette before closing the main window", async ({
  page,
}) => {
  await page.goto("/");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("plan");
  await body.click();
  await page.keyboard.press("Meta+,");
  const settings = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(settings).toBeVisible();
  await page.keyboard.press("Meta+w");
  await expect(settings).toBeHidden();
  await expect(body).toBeFocused();
  expect(await page.evaluate(() => window.__aelomiMock?.quit.exited)).toBe(
    false,
  );
  await page.keyboard.press("Meta+,");
  await expect(settings).toBeVisible();
  await page.evaluate(() => window.__aelomiMock?.menu.activate("closeWindow"));
  await expect(settings).toBeHidden();
  await expect(body).toBeFocused();
  await page.keyboard.press("Meta+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await page.keyboard.press("Meta+w");
  await expect(palette).toBeHidden();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "closeWindow",
      ),
    ),
  ).toEqual([]);
  await page.keyboard.press("Meta+w");
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited))
    .toBe(true);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "closeWindow",
      ),
    ),
  ).toEqual([{ command: "closeWindow", args: [] }]);
});

test("SHELL-6 sidebar shortcuts resume after Settings closes", async ({
  page,
}) => {
  await page.goto("/");
  const sidebar = page.getByRole("navigation", { name: "Pages" });
  await expect(sidebar).toBeVisible();
  await page.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeHidden();
  await page.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeVisible();
  await page.keyboard.press("Meta+,");
  await expect(
    page.getByRole("dialog", { name: "Settings", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Meta+Backslash");
  await page.keyboard.press("Meta+n");
  await page.keyboard.press("Escape");
  await expect(sidebar).toBeVisible();
  await expect(
    page.getByRole("textbox", { name: "Page title", exact: true }),
  ).toHaveValue("Daily");
  await page.keyboard.press("Meta+Backslash");
  await expect(sidebar).toBeHidden();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map(({ title }) => title),
    ),
  ).toEqual(["Daily"]);
});

test("SHELL-10 Settings stays dimmed through Asking and Keep app open restores the selected radio", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "plan",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() =>
    window.__aelomiMock?.settings.refuseUpdates({ Io: "Settings read denied" }),
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await page.clock.runFor(1);
  await expect(page.getByRole("alertdialog")).toContainText(
    "Settings changes have not been saved.",
  );
  const settings = page.locator(".settings-dialog");
  await expect(settings).toBeVisible();
  await expect(settings.locator('input[value="Dark"]')).toBeDisabled();
  await expect(settings.locator(".theme-choices")).toHaveCSS("opacity", "0.5");
  await page.evaluate(() => window.__aelomiMock?.settings.refuseUpdates(null));
  await page.getByRole("button", { name: "Keep app open" }).click();
  const dark = page.getByRole("radio", { name: "Dark", exact: true });
  await expect(dark).toBeEnabled();
  await expect(dark).toBeFocused();
  await expect(dark).toBeChecked();
  await expect(settings.locator(".theme-choices")).toHaveCSS("opacity", "1");
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.settings.get()))
    .toEqual({
      status: "ok",
      data: {
        theme: "Dark",
        open_page: null,
        recent_pages: [],
        expanded_rows: [],
      },
    });
});

test("SHELL-5 theme switches leave bullet markers without animations", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "bullet-page", title: "Bullets" }],
      bodies: {
        "bullet-page": [
          { id: "bullet", type: "bulletListItem", content: ["A bullet"] },
        ],
      },
    };
  });
  await page.goto("/page/bullet-page");
  await expect(
    page.locator('.bn-block-content[data-content-type="bulletListItem"]'),
  ).toHaveText("A bullet");
  await page.keyboard.press("Meta+,");
  for (const name of ["Dark", "Light"]) {
    await page.getByRole("radio", { name, exact: true }).click();
    expect(await page.evaluate(() => document.getAnimations().length)).toBe(0);
  }
});

test("SHELL-5 a mouse-selected Appearance option has contrast without a focus ring in both themes", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { appearance: "dark" };
  });
  await page.goto("/");
  await page.keyboard.press("Meta+,");
  for (const [name, background, root, surface] of [
    ["Light", "rgb(107, 107, 115)", "rgb(250, 250, 250)", "rgb(255, 255, 255)"],
    ["Dark", "rgb(115, 115, 123)", "rgb(24, 24, 27)", "rgb(31, 31, 35)"],
  ] as const) {
    await page.getByRole("radio", { name, exact: true }).click();
    const colors = await page.evaluate(() => {
      const selected = document.querySelector(
        ".theme-choice:has(input:checked)",
      );
      const track = document.querySelector(".theme-choices");
      const dialog = document.querySelector(".settings-dialog");
      if (!selected || !track || !dialog)
        throw new Error("Appearance control missing");
      return {
        selected: getComputedStyle(selected).backgroundColor,
        track: getComputedStyle(track).backgroundColor,
        weight: getComputedStyle(selected).fontWeight,
        outline: getComputedStyle(selected).outlineStyle,
        root: getComputedStyle(document.documentElement).backgroundColor,
        surface: getComputedStyle(dialog).backgroundColor,
        dark: document.documentElement.classList.contains("dark"),
        scheme: getComputedStyle(document.documentElement).colorScheme,
      };
    });
    expect(colors).toEqual({
      selected: background,
      track: root,
      weight: "500",
      outline: "none",
      root,
      surface,
      dark: name === "Dark",
      scheme: name.toLowerCase(),
    });
    const luminance = (color: string) => {
      const channels = color.match(/\d+/g)?.map((value) => {
        const c = Number(value) / 255;
        return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      const [red, green, blue] = channels ?? [];
      if (red === undefined || green === undefined || blue === undefined)
        throw new Error("Invalid color");
      return red * 0.2126 + green * 0.7152 + blue * 0.0722;
    };
    const a = luminance(colors.selected);
    const b = luminance(colors.track);
    expect(
      (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05),
    ).toBeGreaterThanOrEqual(3);
  }
});

test("SHELL-10 Keep app open preserves the settings error while asking and retries at once", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "plan",
  );
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.evaluate(() =>
    window.__aelomiMock?.settings.refuseUpdates({ Io: "Settings read denied" }),
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.keyboard.press("Escape");
  const banner = page.getByRole("alert");
  await expect(banner).toContainText("Settings read denied");
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  await page.clock.runFor(1);
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.clock.runFor(20_000);
  await expect(page.locator(".save-failure-banner")).toContainText(
    "Settings read denied",
  );
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
  ]);
  await page.evaluate(() => window.__aelomiMock?.settings.refuseUpdates(null));
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(banner).toBeHidden();
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
  ]);
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "Dark",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
});

test("SHELL-8 terminal settings errors are dismissible and later theme picks save", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() =>
    window.__aelomiMock?.settings.refuseUpdates("Internal"),
  );
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Dark", exact: true }).click();
  await page.keyboard.press("Escape");
  const banner = page.getByRole("alert");
  await expect(banner).toContainText("A storage error prevented the write.");
  await expect(banner).not.toContainText("Aelomi will keep trying.");
  await page
    .getByRole("button", { name: "Dismiss failed settings change" })
    .click();
  await expect(banner).toBeHidden();
  await page.evaluate(() => window.__aelomiMock?.settings.refuseUpdates(null));
  await page.keyboard.press("Meta+,");
  await page.getByRole("radio", { name: "Light", exact: true }).click();
  expect(
    await page.evaluate(() => window.__aelomiMock?.settings.get()),
  ).toEqual({
    status: "ok",
    data: {
      theme: "Light",
      open_page: null,
      recent_pages: [],
      expanded_rows: [],
    },
  });
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "settingsUpdate",
      ),
    ),
  ).toEqual([
    { command: "settingsUpdate", args: [{ theme: "Dark" }] },
    { command: "settingsUpdate", args: [{ theme: "Light" }] },
  ]);
});

test("SHELL-7 refused storage keeps Close Window registered without workspace commands", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { storageStatus: "Locked" };
  });
  await page.goto("/");
  await expect(page.getByRole("heading")).toHaveText(
    "This data folder is already open",
  );
  for (const command of ["openPalette", "openSettings", "undo", "redo"]) {
    await page.evaluate(
      (command) => window.__aelomiMock?.menu.activate(command),
      command,
    );
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }
  await page.keyboard.press("Meta+w");
  await expect
    .poll(() => page.evaluate(() => window.__aelomiMock?.quit.exited))
    .toBe(true);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.calls.filter(
        (call) => call.command === "closeWindow",
      ),
    ),
  ).toEqual([{ command: "closeWindow", args: [] }]);
});
