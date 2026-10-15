import { create } from "zustand";
import { commands, type QuitStatus } from "../ipc";
import { storageCopy } from "../storage-copy";

/** Why the app is quitting: a drain that found nothing pending, or the user's Quit anyway. */
export type QuitOrigin = "drained" | "userConfirmed";

export type QuitDecision =
  | { kind: "open"; revision: number }
  | { kind: "draining"; revision: number }
  | {
      kind: "asking";
      revision: number;
      pendingDocs: string[];
      pendingSettings: boolean;
      error: string | null;
    }
  | {
      kind: "quitting";
      revision: number;
      pendingDocs: string[];
      pendingSettings: boolean;
      origin: QuitOrigin;
    };

type QuitStore = {
  decision: QuitDecision;
  /** `frontendReady` succeeded; only an opened workspace registers for drains. */
  registered: boolean;
  receiveQuit: (status: QuitStatus) => void;
  register: () => void;
  keepOpen: () => Promise<void>;
  quitAnyway: () => Promise<void>;
};

function decide(status: QuitStatus, current: QuitDecision): QuitDecision {
  const { revision, phase } = status;
  if (current.kind === "quitting" || phase === "Confirmed")
    return {
      kind: "quitting",
      revision,
      pendingDocs:
        typeof phase === "object"
          ? phase.Asking.pending_docs
          : current.kind === "quitting" || current.kind === "asking"
            ? current.pendingDocs
            : [],
      pendingSettings:
        typeof phase === "object"
          ? phase.Asking.pending_settings
          : current.kind === "quitting" || current.kind === "asking"
            ? current.pendingSettings
            : false,
      origin:
        current.kind === "asking" ||
        (current.kind === "quitting" && current.origin === "userConfirmed")
          ? "userConfirmed"
          : "drained",
    };
  switch (phase) {
    case "Open":
      return { kind: "open", revision };
    case "Draining":
      return { kind: "draining", revision };
    default:
      return {
        kind: "asking",
        revision,
        pendingDocs: phase.Asking.pending_docs,
        pendingSettings: phase.Asking.pending_settings,
        error: null,
      };
  }
}

export const useQuitStore = create<QuitStore>((set, get) => ({
  decision: { kind: "open", revision: 0 },
  registered: false,
  receiveQuit: (status) => {
    const decision = get().decision;
    if (status.revision <= decision.revision) return;
    set({ decision: decide(status, decision) });
  },
  register: () => set({ registered: true }),
  keepOpen: async () => {
    const decision = get().decision;
    if (decision.kind !== "asking") return;
    const { revision } = decision;
    set({ decision: { kind: "open", revision } });
    try {
      get().receiveQuit(await commands.keepOpen(revision));
    } catch {
      const current = get().decision;
      if (current.kind === "open" && current.revision === revision) {
        set({ decision: { ...decision, error: storageCopy.keepOpenError } });
      }
    }
  },
  quitAnyway: async () => {
    const decision = get().decision;
    if (decision.kind !== "asking") return;
    set({
      decision: {
        kind: "quitting",
        revision: decision.revision,
        pendingDocs: decision.pendingDocs,
        pendingSettings: decision.pendingSettings,
        origin: "userConfirmed",
      },
    });
    try {
      await commands.quitAnyway();
    } catch {
      const current = get().decision;
      if (current.kind !== "quitting") return;
      set({
        decision: {
          kind: "asking",
          revision: current.revision,
          pendingDocs: current.pendingDocs,
          pendingSettings: current.pendingSettings,
          error: storageCopy.quitError,
        },
      });
    }
  },
}));

/** Editing waits for the quit listener's registration and stops while a quit drains. */
export function canEdit(state: Pick<QuitStore, "decision" | "registered">) {
  return state.registered && state.decision.kind === "open";
}

// Deliberately stricter than IPC admission, which stays open for in-flight work during Draining.
export function acceptsUserCommands(decision: QuitDecision) {
  return decision.kind === "open";
}

/** The quit dialog shows while the user is asked, and through the quit they chose. */
export function quitDialogOpen(decision: QuitDecision) {
  return (
    decision.kind === "asking" ||
    (decision.kind === "quitting" && decision.origin === "userConfirmed")
  );
}
