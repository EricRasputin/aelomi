import { expect, test } from "./fixtures";

const pages = [
  { id: "page-daily", title: "Daily" },
  { id: "page-target", title: "Target" },
  { id: "page-other", title: "Other" },
];

test.beforeEach(async ({ page }) => {
  await page.addInitScript((seeded) => {
    window.__aelomiMockSetup = {
      pages: seeded,
      bodies: {
        "page-daily": [
          {
            id: "block-a",
            content: [
              "see ",
              { link: "page-target" },
              " and ",
              { link: "page-other" },
            ],
          },
          { id: "block-b", content: ["again ", { link: "page-target" }] },
        ],
        "page-target": [{ id: "block-t", content: ["target"] }],
        "page-other": [{ id: "block-o", content: ["other"] }],
      },
    };
  }, pages);
});

async function linkLabels(page: import("@playwright/test").Page) {
  return page.evaluate(() => ({
    subscribed: window.__aelomiLinkLabels?.() ?? -1,
    shown: document.querySelectorAll('[data-testid="page-editor"] .page-link')
      .length,
  }));
}

test("LINK-1 copying and dragging page links leaves one label subscription per shown link", async ({
  page,
}) => {
  await page.goto("/page/page-daily");
  const body = page.getByRole("textbox", { name: "Page body" });
  await expect(body).toHaveText("see Target and Otheragain Target");
  expect(await linkLabels(page)).toEqual({ subscribed: 3, shown: 3 });

  await body.click();
  await page.evaluate(() => {
    addEventListener("copy", (event) => {
      document.body.dataset.copied = event.clipboardData?.getData("text/html");
    });
  });
  await page.keyboard.press("Meta+a");
  for (let i = 0; i < 5; i++) await page.keyboard.press("Meta+c");
  const copied = await page.evaluate(() => document.body.dataset.copied ?? "");
  expect(copied.match(/data-target-page-id="page-[a-z]+"/g)).toEqual([
    'data-target-page-id="page-target"',
    'data-target-page-id="page-other"',
    'data-target-page-id="page-target"',
  ]);
  expect(copied).toContain(">Target</span>");
  await page.evaluate(() => {
    const editor = document.querySelector('[data-testid="page-editor"]');
    for (let i = 0; i < 5; i++)
      editor?.dispatchEvent(
        new DragEvent("dragstart", {
          bubbles: true,
          cancelable: true,
          dataTransfer: new DataTransfer(),
        }),
      );
  });
  expect(await linkLabels(page)).toEqual({ subscribed: 3, shown: 3 });

  await page.evaluate((seeded) => {
    window.__aelomiMock?.workspace.seed(
      seeded.map((p) =>
        p.id === "page-target" ? { ...p, title: "Renamed" } : p,
      ),
    );
  }, pages);
  await expect(body).toHaveText("see Renamed and Otheragain Renamed");
  expect(await linkLabels(page)).toEqual({ subscribed: 3, shown: 3 });

  await page.goto("/page/page-other");
  await expect(page.getByRole("textbox", { name: "Page body" })).toHaveText(
    "other",
  );
  expect(await linkLabels(page)).toEqual({ subscribed: 0, shown: 0 });
});
