import { expect, test } from "./fixtures";

test("SHELL-13 opens the app without non-local requests", async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByRole("status", { name: "Connection status" }),
  ).toHaveText("IPC ok · core 0.1.0-mock · seq 1");
});
