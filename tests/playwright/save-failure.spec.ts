import { contrastRatio, type Rgb } from "../contrast";
import { expect, test } from "./fixtures";

type RenderedText = {
  text: string;
  fill: Rgb;
  background: Rgb;
  visible: boolean;
  inBanner: boolean;
};

function readRenderedText(banner: Element): RenderedText[] {
  const context = document
    .createElement("canvas")
    .getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("No 2D canvas context");
  const toRgba = (color: string) => {
    context.clearRect(0, 0, 1, 1);
    context.fillStyle = color;
    context.fillRect(0, 0, 1, 1);
    const [red = 0, green = 0, blue = 0, alpha = 0] = context.getImageData(
      0,
      0,
      1,
      1,
    ).data;
    const rgb: Rgb = [red, green, blue];
    return { rgb, alpha };
  };
  const opaque = (color: string, label: string): Rgb => {
    const { rgb, alpha } = toRgba(color);
    if (alpha !== 255) throw new Error(`${label} is translucent: ${color}`);
    return rgb;
  };
  const backdrop = (element: Element, label: string): Rgb => {
    for (
      let layer: Element | null = element;
      layer;
      layer = layer.parentElement
    ) {
      const { backgroundColor, backgroundImage } = getComputedStyle(layer);
      if (backgroundImage !== "none") {
        throw new Error(`${label} sits on a background image`);
      }
      if (toRgba(backgroundColor).alpha > 0) {
        return opaque(backgroundColor, `${label} background`);
      }
    }
    throw new Error(`${label} has no background`);
  };
  const box = banner.getBoundingClientRect();
  const texts: RenderedText[] = [];
  const walker = document.createTreeWalker(banner, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.textContent?.trim();
    const element = node.parentElement;
    if (!text || !element) continue;
    const label = `"${text.slice(0, 40)}"`;
    for (
      let layer: Element | null = element;
      layer;
      layer = layer.parentElement
    ) {
      const { opacity } = getComputedStyle(layer);
      if (Number(opacity) < 1) {
        throw new Error(`${label} is faded by opacity ${opacity}`);
      }
    }
    const style = getComputedStyle(element);
    const fill = style.getPropertyValue("-webkit-text-fill-color");
    if (!fill) throw new Error(`${label} has no computed text fill`);
    const range = document.createRange();
    range.selectNodeContents(node);
    const rects = [...range.getClientRects()];
    texts.push({
      text,
      fill: opaque(fill, `${label} fill`),
      background: backdrop(element, label),
      visible:
        style.visibility === "visible" &&
        rects.some((rect) => rect.width > 0 && rect.height > 0),
      inBanner: rects.every(
        (rect) => rect.left >= box.left && rect.right <= box.right,
      ),
    });
  }
  return texts;
}

for (const colorScheme of ["light", "dark"] as const) {
  test(`SHELL-10 ${colorScheme} warnings wrap and preserve the quit decision after recovery`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    await page.setViewportSize({ width: 480, height: 720 });
    await page.goto("/dev/ipc");
    await expect(page.locator("html")).toHaveAttribute(
      "data-theme",
      colorScheme,
    );
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
    await expect(banner).toBeVisible();
    const texts = await banner.evaluate(readRenderedText);
    expect(texts.map(({ text }) => text)).toEqual([
      "Changes aren't being saved.",
      reason,
      "Your changes are kept in memory. Aelomi will keep trying.",
    ]);
    for (const { text, fill, background, visible, inBanner } of texts) {
      const label = `${colorScheme} "${text.slice(0, 27)}"`;
      expect(visible, `${label} is visible`).toBe(true);
      expect(inBanner, `${label} wraps inside the banner`).toBe(true);
      expect(
        contrastRatio(fill, background),
        `${label} meets the WCAG AA text contrast`,
      ).toBeGreaterThanOrEqual(4.5);
    }
    const opposite = colorScheme === "light" ? "dark" : "light";
    await page.emulateMedia({ colorScheme: opposite });
    await expect(page.locator("html")).toHaveAttribute("data-theme", opposite);
    const otherTheme = await banner.evaluate(readRenderedText);
    expect(
      otherTheme.map(({ background }) => background),
      `${colorScheme} warning background changes with the theme`,
    ).not.toEqual(texts.map(({ background }) => background));
    await page.emulateMedia({ colorScheme });
    await expect(page.locator("html")).toHaveAttribute(
      "data-theme",
      colorScheme,
    );
    expect(
      (await banner.evaluate(readRenderedText)).map(
        ({ background }) => background,
      ),
    ).toEqual(texts.map(({ background }) => background));
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
  await page.goto("/dev/ipc");
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

test("SHELL-10 names pending settings in quit confirmation and retries after recovery", async ({
  page,
}) => {
  await page.goto("/dev/ipc");
  await page.evaluate(async () => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.failWrites(true, "PermissionDenied");
    await mock.settings.update({ theme: "Dark" });
  });
  await expect(page.getByRole("alert")).toContainText(
    "Changes aren't being saved.",
  );
  await page.evaluate(() => window.__aelomiMock?.quit.request());
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("Settings changes have not been saved.");
  await page.getByRole("button", { name: "Keep app open" }).click();
  await expect(dialog).toBeHidden();
  await page.evaluate(() => window.__aelomiMock?.storage.failWrites(false));
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(await page.evaluate(() => window.__aelomiMock?.quit.exited)).toBe(
    false,
  );
});

test("SHELL-10 a pre-acceptance rename failure shows the banner with healthy storage and clears on recovery", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = { pages: [{ id: "a", title: "Before" }] };
  });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Before", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => {
    const mock = window.__aelomiMock;
    if (!mock) throw new Error("Missing IPC mock");
    mock.storage.refuseRecords({ Io: "record read failed" });
    void mock.workspace.rename("a", "Recovered");
  });
  await expect(page.getByRole("alert")).toContainText("record read failed");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Before"]);
  await page.evaluate(() => window.__aelomiMock?.storage.refuseRecords(null));
  await expect(
    page.getByRole("heading", { name: "Recovered", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Recovered"]);
});

test("SHELL-10 a terminal retry failure stays visible until dismissed", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "a", title: "Before" },
        { id: "b", title: "Other" },
      ],
    };
  });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Before", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__aelomiMock?.storage.refuseRecords({ Io: "read failed" });
    void window.__aelomiMock?.workspace.rename("a", "Rejected");
  });
  await expect(page.getByRole("alert")).toContainText("read failed");
  await page.evaluate(() =>
    window.__aelomiMock?.storage.refuseRecords("NotActive"),
  );
  await expect(
    page.getByRole("button", { name: "Dismiss failed change" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText("NotActive");
  await page.evaluate(async () => {
    window.__aelomiMock?.storage.refuseRecords(null);
    await window.__aelomiMock?.workspace.rename("b", "Next");
  });
  await expect(page.getByRole("alert")).toContainText("NotActive");
  expect(
    await page.evaluate(() =>
      window.__aelomiMock?.workspace.tree().pages.map((p) => p.title),
    ),
  ).toEqual(["Before", "Next"]);
  await page.getByRole("button", { name: "Dismiss failed change" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
});
