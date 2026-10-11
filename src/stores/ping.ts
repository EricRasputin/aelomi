import { create } from "zustand";
import { commands, type PingReply } from "../ipc";

type PingStatus =
  | { kind: "idle" }
  | { kind: "pending" }
  | { kind: "ready"; reply: PingReply }
  | { kind: "error"; message: string };

type PingStore = {
  status: PingStatus;
  seq: number;
  ping: () => Promise<void>;
};

export const usePingStore = create<PingStore>((set, get) => ({
  status: { kind: "idle" },
  seq: 0,
  ping: async () => {
    if (get().status.kind === "pending") return;
    const seq = get().seq + 1;
    set({ status: { kind: "pending" } });
    try {
      const reply = await commands.ping({ seq });
      set({ status: { kind: "ready", reply }, seq: reply.seq });
    } catch (error: unknown) {
      set({
        status: {
          kind: "error",
          message: error instanceof Error ? error.message : String(error),
        },
      });
    }
  },
}));
