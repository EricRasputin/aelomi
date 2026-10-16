import type { commands, StorageError, StorageStatus } from "./bindings";

export type RecoverySetup = {
  snapshot?: "reject" | "stale";
  holdTree?: boolean;
  revealError?: string;
  createError?: StorageError;
};

type RecoveryCommands = Pick<
  typeof commands,
  "showRecoveryFolder" | "startEmptyWorkspace"
>;

export function createRecoveryMock(owner: {
  ready: () => Promise<void>;
  status: () => StorageStatus;
  setup: () => RecoverySetup | undefined;
  record: (command: keyof RecoveryCommands) => void;
  open: () => Promise<void>;
}) {
  let releaseTree: (() => void) | undefined;
  return {
    commands: {
      showRecoveryFolder: async () => {
        owner.record("showRecoveryFolder");
        await owner.ready();
        const error = owner.setup()?.revealError;
        if (error) return { status: "error", error: { Io: error } };
        const status = owner.status();
        return typeof status === "object" &&
          ("CloudPath" in status || "FinishMoving" in status)
          ? { status: "ok", data: null }
          : { status: "error", error: "Unavailable" };
      },
      startEmptyWorkspace: async () => {
        owner.record("startEmptyWorkspace");
        await owner.ready();
        const status = owner.status();
        if (
          typeof status !== "object" ||
          !("FinishMoving" in status || "RecoveryRecordUnreadable" in status)
        )
          return { status: "error", error: "Unavailable" };
        const error = owner.setup()?.createError;
        if (error) return { status: "error", error };
        await owner.open();
        return { status: "ok", data: "Ready" };
      },
    } satisfies RecoveryCommands,
    snapshot() {
      const snapshot = owner.setup()?.snapshot;
      if (snapshot === "reject") throw new Error("Index snapshot rejected");
      return snapshot === "stale" ? "Rebuilding" : null;
    },
    async waitForTree() {
      if (owner.status() === "Ready" && owner.setup()?.holdTree)
        await new Promise<void>((resolve) => {
          releaseTree = resolve;
        });
    },
    control: {
      releaseTree() {
        releaseTree?.();
      },
    },
  };
}
