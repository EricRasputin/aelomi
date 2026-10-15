import { expect } from "e2e";
import { test } from "./fixtures";

test("SHELL-13 acceptance 9: a URL opens in the default browser and the page stays shown", async ({
  app,
  browser,
  screen,
}) => {
  await browser.addInitScript(() => {
    window.__aelomiMockSetup = {
      pages: [{ id: "page-a", title: "A" }],
      bodies: {
        "page-a": [
          {
            id: "block-a",
            content: [{ text: "Example", url: "https://example.com" }],
          },
        ],
      },
    };
  });
  await app.open("/");
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
  await screen.getByRole("link", "Example", { exact: true }).click();
  expect(
    await browser.evaluate(
      () =>
        window.__aelomiMock?.calls.filter(
          (call) => call.command === "openUrl",
        ) ?? null,
    ),
  ).toEqual([{ command: "openUrl", args: ["https://example.com"] }]);
  await expect(screen.getByRole("heading", "A", { exact: true })).toBeVisible();
  await expect(screen.getByRole("textbox", "Page body")).toHaveText("Example");
});
