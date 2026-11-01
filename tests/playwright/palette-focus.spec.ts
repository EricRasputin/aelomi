import { expect, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [
        { id: "page-daily", title: "Daily" },
        { id: "page-other", title: "Other" },
      ],
      bodies: {
        "page-daily": [{ id: "block-plan", content: ["plan"] }],
        "page-other": [{ id: "block-other", content: ["other"] }],
      },
    };
  });
});

async function openDaily(page: import("@playwright/test").Page) {
  await page.goto("/page/page-daily");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("plan");
  await body.click();
  await page.keyboard.press("Meta+ArrowRight");
  return body;
}

const mainInert = (page: import("@playwright/test").Page) =>
  page.evaluate(
    () => document.querySelector("main")?.closest("[inert]") !== null,
  );

test("CMD-1 the key after ⌘K lands in the palette, and Escape returns it to the editor", async ({
  page,
}) => {
  const body = await openDaily(page);
  await page.keyboard.press("Meta+k");
  await page.keyboard.type("oth");
  await expect(
    page.getByRole("combobox", { name: "Search pages" }),
  ).toHaveValue("oth");
  expect(await mainInert(page)).toBe(true);
  await page.keyboard.press("Escape");
  await page.keyboard.type("x");
  await expect(body).toHaveText("planx");
  expect(await mainInert(page)).toBe(false);
});

test("CMD-7 choosing the open page returns focus to its editor", async ({
  page,
}) => {
  const body = await openDaily(page);
  await page.keyboard.press("Meta+k");
  await page.keyboard.type("daily");
  await expect(page.getByRole("option", { name: "Daily" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Command palette" }),
  ).toHaveCount(0);
  await page.keyboard.type("y");
  await expect(body).toHaveText("plany");
  expect(await mainInert(page)).toBe(false);
});

test("CMD-7 choosing another page focuses that page's editor", async ({
  page,
}) => {
  await openDaily(page);
  await page.keyboard.press("Meta+k");
  await page.keyboard.type("other");
  await expect(page.getByRole("option", { name: "Other" })).toBeVisible();
  await page.keyboard.press("Enter");
  const other = page.getByRole("textbox", { name: "Page body" });
  await expect(other).toHaveText("other");
  await expect(other).toBeFocused();
  expect(await mainInert(page)).toBe(false);
});
