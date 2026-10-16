import { create } from "zustand";
import type { PageError, TreePage } from "../ipc";

export type OperationId = string;
export type PageId = TreePage["id"];
export type RetainedRecordOp = {
  id: OperationId;
  op: "create" | "rename" | "move" | "trash" | "restore";
  pageId: PageId;
  error: PageError | string;
  attempts: number;
  since: number;
};
export type RecordCommandStore = {
  retained: Map<OperationId, RetainedRecordOp>;
  failing: boolean;
};
export const useRecordCommandStore = create<RecordCommandStore>(() => ({
  retained: new Map(),
  failing: false,
}));

export function retainRecordOp(operation: RetainedRecordOp) {
  const retained = new Map(useRecordCommandStore.getState().retained);
  retained.set(operation.id, operation);
  useRecordCommandStore.setState({ retained, failing: retained.size > 0 });
}

export function forgetRecordOp(id: OperationId) {
  const retained = new Map(useRecordCommandStore.getState().retained);
  retained.delete(id);
  useRecordCommandStore.setState({ retained, failing: retained.size > 0 });
}
