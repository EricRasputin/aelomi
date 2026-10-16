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
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it. Press Return:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/Mobile Documents",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/Application Support/Aelomi",
        after:
          'Finder must open "Aelomi" directly inside "/Users/test/Library/Application Support", with format.json and docs. Continue to step 4 only after this exact path opens with those files.\n\nIf Finder can\'t find it, close Go to Folder.',
        remedy: {
          text: "If the folder is elsewhere, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
          path: "/Users/test/Library/Application Support",
          after:
            "In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V). If you haven't moved the folder yet, repeat step 2. Repeat this path check before you restart Aelomi.",
        },
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
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it. Press Return:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/CloudStorage/Dropbox/Apps",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi Notes" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac. Then rename it "Aelomi".',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/Application Support/Aelomi",
        after:
          'Finder must open "Aelomi" directly inside "/Users/test/Library/Application Support", with format.json and docs. Continue to step 4 only after this exact path opens with those files.\n\nIf Finder can\'t find it, close Go to Folder. If the folder is named "Aelomi Notes" directly inside "/Users/test/Library/Application Support", select it, press Return, type "Aelomi", and press Return.',
        remedy: {
          text: "If the folder is elsewhere, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
          path: "/Users/test/Library/Application Support",
          after:
            'In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V). Select the moved folder, press Return, type "Aelomi", and press Return. If you haven\'t moved the folder yet, repeat step 2. Repeat this path check before you restart Aelomi.',
        },
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
        text: "This folder is a link into cloud storage:",
        path: "/Users/test/Library/Application Support",
        after:
          "Aelomi can't keep its database there and hasn't changed anything.",
      },
      {
        kind: "text",
        text: "For Aelomi to open it, this folder must be a regular folder on this Mac. Turning off syncing for it in a cloud app can remove its files from this Mac, including other apps' files.",
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
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it. Press Return:",
        path: "/Users/test/Library/Application Support",
        after:
          'Find "Aelomi Dev". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/Mobile Documents",
        after:
          'Move the new window aside so you can see the window from step 1. Drag "Aelomi Dev" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac.',
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
        path: "/Users/test/Library/Application Support/Aelomi Dev",
        after:
          'Finder must open "Aelomi Dev" directly inside "/Users/test/Library/Application Support", with format.json and docs. Continue to step 4 only after this exact path opens with those files.\n\nIf Finder can\'t find it, close Go to Folder.',
        remedy: {
          text: "If the folder is elsewhere, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
          path: "/Users/test/Library/Application Support",
          after:
            "In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V). If you haven't moved the folder yet, repeat step 2. Repeat this path check before you restart Aelomi.",
        },
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

test("maps a pending leaf refusal to only the remaining move steps", () => {
  expect(
    storageStartup({
      FinishMoving: {
        destination_exists: false,
        configured: "/Local/Aelomi",
        resolved: "/Cloud/Aelomi Notes",
        recovery: {
          LinkAtDataFolder: {
            configured_parent: "/Local",
            configured_name: "Aelomi",
            resolved_parent: "/Cloud",
            resolved_name: "Aelomi Notes",
          },
        },
      },
    }),
  ).toEqual({
    kind: "finish-moving",
    configured: "/Local/Aelomi",
    lastSeen: "/Cloud/Aelomi Notes",
    title: "Finish moving your pages",
    message:
      "Aelomi last found your pages in cloud storage. The destination has no workspace yet, so Aelomi hasn't opened a new one.",
    detail: null,
    recovery: {
      resolved: "/Cloud/Aelomi Notes",
      steps: [
        {
          kind: "show",
          text: 'Look inside each folder in "/Local" for "Aelomi Notes" or "Aelomi". Finder search skips Library folders by default.',
        },
        {
          kind: "folder",
          text: "Copy this destination path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
          path: "/Local",
          after:
            'In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V). Select the moved folder, press Return, type "Aelomi", and press Return. Your pages move to this Mac.',
        },
        {
          kind: "folder",
          text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
          path: "/Local/Aelomi",
          after:
            'Finder must open "Aelomi" directly inside "/Local", with format.json and docs. Continue to step 4 only after this exact path opens with those files.\n\nIf Finder can\'t find it, close Go to Folder. If the folder is named "Aelomi Notes" directly inside "/Local", select it, press Return, type "Aelomi", and press Return.',
          remedy: {
            text: "If the folder is elsewhere, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
            path: "/Local",
            after:
              'In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V). Select the moved folder, press Return, type "Aelomi", and press Return. If you haven\'t moved the folder yet, repeat step 2. Repeat this path check before you restart Aelomi.',
          },
        },
        { kind: "text", text: "Quit Aelomi and open it again." },
      ],
    },
  });
});

test.each(["Override", { SharedLink: { link: "/Local" } }] as const)(
  "maps pending parent and override refusals without link removal: %s",
  (recovery) => {
    const startup = storageStartup({
      FinishMoving: {
        destination_exists: false,
        configured: "/Local/Aelomi",
        resolved: "/Cloud/Aelomi",
        recovery,
      },
    });
    expect(startup.kind === "finish-moving" ? startup.recovery : null).toEqual({
      resolved: "/Cloud/Aelomi",
      steps: [
        {
          kind: "show",
          text: 'Look inside each folder in "/Local" for "Aelomi" or "Aelomi". Finder search skips Library folders by default.',
        },
        {
          kind: "folder",
          text: "Move the folder that holds your pages to this exact destination on this Mac. In Finder, use Go > Go to Folder to check it. Paste this path and press Return:",
          path: "/Local/Aelomi",
          after:
            "Continue only after this exact path opens with format.json and docs. If it doesn't, find the folder you moved and put it at this path. Check the path again before you restart Aelomi.",
        },
        { kind: "text", text: "Quit Aelomi and open it again." },
      ],
    });
  },
);

test("an unreadable record explains the lost location and keeps explicit creation available", () => {
  expect(
    storageStartup({
      RecoveryRecordUnreadable: { configured: "/Local/Aelomi" },
    }),
  ).toEqual({
    kind: "finish-moving",
    configured: "/Local/Aelomi",
    lastSeen: null,
    title: "Finish moving your pages",
    message:
      "Aelomi lost track of where your pages were. Find the folder you moved before you restart, or explicitly choose a new empty workspace.",
    detail: null,
    recovery: {
      resolved: null,
      steps: [
        {
          kind: "folder",
          text: "Aelomi expects your pages at:",
          path: "/Local/Aelomi",
          after:
            "Move the folder that contains your pages to this exact path. Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G), paste it, and press Return. Continue only after this exact path opens with format.json and docs.",
        },
        { kind: "text", text: "Quit Aelomi and open it again." },
      ],
    },
  });
});

test.each([false, true])(
  "destination exists %s drives the exact rename step",
  (exists) => {
    const status = {
      FinishMoving: {
        configured: "/Local/Aelomi",
        resolved: "/Cloud/Aelomi Notes",
        destination_exists: exists,
        recovery: {
          LinkAtDataFolder: {
            configured_parent: "/Local",
            configured_name: "Aelomi",
            resolved_parent: "/Cloud",
            resolved_name: "Aelomi Notes",
          },
        },
      },
    };
    const mapped = storageStartup(status);
    if (mapped.kind !== "finish-moving")
      throw new Error("expected pending move");
    expect(
      mapped.recovery?.steps.filter(
        (step) =>
          step.kind === "folder" && step.remedy?.after.includes("(old)"),
      ),
    ).toEqual(
      exists
        ? [
            {
              kind: "folder",
              text: "Before you rename anything, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
              path: "/Local/Aelomi",
              after:
                'Go to Folder must open "/Local/Aelomi". This is not the folder that Show in Finder selected. If this exact path does not open, stop this rename step and check the location.',
              remedy: {
                text: "Now copy this parent path. In the same Finder window, choose Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
                path: "/Local",
                after:
                  'In this window, select the existing folder named "Aelomi", press Return, type "Aelomi (old)", and press Return. Keep this folder until your pages are back.',
              },
            },
          ]
        : [],
    );
  },
);
