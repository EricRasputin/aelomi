import { create } from "zustand";
import type { PageError, StorageError, TreePage } from "../ipc";

export type OperationId = string;
export type PageId = TreePage["id"];
export type TransportFailure = { Transport: string };
export type RecordCommandError =
  | PageError
  | StorageError
  | TransportFailure
  | "Abandoned";
export type RecordOp = "create" | "rename" | "trash";
export type RetainedRecordOp = {
  id: OperationId;
  op: RecordOp;
  pageId: PageId;
  error: RecordCommandError;
  attempts: number;
  since: number;
};
export type FailedRecordOp = {
  id: OperationId;
  op: RecordOp;
  pageId: PageId;
  error: RecordCommandError;
  at: number;
};
export type RecordCommandStore = {
  retained: Map<OperationId, RetainedRecordOp>;
  failed: Map<OperationId, FailedRecordOp>;
  pendingCreates: ReadonlySet<PageId | null>;
  dismissFailed: (id: OperationId) => void;
};
export const useRecordCommandStore = create<RecordCommandStore>(() => ({
  retained: new Map(),
  failed: new Map(),
  pendingCreates: new Set(),
  dismissFailed: () => {},
}));

export function failing(state: RecordCommandStore): boolean {
  return state.retained.size > 0 || state.failed.size > 0;
}

export function pendingCreate(parentId: PageId | null): boolean {
  return useRecordCommandStore.getState().pendingCreates.has(parentId);
}
