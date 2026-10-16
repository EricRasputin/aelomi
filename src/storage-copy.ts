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
  | { kind: "folder"; text: string; path: string; after: string };

export type StorageStartup =
  | { kind: "loading" }
  | { kind: "ready" }
  | {
      kind: "refused";
      title: string;
      message: string;
      detail: string | null;
      recovery: {
        resolved: string;
        steps: readonly RecoveryStep[];
      } | null;
    };

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
    const renameRemedy =
      configured_name === resolved_name
        ? ""
        : ` If the folder is in the window from step 1, rename it "${configured_name}" again.`;
    const undoRename =
      configured_name === resolved_name ? "" : "undo any rename first, then ";
    steps = [
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder (⇧⌘G) and paste it:",
        path: configured_parent,
        after: `Find "${configured_name}". It has a small arrow because it is a link. Drag it to the Trash. This removes only the link, not your pages. Choose View > as Icons. Leave this Finder window open.`,
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose File > New Finder Window (⌘N). Then choose Go > Go to Folder and paste it:",
        path: resolved_parent,
        after: `Move the new window aside so you can see the window from step 1. Drag "${resolved_name}" onto an empty area of the window from step 1, not onto a folder. Finder or your cloud app may warn that the folder will be removed from iCloud Drive or the cloud app. Continue. Your pages move to this Mac.${rename}`,
      },
      {
        kind: "folder",
        text: "Copy this path. In Finder, choose Go > Go to Folder and paste it:",
        path: configured,
        after: `Finder should open a folder named "${configured_name}" with your Aelomi files. If it can't find it, close Go to Folder.${renameRemedy} If you moved it elsewhere, ${undoRename}choose Edit > Undo Move of "${resolved_name}" in Finder and repeat step 2. If you haven't moved it, repeat step 2.`,
      },
      { kind: "text", text: storageCopy.restart },
    ];
  }
  return { resolved, steps };
}

export function storageStartup(status: StorageStatus): StorageStartup {
  if (typeof status === "object") {
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
  if (typeof error === "object") return error.Io;
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
  if (error.Io !== undefined) return error.Io;
  if (error.Internal !== undefined) return error.Internal;
  return error.Malformed.reason;
}
