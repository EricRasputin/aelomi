import { acceptsUserCommands, useQuitStore } from "../stores/quit";
import {
  type RecordCommandStore,
  useRecordCommandStore,
} from "../stores/record-commands";

export function pageCreatePending(state: RecordCommandStore, pageId: string) {
  return [...state.retained.values()].some(
    (op) => op.op === "create" && op.pageId === pageId,
  );
}

export function canOpenPage(pageId: string) {
  return (
    acceptsUserCommands(useQuitStore.getState().decision) &&
    !pageCreatePending(useRecordCommandStore.getState(), pageId)
  );
}
