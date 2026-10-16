import { acceptsUserCommands, useQuitStore } from "../stores/quit";
import {
  type RecordCommandStore,
  useRecordCommandStore,
} from "../stores/record-commands";
import { useShellStore } from "../stores/shell";

export function pageCreatePending(state: RecordCommandStore, pageId: string) {
  const intent = useShellStore.getState().createIntent;
  if (
    state.pendingCreates.has(null) &&
    intent?.stage.kind === "pending" &&
    !intent.existingPageIds.has(pageId)
  )
    return true;
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
