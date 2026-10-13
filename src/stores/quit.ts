import { create } from "zustand";
import { commands, type QuitBlocked } from "../ipc";
import { storageCopy } from "../storage-copy";

type QuitDecision =
  | { kind: "open" }
  | { kind: "asking"; pendingDocs: string[]; error: string | null }
  | { kind: "quitting"; pendingDocs: string[] };

type QuitStore = {
  decision: QuitDecision;
  receiveBlocked: (blocked: QuitBlocked) => void;
  keepOpen: () => void;
  quitAnyway: () => Promise<void>;
};

export const useQuitStore = create<QuitStore>((set, get) => ({
  decision: { kind: "open" },
  receiveBlocked: ({ pending_docs }) => {
    if (get().decision.kind === "quitting") return;
    set({
      decision: { kind: "asking", pendingDocs: pending_docs, error: null },
    });
  },
  keepOpen: () => {
    if (get().decision.kind === "asking") set({ decision: { kind: "open" } });
  },
  quitAnyway: async () => {
    const decision = get().decision;
    if (decision.kind !== "asking") return;
    set({ decision: { kind: "quitting", pendingDocs: decision.pendingDocs } });
    try {
      await commands.quitAnyway();
    } catch {
      set({
        decision: {
          kind: "asking",
          pendingDocs: decision.pendingDocs,
          error: storageCopy.quitError,
        },
      });
    }
  },
}));
