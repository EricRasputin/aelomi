import { create } from "zustand";
import { commands, type PageId, type RecordCommandResult } from "../ipc";
import { forgetCommand, retainCommand } from "../ipc/pending-commands";
import { newPageId } from "../ipc/record-commands";
import { usePageTitlesStore } from "../stores/page-titles";
import { usePageTreeStore } from "../stores/page-tree";
import { useShellStore } from "../stores/shell";

export type NewPage =
  | { stage: "created" }
  | { stage: "creating" | "failed"; accepted: boolean };
export type NewPageStage = NewPage["stage"];
export const useNewPagesStore = create<{
  pages: ReadonlyMap<PageId, NewPage>;
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
function stage(id: PageId, next: NewPage) {
  useNewPagesStore.setState(({ pages }) => ({
    pages: new Map(pages).set(id, next),
  }));
}
export function createNewPage(): PageId {
  const id = newPageId();
  const reply = commands.pageCreate(null, id);
  stage(id, { stage: "creating", accepted: false });
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
    stage(id, { stage: "created" });
    reconcile();
    return;
  }
  const entry = useNewPagesStore.getState().pages.get(id);
  stage(id, {
    stage: "failed",
    accepted:
      reply.accepted === true ||
      listed(id) ||
      (entry?.stage !== "created" && entry?.accepted === true),
  });
  useShellStore.setState({ message: "A new page couldn't be created." });
  if (titleOf(id))
    retainCommand(token(id), {
      retry: () => retryNewPage(id),
      docs: ["workspace"],
    });
}
export function retryNewPage(id: PageId) {
  const entry = useNewPagesStore.getState().pages.get(id);
  if (entry?.stage !== "failed") return;
  forgetCommand(token(id));
  stage(id, { stage: "creating", accepted: entry.accepted });
  const reply = commands.pageCreate(null, id);
  void reply.then((result) => settleNewPage(id, result, true));
  const draft = titleOf(id);
  if (draft) usePageTitlesStore.getState().rename(id, draft);
}
function listed(id: PageId) {
  const { source } = usePageTreeStore.getState();
  return source.kind === "ready" && source.byId.has(id);
}
function titleOf(id: PageId) {
  const draft = usePageTitlesStore.getState().drafts.get(id);
  if (draft) return draft.title;
  const { source } = usePageTreeStore.getState();
  return source.kind === "ready" ? (source.byId.get(id)?.title ?? "") : "";
}
export function activateNewPage(current: PageId | null): PageId {
  const entry = current ? useNewPagesStore.getState().pages.get(current) : null;
  if (
    current &&
    entry &&
    entry.stage !== "created" &&
    titleOf(current) === ""
  ) {
    if (entry.stage === "failed") retryNewPage(current);
    return current;
  }
  return createNewPage();
}
export function forgetEmptyFailuresExcept(keep: PageId) {
  for (const [id, entry] of useNewPagesStore.getState().pages)
    if (
      id !== keep &&
      entry.stage === "failed" &&
      !entry.accepted &&
      !listed(id) &&
      titleOf(id) === ""
    )
      forgetNewPage(id);
}
function reconcile() {
  const { source } = usePageTreeStore.getState();
  if (source.kind !== "ready" || source.error) return;
  for (const [id, entry] of useNewPagesStore.getState().pages)
    if (entry.stage === "created" && source.byId.has(id)) forgetNewPage(id);
}
usePageTreeStore.subscribe(reconcile);
