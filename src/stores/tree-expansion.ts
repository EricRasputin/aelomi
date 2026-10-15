import { create } from "zustand";
import { commands } from "../ipc";
import { trackCommand } from "../ipc/pending-commands";
import { acceptsUserCommands, useQuitStore } from "./quit";
import { useShellStore } from "./shell";

type Expansion =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; rows: ReadonlySet<string> };

type ExpansionStore = {
  expansion: Expansion;
  load: () => Promise<void>;
  toggle: (pageId: string) => Promise<void>;
};

let loading: Promise<void> | null = null;
let writes: Promise<void> = Promise.resolve();

export const useTreeExpansionStore = create<ExpansionStore>((set, get) => ({
  expansion: { kind: "loading" },
  load: () => {
    if (loading) return loading;
    loading = (async () => {
      try {
        const reply = await commands.settingsGet();
        if (reply.status === "error")
          throw new Error(JSON.stringify(reply.error));
        set({
          expansion: { kind: "ready", rows: new Set(reply.data.expanded_rows) },
        });
      } catch {
        set({
          expansion: {
            kind: "error",
            message: "The sidebar couldn't load its expanded pages.",
          },
        });
      } finally {
        loading = null;
      }
    })();
    return loading;
  },
  toggle: async (pageId) => {
    if (!acceptsUserCommands(useQuitStore.getState().decision)) {
      useShellStore.setState({
        message: "Aelomi is closing. Keep the app open to change the sidebar.",
      });
      return;
    }
    useShellStore.setState({ message: null });
    const { expansion } = get();
    if (expansion.kind !== "ready") return;
    const rows = new Set(expansion.rows);
    if (rows.has(pageId)) rows.delete(pageId);
    else rows.add(pageId);
    set({ expansion: { kind: "ready", rows } });
    const write = trackCommand(
      writes.then(() => commands.settingsUpdate({ expanded_rows: [...rows] })),
    );
    writes = (async () => {
      try {
        const reply = await write;
        if (reply.status === "error")
          throw new Error(JSON.stringify(reply.error));
      } catch {
        useShellStore.setState({
          message:
            "The sidebar's expanded pages couldn't be saved. Try changing the disclosure again.",
        });
      }
    })();
    await writes;
  },
}));
