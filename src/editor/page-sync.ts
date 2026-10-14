import * as Y from "yjs";
import { deliver } from "./delivery";

export const loadOrigin = Symbol("doc_load");

function deletes(update: Uint8Array): boolean {
  return Y.decodeUpdate(update).ds.clients.size > 0;
}

type SyncOptions = {
  docId: string;
  ydoc: Y.Doc;
  /** Whether the user has typed, clicked, pasted, or dropped in the editor yet. */
  hasUserInput: () => boolean;
  /** Called once when an update before any user input would delete source. */
  onTrip: () => void;
};

/**
 * Sends every Yjs update except the load itself to the store at once, with no
 * debounce, merge, or timer. Before the user's first input, an update that
 * deletes anything is not sent and stops the sync: the page reopens read-only
 * from the bytes it loaded, so a binding can never persist a destructive
 * initialization.
 */
export function syncPage({
  docId,
  ydoc,
  hasUserInput,
  onTrip,
}: SyncOptions): () => void {
  let tripped = false;
  const onUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === loadOrigin || tripped) return;
    if (!hasUserInput() && deletes(update)) {
      tripped = true;
      onTrip();
      return;
    }
    deliver(docId, update);
  };
  ydoc.on("update", onUpdate);
  return () => ydoc.off("update", onUpdate);
}
