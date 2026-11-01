export default async function tooltip({ page, expect, shot }) {
  await page.goto("/dev/ipc");
  await expect(page.getByRole("status", { name: "Connection status" })).toHaveText(
    "IPC ok · core 0.1.0-mock · seq 1",
  );
  await page.getByRole("button", { name: "Ping again" }).hover();
  await expect(page.getByRole("tooltip")).toHaveText(
    "Check the connection to the Rust core",
  );
  await shot("tooltip");
}
