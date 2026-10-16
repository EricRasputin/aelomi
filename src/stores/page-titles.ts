import { create } from "zustand";
import { commands } from "../ipc";
import { commandsAccepted } from "../ipc/pending-commands";
import { observeRename, renameState } from "../ipc/record-commands";
import { usePageTreeStore } from "./page-tree";
import { useQuitStore } from "./quit";

type Draft = {
  title: string;
  version: number;
  operation: ReturnType<typeof commands.pageRename>;
  error: string | null;
};
type TitleStore = {
  drafts: ReadonlyMap<string, Draft>;
  rename: (pageId: string, title: string) => Promise<void>;
};
let version = 0;
const observers = new Map<string, () => void>();

export const usePageTitlesStore = create<TitleStore>((set, get) => ({
  drafts: new Map(),
  rename: async (pageId, title) => {
    const { registered, decision } = useQuitStore.getState();
    const { source } = usePageTreeStore.getState();
    if (
      !registered ||
      !commandsAccepted() ||
      (decision.kind !== "open" && decision.kind !== "draining") ||
      source.kind !== "ready" ||
      !source.byId.has(pageId)
    )
      return;
    observers.get(pageId)?.();
    const operation = commands.pageRename(pageId, title);
    set({
      drafts: new Map(get().drafts).set(pageId, {
        title,
        version: ++version,
        operation,
        error: null,
      }),
    });
    observers.set(pageId, observeRename(operation, reconcile));
    reconcile();
    await operation.catch(() => {});
  },
}));

function reconcile() {
  const { source } = usePageTreeStore.getState();
  const current = usePageTitlesStore.getState().drafts;
  const drafts = new Map(current);
  for (const [pageId, draft] of current) {
    const state = renameState(draft.operation);
    if (
      state?.kind === "acknowledged" &&
      source.kind === "ready" &&
      source.byId.get(pageId)?.title === draft.title
    ) {
      drafts.delete(pageId);
      observers.get(pageId)?.();
      observers.delete(pageId);
      continue;
    }
    const error =
      state?.kind === "refused"
        ? "This title couldn't be changed. Your text is kept here."
        : null;
    if (error !== draft.error) drafts.set(pageId, { ...draft, error });
  }
  if ([...current].some(([id, draft]) => drafts.get(id) !== draft))
    usePageTitlesStore.setState({ drafts });
}

usePageTreeStore.subscribe(reconcile);

export function usePageTitle(pageId: string) {
  const title = usePageTreeStore((state) =>
    state.source.kind === "ready"
      ? (state.source.byId.get(pageId)?.title ?? null)
      : null,
  );
  const draft = usePageTitlesStore((state) => state.drafts.get(pageId));
  return title === null ? null : (draft?.title ?? title);
}
