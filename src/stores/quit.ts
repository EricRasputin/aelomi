import { create } from "zustand";
import { commands, type QuitStatus } from "../ipc";
import { storageCopy } from "../storage-copy";

type QuitDecision =
  | { kind: "open"; revision: number }
  | {
      kind: "asking";
      revision: number;
      pendingDocs: string[];
      error: string | null;
    }
  | { kind: "quitting"; revision: number; pendingDocs: string[] };

type QuitStore = {
  decision: QuitDecision;
  receiveQuit: (status: QuitStatus) => void;
  keepOpen: () => Promise<void>;
  quitAnyway: () => Promise<void>;
};

export const useQuitStore = create<QuitStore>((set, get) => ({
  decision: { kind: "open", revision: 0 },
  receiveQuit: ({ revision, pending_docs }) => {
    const decision = get().decision;
    if (revision <= decision.revision) return;
    if (decision.kind === "quitting") {
      set({
        decision: {
          kind: "quitting",
          revision,
          pendingDocs: pending_docs ?? decision.pendingDocs,
        },
      });
    } else if (pending_docs) {
      set({
        decision: {
          kind: "asking",
          revision,
          pendingDocs: pending_docs,
          error: null,
        },
      });
    } else {
      set({ decision: { kind: "open", revision } });
    }
  },
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
          error: storageCopy.quitError,
        },
      });
    }
  },
}));
