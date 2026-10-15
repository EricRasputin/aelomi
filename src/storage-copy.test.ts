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
test.each([
  {
    name: "link at the data folder",
    configured: "/Users/test/Library/Application Support/Aelomi",
    resolved: "/Users/test/Library/Mobile Documents/Aelomi",
    link: "/Users/test/Library/Application Support/Aelomi",
    steps: [
      'In Finder, open /Users/test/Library/Application Support with Go > Go to Folder (⇧⌘G). Drag "Aelomi" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.',
      "Move /Users/test/Library/Mobile Documents/Aelomi so its new path is /Users/test/Library/Application Support/Aelomi.",
      "Quit Aelomi and reopen it.",
    ],
  },
  {
    name: "link at a parent",
    configured: "/Users/test/Library/Support/Aelomi",
    resolved: "/Users/test/Library/CloudStorage/Dropbox/Aelomi",
    link: "/Users/test/Library/Support",
    steps: [
      'In Finder, open /Users/test/Library with Go > Go to Folder (⇧⌘G). Drag "Support" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.',
      'Create a regular folder named "Support" in /Users/test/Library. Recreate any other folders needed to reach /Users/test/Library/Support.',
      "Move /Users/test/Library/CloudStorage/Dropbox/Aelomi so its new path is /Users/test/Library/Support/Aelomi.",
      "Quit Aelomi and reopen it.",
    ],
  },
  {
    name: "direct override",
    configured: "/Users/test/Library/Mobile Documents/Aelomi",
    resolved: "/Users/test/Library/Mobile Documents/Aelomi",
    link: null,
    steps: [
      "Choose a local path on this Mac, outside cloud storage. Set AELOMI_DATA_DIR to that path.",
      "Move /Users/test/Library/Mobile Documents/Aelomi so its new path is the AELOMI_DATA_DIR path.",
      "Quit Aelomi and reopen it.",
    ],
  },
  {
    name: "debug build path",
    configured: "/Users/test/Library/Application Support/Aelomi Dev",
    resolved: "/Users/test/Library/Mobile Documents/Aelomi Dev",
    link: "/Users/test/Library/Application Support/Aelomi Dev",
    steps: [
      'In Finder, open /Users/test/Library/Application Support with Go > Go to Folder (⇧⌘G). Drag "Aelomi Dev" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.',
      "Move /Users/test/Library/Mobile Documents/Aelomi Dev so its new path is /Users/test/Library/Application Support/Aelomi Dev.",
      "Quit Aelomi and reopen it.",
    ],
  },
])("maps safe recovery steps for $name", ({ name: _name, steps, ...cloud }) => {
  expect(storageStartup({ CloudPath: cloud })).toEqual({
    kind: "refused",
    title: "Move your data folder out of cloud storage",
    message:
      "Cloud sync corrupts the database. Aelomi can't open this folder safely. Aelomi hasn't opened or changed anything in it.",
    detail: null,
    recovery: { configured: cloud.configured, resolved: cloud.resolved, steps },
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
