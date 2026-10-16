import { expect } from "e2e";
import { test } from "./fixtures";

test("Open Aelomi and see the core reply", async ({ app, screen }) => {
  await app.open("/");
  await expect(screen.getByRole("status", "Connection status")).toHaveText(
    "IPC ok · core 0.1.0-mock · seq 1",
  );
});
