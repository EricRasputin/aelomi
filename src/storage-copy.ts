import type { FailReason, StorageError, StorageStatus } from "./ipc";

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
  cloudRisk:
    "Cloud sync corrupts the database. Aelomi can't open this folder safely. Aelomi hasn't opened or changed anything in it.",
  configuredPath: "Configured data folder",
  cloudDataPath: "Cloud data folder",
  details: "Details",
  finder: "Keep this screen open while you move the folder in Finder.",
  moveTitle: "Move it back to this Mac",
  occupiedDestination:
    "If a folder already exists at the destination, Aelomi may have created it when it last opened. Move it aside, then move your data in.",
  restart: "Quit Aelomi and reopen it.",
  override:
    "Choose a local path on this Mac, outside cloud storage. Set AELOMI_DATA_DIR to that path.",
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

export type StorageStartup =
  | { kind: "loading" }
  | { kind: "ready" }
  | {
      kind: "refused";
      title: string;
      message: string;
      detail: string | null;
      recovery: {
        configured: string;
        resolved: string;
        steps: readonly string[];
      } | null;
    };

function cloudRecovery(
  cloud: Extract<StorageStatus, { CloudPath: object }>["CloudPath"],
) {
  const { configured, resolved, link } = cloud;
  const steps: string[] = [];
  if (link !== null) {
    const slash = link.lastIndexOf("/");
    const parent = link.slice(0, slash) || "/";
    const name = link.slice(slash + 1);
    steps.push(
      `In Finder, open ${parent} with Go > Go to Folder (⇧⌘G). Drag "${name}" (it has an arrow badge) to the Trash. The cloud folder it points to stays where it is.`,
    );
    if (link !== configured) {
      const destinationParent = configured.slice(
        0,
        configured.lastIndexOf("/"),
      );
      steps.push(
        `Create a regular folder named "${name}" in ${parent}. Recreate any other folders needed to reach ${destinationParent}.`,
      );
    }
    steps.push(`Move ${resolved} so its new path is ${configured}.`);
  } else {
    steps.push(
      storageCopy.override,
      `Move ${resolved} so its new path is the AELOMI_DATA_DIR path.`,
    );
  }
  steps.push(storageCopy.restart);
  return { configured, resolved, steps };
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
