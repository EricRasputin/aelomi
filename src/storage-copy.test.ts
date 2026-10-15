import { expect, test } from "vitest";
import { storageFailureReason, storageStartup } from "./storage-copy";

test.each([
  ["DiskFull", "The disk is full."],
  [
    "PermissionDenied",
    "Aelomi doesn't have permission to write to its data folder.",
  ],
  ["ReadOnlyVolume", "The data folder is on a read-only disk."],
  ["Internal", "A storage error prevented the write."],
] as const)("explains %s", (reason, expected) => {
  expect(storageFailureReason(reason)).toBe(expected);
});

test("keeps the complete storage IO reason", () => {
  const reason = `Write failed for a local folder. ${"detail ".repeat(100)}`;
  expect(storageFailureReason({ Io: reason })).toBe(reason);
});
test("maps a cloud refusal to its resolved path and move instructions", () => {
  expect(
    storageStartup({
      CloudPath: {
        configured: "/Users/test/Library/Application Support/Aelomi Dev",
        resolved: "~/Library/Mobile Documents/Aelomi",
      },
    }),
  ).toEqual({
    kind: "refused",
    title: "Move your data folder out of cloud storage",
    message:
      "Cloud sync corrupts the database. Aelomi can't open this folder safely. Aelomi hasn't opened or changed anything in it.",
    detail: "~/Library/Mobile Documents/Aelomi",
    recovery: {
      configured: "/Users/test/Library/Application Support/Aelomi Dev",
      resolved: "~/Library/Mobile Documents/Aelomi",
    },
  });
});

test("maps every other startup outcome to a plain message", () => {
  expect([
    storageStartup("Ready"),
    storageStartup("Locked"),
    storageStartup("NewerFormat"),
    storageStartup("UnknownFormat"),
    storageStartup({ Io: "Permission denied" }),
  ]).toEqual([
    { kind: "ready" },
    {
      kind: "refused",
      title: "This data folder is already open",
      message:
        "Another copy of Aelomi is using this folder. Quit both copies, then open Aelomi again.",
      detail: null,
      recovery: null,
    },
    {
      kind: "refused",
      title: "This data folder needs a newer Aelomi",
      message:
        "A newer version of Aelomi created this folder. Open it with that version or a later one.",
      detail: null,
      recovery: null,
    },
    {
      kind: "refused",
      title: "This data folder isn't recognized",
      message:
        "Aelomi can't read this folder's storage format. Its contents haven't been changed.",
      detail: null,
      recovery: null,
    },
    {
      kind: "refused",
      title: "Aelomi couldn't open its data folder",
      message:
        "The folder couldn't be read. Check that the disk is available and you have permission to open it.",
      detail: "Permission denied",
      recovery: null,
    },
  ]);
});
