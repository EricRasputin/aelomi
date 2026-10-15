export default async function ping({ page, expect, shot }) {
  await page.goto("/dev/ipc");
  const status = page.getByRole("status");
  await expect(status).toHaveText("IPC ok · core 0.1.0-mock · seq 1");
  await shot("before");
  await page.getByRole("button", { name: "Ping again" }).click();
  await expect(status).toHaveText("IPC ok · core 0.1.0-mock · seq 2");
  await shot("after");
  const seqs = await page.evaluate(() =>
    window.__aelomiMock.calls.map((call) => call.args[0].seq),
  );
  expect(seqs, "ping calls the mock received").toEqual([1, 2]);
}
