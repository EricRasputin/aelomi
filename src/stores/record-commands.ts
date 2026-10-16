import { create } from "zustand";
import type { PageError, TreePage } from "../ipc/bindings";

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
