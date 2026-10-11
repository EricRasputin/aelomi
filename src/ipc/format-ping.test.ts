import { expect, test } from "vitest";
import { formatPingReply } from "./format-ping";

test("formats the core reply for the status line", () => {
  expect(formatPingReply({ seq: 1, core_version: "0.1.0" })).toBe(
    "IPC ok · core 0.1.0 · seq 1",
  );
});
