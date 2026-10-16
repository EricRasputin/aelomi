export default async function settings({ page, expect, shot }) {
  await page.addInitScript(() => {
    const worst = new URLSearchParams(location.search).get("data") === "worst";
    const titles = [
      "Quarterly planning and international research coordination notes",
      "Benachrichtigungseinstellungen",
      "",
      "王秀英 👩🏽‍💻 Đặng Thị Ngọc Hân",
    ];
    window.__aelomiMockSetup = {
      appearance: "light",
      settings: { theme: worst ? { Unknown: "Future-high-contrast-appearance-王秀英-👩🏽‍💻" } : "System" },
      pages: worst
        ? Array.from({ length: 1000 }, (_, i) => ({
            id: `page-${i}`,
            title: titles[i % titles.length],
            parentId: i > 0 && i < 50 ? `page-${i - 1}` : null,
          }))
        : [{ id: "page-0", title: "Daily" }],
      bodies: { "page-0": [{ id: "block-plan", content: ["plan"] }] },
    };
    document.addEventListener("DOMContentLoaded", () => {
      const toggle = document.createElement("button");
      toggle.textContent = worst ? "Demo data" : "Worst case";
      toggle.setAttribute("aria-label", "Demo data / Worst case");
      toggle.style.cssText = "position:fixed;bottom:8px;left:50%;transform:translateX(-50%);padding:6px 12px;background:#eee;color:#222;border:1px solid #aaa;border-radius:8px;z-index:100";
      toggle.onclick = () => { location.search = worst ? "" : "?data=worst"; };
      document.body.append(toggle);
    });
  });
  await page.goto("/page/page-0");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText("plan");
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  await page.keyboard.press("Meta+,");
  await expect(dialog).toBeVisible();
  await shot("settings-demo-light");
  await dialog.getByRole("radio", { name: "Dark", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await shot("settings-demo-dark");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Demo data / Worst case" }).click();
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText("plan");
  await page.keyboard.press("Meta+,");
  await expect(dialog.getByRole("radio", { name: "System", exact: true })).toBeChecked();
  for (const width of [960, 320, 2560]) {
    await page.setViewportSize({ width, height: 640 });
    for (const theme of ["Light", "Dark"]) {
      await dialog.getByRole("radio", { name: theme, exact: true }).click();
      await expect(dialog).toBeInViewport();
      expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
      await shot(`settings-worst-${width}-${theme.toLowerCase()}`);
    }
  }
  await page.setViewportSize({ width: 480, height: 360 });
  await page.evaluate(() => { document.documentElement.style.fontSize = "32px"; });
  await dialog.getByRole("button", { name: "Close Settings" }).scrollIntoViewIfNeeded();
  await expect(dialog.getByRole("button", { name: "Close Settings" })).toBeInViewport();
  await dialog.getByRole("radio", { name: "Dark", exact: true }).scrollIntoViewIfNeeded();
  await expect(dialog.getByRole("radio", { name: "Dark", exact: true })).toBeInViewport();
  expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  await shot("settings-worst-large-text");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
}
