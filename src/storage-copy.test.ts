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
    cloud: {
      configured: "/Users/test/Library/Application Support/Aelomi",
      resolved: "/Users/test/Library/Mobile Documents/Aelomi",
      recovery: {
        LinkAtDataFolder: {
          configured_parent: "/Users/test/Library/Application Support",
          configured_name: "Aelomi",
          resolved_parent: "/Users/test/Library/Mobile Documents",
          resolved_name: "Aelomi",
        },
      },
    },
    steps: [
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it:",
        path: "/Users/test/Library/Mobile Documents",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi" onto an empty area of the window from step 1, not onto a folder.',
      },
      {
        kind: "folder",
        text: "Choose Go > Go to Folder and paste:",
        path: "/Users/test/Library/Application Support/Aelomi",
        after:
          "Finder should open your data folder. If it can't find it, choose Edit > Undo in Finder and repeat step 2.",
      },
      { kind: "text", text: "Quit Aelomi and open it again." },
    ],
  },
  {
    name: "link with differing names",
    cloud: {
      configured: "/Users/test/Library/Application Support/Aelomi",
      resolved: "/Users/test/Library/CloudStorage/Dropbox/Apps/Aelomi Notes",
      recovery: {
        LinkAtDataFolder: {
          configured_parent: "/Users/test/Library/Application Support",
          configured_name: "Aelomi",
          resolved_parent: "/Users/test/Library/CloudStorage/Dropbox/Apps",
          resolved_name: "Aelomi Notes",
        },
      },
    },
    steps: [
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it:",
        path: "/Users/test/Library/CloudStorage/Dropbox/Apps",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi Notes" onto an empty area of the window from step 1, not onto a folder. Then rename it "Aelomi".',
      },
      {
        kind: "folder",
        text: "Choose Go > Go to Folder and paste:",
        path: "/Users/test/Library/Application Support/Aelomi",
        after:
          "Finder should open your data folder. If it can't find it, choose Edit > Undo in Finder and repeat step 2.",
      },
      { kind: "text", text: "Quit Aelomi and open it again." },
    ],
  },
  {
    name: "shared Application Support link",
    cloud: {
      configured: "/Users/test/Library/Application Support/Aelomi",
      resolved: "/Users/test/Library/CloudStorage/Dropbox/Support/Aelomi",
      recovery: {
        SharedLink: { link: "/Users/test/Library/Application Support" },
      },
    },
    steps: [
      {
        kind: "folder",
        text: "This folder is synced to cloud storage:",
        path: "/Users/test/Library/Application Support",
        after:
          "Other apps and files may use it. Aelomi can't keep its database there.",
      },
      {
        kind: "text",
        text: "In your cloud app's settings, turn off syncing for this folder so it is a regular folder on this Mac.",
      },
      { kind: "text", text: "Quit Aelomi and open it again." },
    ],
  },
  {
    name: "direct override",
    cloud: {
      configured: "/Users/test/Library/Mobile Documents/Aelomi",
      resolved: "/Users/test/Library/Mobile Documents/Aelomi",
      recovery: "Override",
    },
    steps: [
      {
        kind: "text",
        text: "Set AELOMI_DATA_DIR to a folder outside cloud storage, or unset it.",
      },
      { kind: "text", text: "Quit Aelomi and open it again." },
    ],
  },
  {
    name: "debug build path",
    cloud: {
      configured: "/Users/test/Library/Application Support/Aelomi Dev",
      resolved: "/Users/test/Library/Mobile Documents/Aelomi Dev",
      recovery: {
        LinkAtDataFolder: {
          configured_parent: "/Users/test/Library/Application Support",
          configured_name: "Aelomi Dev",
          resolved_parent: "/Users/test/Library/Mobile Documents",
          resolved_name: "Aelomi Dev",
        },
      },
    },
    steps: [
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi Dev". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it:",
        path: "/Users/test/Library/Mobile Documents",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi Dev" onto an empty area of the window from step 1, not onto a folder.',
      },
      {
        kind: "folder",
        text: "Choose Go > Go to Folder and paste:",
        path: "/Users/test/Library/Application Support/Aelomi Dev",
        after:
          "Finder should open your data folder. If it can't find it, choose Edit > Undo in Finder and repeat step 2.",
      },
      { kind: "text", text: "Quit Aelomi and open it again." },
    ],
  },
] as const)("maps safe recovery steps for $name", ({ cloud, steps }) => {
  expect(storageStartup({ CloudPath: cloud })).toEqual({
    kind: "refused",
    title: "Move your data folder out of cloud storage",
    message: "Cloud sync corrupts the database.",
    detail: null,
    recovery: { resolved: cloud.resolved, steps },
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
