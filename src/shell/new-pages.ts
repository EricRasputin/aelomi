import { create } from "zustand";
import { commands, type PageId, type RecordCommandResult } from "../ipc";
import { forgetCommand, retainCommand } from "../ipc/pending-commands";
import { newPageId } from "../ipc/record-commands";
import { usePageTitlesStore } from "../stores/page-titles";
import { usePageTreeStore } from "../stores/page-tree";
import { useShellStore } from "../stores/shell";

export type NewPageStage = "creating" | "created" | "failed";
export const useNewPagesStore = create<{
  pages: ReadonlyMap<PageId, NewPageStage>;
}>(() => ({ pages: new Map() }));
const tokens = new Map<PageId, object>();
function token(id: PageId) {
  let value = tokens.get(id);
  if (!value) {
    value = {};
    tokens.set(id, value);
  }
  return value;
}
export function forgetNewPage(id: PageId) {
  const retained = tokens.get(id);
  if (retained) forgetCommand(retained);
  tokens.delete(id);
  const pages = new Map(useNewPagesStore.getState().pages);
  pages.delete(id);
  useNewPagesStore.setState({ pages });
}
function stage(id: PageId, next: NewPageStage) {
  useNewPagesStore.setState(({ pages }) => ({
    pages: new Map(pages).set(id, next),
  }));
}
export function createNewPage(): PageId {
  const id = newPageId();
  const reply = commands.pageCreate(null, id);
  stage(id, "creating");
  void reply.then((result) => settleNewPage(id, result, false));
  return id;
}
export function settleNewPage(
  id: PageId,
  reply: RecordCommandResult<PageId>,
  retry: boolean,
) {
  if (
    !useNewPagesStore.getState().pages.has(id) ||
    (reply.status === "error" && reply.error === "Abandoned")
  )
    return;
  if (
    reply.status === "ok" ||
    (retry && reply.status === "error" && reply.error === "AlreadyExists")
  ) {
    stage(id, "created");
    reconcile();
    return;
  }
  stage(id, "failed");
  useShellStore.setState({ message: "A new page couldn't be created." });
  if (usePageTitlesStore.getState().drafts.get(id)?.title)
    retainCommand(token(id), {
      retry: () => retryNewPage(id),
      docs: ["workspace"],
    });
}
export function retryNewPage(id: PageId) {
  if (!useNewPagesStore.getState().pages.has(id)) return;
  forgetCommand(token(id));
  stage(id, "creating");
  const reply = commands.pageCreate(null, id);
  void reply.then((result) => settleNewPage(id, result, true));
  const draft = usePageTitlesStore.getState().drafts.get(id)?.title;
  if (draft) usePageTitlesStore.getState().rename(id, draft);
}
function reconcile() {
  const { source } = usePageTreeStore.getState();
  if (source.kind !== "ready" || source.error) return;
  for (const [id, state] of useNewPagesStore.getState().pages)
    if (state === "created" && source.byId.has(id)) forgetNewPage(id);
}
usePageTreeStore.subscribe(reconcile);
