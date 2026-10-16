import type { PageId, TreePage } from "../ipc";
import { type SourceTree, usePageTreeStore } from "../stores/page-tree";
import { acceptsUserCommands, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";
import { type NewPage, type NewPageStage, useNewPagesStore } from "./new-pages";

export type PagePresence =
  | { kind: "trashed" }
  | { kind: "new"; stage: NewPageStage }
  | { kind: "listed"; fresh: boolean }
  | { kind: "missing"; fresh: boolean };
export function presenceOf(
  id: PageId,
  source: SourceTree,
  pages: ReadonlyMap<PageId, NewPage>,
  trashed: ReadonlySet<PageId>,
): PagePresence {
  if (trashed.has(id)) return { kind: "trashed" };
  const entry = pages.get(id);
  if (entry) return { kind: "new", stage: entry.stage };
  const fresh = source.kind === "ready" && !source.error;
  return source.kind === "ready" && source.byId.has(id)
    ? { kind: "listed", fresh }
    : { kind: "missing", fresh };
}
export function pagePresence(id: PageId) {
  return presenceOf(
    id,
    usePageTreeStore.getState().source,
    useNewPagesStore.getState().pages,
    useShellStore.getState().trashed,
  );
}
export function usePagePresence(id: PageId) {
  const source = usePageTreeStore((s) => s.source);
  const pages = useNewPagesStore((s) => s.pages);
  const trashed = useShellStore((s) => s.trashed);
  return presenceOf(id, source, pages, trashed);
}
export function bodyReady(p: PagePresence) {
  return p.kind === "listed" || (p.kind === "new" && p.stage === "created");
}
export function canOpenPage(id: PageId) {
  return (
    acceptsUserCommands(useQuitStore.getState().decision) &&
    pagePresence(id).kind !== "trashed"
  );
}
export function visiblePages(
  source: SourceTree,
  pages: ReadonlyMap<PageId, NewPage>,
  trashed: ReadonlySet<PageId>,
): TreePage[] {
  const visible =
    source.kind === "ready"
      ? source.tree.pages.filter((p) => !trashed.has(p.id))
      : [];
  const listed = new Set(visible.map((p) => p.id));
  for (const id of pages.keys())
    if (!listed.has(id) && !trashed.has(id))
      visible.push({
        id,
        title: "",
        parent_id: null,
        created_at: null,
        updated_at: null,
      });
  return visible;
}
usePageTreeStore.subscribe(({ source }) => {
  if (source.kind !== "ready" || source.error) return;
  const shell = useShellStore.getState();
  const trashed = new Set(
    [...shell.trashed].filter((id) => source.byId.has(id)),
  );
  const trashRequest =
    shell.trashRequest?.status === "error" &&
    !source.byId.has(shell.trashRequest.pageId)
      ? null
      : shell.trashRequest;
  if (
    trashed.size !== shell.trashed.size ||
    trashRequest !== shell.trashRequest
  )
    useShellStore.setState({
      trashed,
      trashRequest,
      ...(trashRequest !== shell.trashRequest ? { message: null } : {}),
    });
});
