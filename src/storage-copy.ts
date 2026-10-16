import type {
  FailReason,
  RecordCommandError,
  StorageError,
  StorageStatus,
} from "./ipc";

export const storageCopy = {
  banner: "Changes aren't being saved.",
  retry: "Your changes are kept in memory. Aelomi will keep trying.",
  diskFull: "The disk is full.",
  permissionDenied:
    "Aelomi doesn't have permission to write to its data folder.",
  readOnly: "The data folder is on a read-only disk.",
  storageError: "A storage error prevented the write.",
  quitTitle: "Quit with unsaved changes?",
  quitWarning:
    "Some changes haven't been saved to disk. If you quit now, unsaved changes will be lost. Keep Aelomi open to keep trying.",
  settingsPending: "Settings changes have not been saved.",
  keepOpen: "Keep app open",
  quitAnyway: "Quit anyway",
  quitError: "Aelomi couldn't quit. Try again, or keep the app open.",
  keepOpenError: "Aelomi couldn't record your choice. Try again.",
  notOpen: "The data folder isn't open.",
  refusedEdit: "The core refused an edit.",
  invalidDoc: "The page ID isn't valid.",
  opening: "Opening your data folder…",
  finishTitle: "Finish moving your pages",
  finishMessage:
    "Aelomi last found your pages in cloud storage. The destination has no workspace yet, so Aelomi hasn't opened a new one.",
  lastSeen: "Where your pages were last seen",
  expectedLocation: "Aelomi expects your pages at:",
  showFolder: "Show in Finder",
  showFolderError: "Aelomi couldn't show that folder.",
  startEmpty: "Start a new empty workspace here",
  startEmptyTitle: "Start a new empty workspace?",
  startEmptyWarning:
    "Your pages stay where they are. Aelomi won't move or delete them. The new workspace won't contain those pages.",
  newLocation: "The new empty workspace will be created at:",
  lostTrack:
    "Aelomi lost track of where your pages were. Find the folder you moved before you restart, or explicitly choose a new empty workspace.",
  locationForgotten:
    "Aelomi will forget this last-seen location after you continue.",
  cancelEmpty: "Keep moving my pages",
  confirmEmpty: "Create empty workspace",
  cloudTitle: "Move your data folder out of cloud storage",
  cloudRisk: "Cloud sync corrupts the database.",
  cloudDataPath: "Where your data really is",
  recoveryKeepOpen: "Keep Aelomi open until the last step.",
  details: "Details",
  unchanged: "Aelomi hasn't changed anything in your data folder.",
  restart: "Quit Aelomi and open it again.",
  override:
    "Set AELOMI_DATA_DIR to a folder outside cloud storage, or unset it.",
  startupTitle: "Aelomi couldn't start",
  startup: "Aelomi couldn't read its startup status. Quit and reopen it.",
  lockedTitle: "This data folder is already open",
  locked:
    "Another copy of Aelomi is using this folder. Quit both copies, then open Aelomi again.",
  newerTitle: "This data folder needs a newer Aelomi",
  newer:
    "A newer version of Aelomi created this folder. Open it with that version or a later one.",
  unknownTitle: "This data folder isn't recognized",
  unknown:
    "Aelomi can't read this folder's storage format. Its contents haven't been changed.",
  ioTitle: "Aelomi couldn't open its data folder",
  io: "The folder couldn't be read. Check that the disk is available and you have permission to open it.",
};

export type RecoveryStep =
  | { kind: "text"; text: string }
  | {
      kind: "folder";
      text: string;
      path: string;
      after: string;
      remedy?: { text: string; path: string; after: string };
    }
  | { kind: "show"; text: string };

type Refusal = {
  title: string;
  message: string;
  detail: string | null;
  recovery: { resolved: string | null; steps: readonly RecoveryStep[] } | null;
};

export type StorageStartup =
  | { kind: "loading" }
  | { kind: "ready" }
  | ({ kind: "refused" } & Refusal)
  | ({
      kind: "finish-moving";
      configured: string;
      lastSeen: string | null;
    } & Refusal);

function cloudRecovery(
  cloud: Extract<StorageStatus, { CloudPath: object }>["CloudPath"],
) {
  const { configured, resolved, recovery } = cloud;
  let steps: RecoveryStep[];
  if (recovery === "Override") {
    steps = [
      { kind: "text", text: storageCopy.override },
      { kind: "text", text: storageCopy.restart },
    ];
  } else if (recovery.SharedLink !== undefined) {
    steps = [
      {
        kind: "folder",
        text: "This folder is a link into cloud storage:",
        path: recovery.SharedLink.link,
        after:
          "Aelomi can't keep its database there and hasn't changed anything.",
      },
      {
        kind: "text",
        text: "For Aelomi to open it, this folder must be a regular folder on this Mac. Turning off syncing for it in a cloud app can remove its files from this Mac, including other apps' files.",
      },
      { kind: "text", text: storageCopy.restart },
    ];
  } else {
    const {
      configured_parent,
      configured_name,
      resolved_parent,
      resolved_name,
    } = recovery.LinkAtDataFolder;
    const rename =
      configured_name === resolved_name
        ? ""
        : ` Then rename it "${configured_name}".`;
    steps = [
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it. Press Return:",
        path: configured_parent,
        after: `Find "${configured_name}". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.`,
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it. Press Return:",
        path: resolved_parent,
        after: `Move the new window aside so you can see the window from step 1. Drag "${resolved_name}" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac.${rename}`,
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
        path: configured,
        ...configuredPathCheck(
          configured_parent,
          configured_name,
          resolved_name,
          2,
          4,
        ),
      },
      { kind: "text", text: storageCopy.restart },
    ];
  }
  return { resolved, steps };
}

function configuredPathCheck(
  parent: string,
  name: string,
  previousName: string,
  moveStep: number,
  restartStep: number,
): { after: string; remedy: { text: string; path: string; after: string } } {
  const rename =
    name === previousName
      ? ""
      : ` If the folder is named "${previousName}" directly inside "${parent}", select it, press Return, type "${name}", and press Return.`;
  const afterMove =
    name === previousName
      ? ""
      : ` Select the moved folder, press Return, type "${name}", and press Return.`;
  return {
    after: `Finder must open "${name}" directly inside "${parent}", with format.json and docs. Continue to step ${restartStep} only after this exact path opens with those files.\n\nIf Finder can't find it, close Go to Folder.${rename}`,
    remedy: {
      text: "If the folder is elsewhere, copy this path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
      path: parent,
      after: `In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V).${afterMove} If you haven't moved the folder yet, repeat step ${moveStep}. Repeat this path check before you restart Aelomi.`,
    },
  };
}

function lostFolderGuidance(
  pending: Extract<StorageStatus, { FinishMoving: object }>["FinishMoving"],
): string {
  const { configured, resolved, recovery } = pending;
  const parent = configured.slice(0, configured.lastIndexOf("/")) || "/";
  const configuredName = configured.slice(configured.lastIndexOf("/") + 1);
  const resolvedName = resolved.slice(resolved.lastIndexOf("/") + 1);
  const names =
    recovery !== "Override" && recovery.LinkAtDataFolder !== undefined
      ? recovery.LinkAtDataFolder
      : { configured_name: configuredName, resolved_name: resolvedName };
  return `Look inside each folder in "${parent}" for "${names.resolved_name}" or "${names.configured_name}". Finder search skips Library folders by default.`;
}

function finishRecovery(
  pending: Extract<StorageStatus, { FinishMoving: object }>["FinishMoving"],
) {
  const { configured, resolved, recovery, destination_exists } = pending;
  const steps: RecoveryStep[] = [
    {
      kind: "show",
      text: lostFolderGuidance(pending),
    },
  ];
  if (recovery !== "Override" && recovery.LinkAtDataFolder !== undefined) {
    const { configured_parent, configured_name, resolved_name } =
      recovery.LinkAtDataFolder;
    const rename =
      configured_name === resolved_name
        ? ""
        : ` Select the moved folder, press Return, type "${configured_name}", and press Return.`;
    if (destination_exists)
      steps.push({
        kind: "text",
        text: `In "${configured_parent}", select the existing folder named "${configured_name}", press Return, type "${configured_name} (old)", and press Return. Keep this folder until your pages are back.`,
      });
    steps.push(
      {
        kind: "folder",
        text: "Copy this destination path. In Finder, choose File > New Finder Window (⌘N), then Go > Go to Folder (⇧⌘G). Paste the path and press Return:",
        path: configured_parent,
        after: `In the other Finder window, select the folder that holds your pages and press ⌘C. Return to the destination window. Hold Option and choose Edit > Move Item Here (⌥⌘V).${rename} Your pages move to this Mac.`,
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it. Press Return:",
        path: configured,
        ...configuredPathCheck(
          configured_parent,
          configured_name,
          resolved_name,
          destination_exists ? 3 : 2,
          destination_exists ? 5 : 4,
        ),
      },
    );
  } else {
    if (destination_exists) {
      const parent = configured.slice(0, configured.lastIndexOf("/")) || "/";
      const name = configured.slice(configured.lastIndexOf("/") + 1);
      steps.push({
        kind: "text",
        text: `In "${parent}", select the existing folder named "${name}", press Return, type "${name} (old)", and press Return. Keep this folder until your pages are back.`,
      });
    }
    steps.push({
      kind: "folder",
      text: "Move the folder that holds your pages to this exact destination on this Mac. In Finder, use Go > Go to Folder to check it. Paste this path and press Return:",
      path: configured,
      after:
        "Continue only after this exact path opens with format.json and docs. If it doesn't, find the folder you moved and put it at this path. Check the path again before you restart Aelomi.",
    });
  }
  steps.push({ kind: "text", text: storageCopy.restart });
  return { resolved, steps };
}

export function storageStartup(status: StorageStatus): StorageStartup {
  if (typeof status === "object") {
    if (status.RecoveryRecordUnreadable !== undefined)
      return {
        kind: "finish-moving",
        configured: status.RecoveryRecordUnreadable.configured,
        lastSeen: null,
        title: storageCopy.finishTitle,
        message: storageCopy.lostTrack,
        detail: null,
        recovery: {
          resolved: null,
          steps: [
            {
              kind: "folder",
              text: storageCopy.expectedLocation,
              path: status.RecoveryRecordUnreadable.configured,
              after:
                "Move the folder that contains your pages to this exact path. Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G), paste it, and press Return. Continue only after this exact path opens with format.json and docs.",
            },
            { kind: "text", text: storageCopy.restart },
          ],
        },
      };
    if (status.FinishMoving !== undefined)
      return {
        kind: "finish-moving",
        configured: status.FinishMoving.configured,
        lastSeen: status.FinishMoving.resolved,
        title: storageCopy.finishTitle,
        message: storageCopy.finishMessage,
        detail: null,
        recovery: finishRecovery(status.FinishMoving),
      };
    if (status.CloudPath !== undefined)
      return {
        kind: "refused",
        title: storageCopy.cloudTitle,
        message: storageCopy.cloudRisk,
        detail: null,
        recovery: cloudRecovery(status.CloudPath),
      };
    return {
      kind: "refused",
      title: storageCopy.ioTitle,
      message: storageCopy.io,
      detail: status.Io,
      recovery: null,
    };
  }
  switch (status) {
    case "Ready":
      return { kind: "ready" };
    case "Locked":
      return {
        kind: "refused",
        title: storageCopy.lockedTitle,
        message: storageCopy.locked,
        detail: null,
        recovery: null,
      };
    case "NewerFormat":
      return {
        kind: "refused",
        title: storageCopy.newerTitle,
        message: storageCopy.newer,
        detail: null,
        recovery: null,
      };
    case "UnknownFormat":
      return {
        kind: "refused",
        title: storageCopy.unknownTitle,
        message: storageCopy.unknown,
        detail: null,
        recovery: null,
      };
    default: {
      const exhaustive: never = status;
      return exhaustive;
    }
  }
}

export function storageErrorText(error: StorageError): string {
  if (typeof error === "object") {
    if (error.DestinationNotEmpty !== undefined)
      return `This folder already contains items: ${error.DestinationNotEmpty.items.join(", ")}. Aelomi can't start an empty workspace here. Nothing was changed.`;
    return error.Io;
  }
  switch (error) {
    case "Unavailable":
      return storageCopy.notOpen;
    case "InvalidUpdate":
      return storageCopy.refusedEdit;
    case "InvalidDocId":
      return storageCopy.invalidDoc;
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

export function storageFailureReason(reason: FailReason): string {
  if (typeof reason === "object") return reason.Io;
  switch (reason) {
    case "DiskFull":
      return storageCopy.diskFull;
    case "PermissionDenied":
      return storageCopy.permissionDenied;
    case "ReadOnlyVolume":
      return storageCopy.readOnly;
    case "Internal":
      return storageCopy.storageError;
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

export function recordErrorText(error: RecordCommandError): string {
  if (typeof error === "string") return error;
  if ("Transport" in error) return error.Transport;
  if ("DestinationNotEmpty" in error) return storageErrorText(error);
  if (error.Io !== undefined) return error.Io;
  if (error.Internal !== undefined) return error.Internal;
  return error.Malformed.reason;
}
