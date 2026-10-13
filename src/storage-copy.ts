import type { FailReason } from "./ipc";

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
  keepOpen: "Keep app open",
  quitAnyway: "Quit anyway",
  quitError: "Aelomi couldn't quit. Try again, or keep the app open.",
};

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
