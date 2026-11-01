import { expect, test } from "./fixtures";

test("the connection tooltip appears on hover and the trigger still pings", async ({
  page,
}) => {
  await page.goto("/dev/ipc");
  const ping = page.getByRole("button", { name: "Ping again" });
  await expect(
    page.getByRole("status", { name: "Connection status" }),
  ).toHaveText("IPC ok · core 0.1.0-mock · seq 1");
  await ping.hover();
  const tooltip = page.getByRole("tooltip");
  await expect(tooltip).toHaveText("Check the connection to the Rust core");
  await expect(ping).toHaveAccessibleDescription(
    "Check the connection to the Rust core",
  );
  await page.mouse.move(4, 4);
  await expect(tooltip).not.toBeVisible();
  await ping.click();
  await expect(
    page.getByRole("status", { name: "Connection status" }),
  ).toHaveText("IPC ok · core 0.1.0-mock · seq 2");
});
