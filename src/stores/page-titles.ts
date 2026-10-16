import { create } from "zustand";
import { commands, type RecordCommandResult } from "../ipc";
import { commandsAccepted } from "../ipc/pending-commands";
import { usePageTreeStore } from "./page-tree";
import { useQuitStore } from "./quit";

type Draft = {
  title: string;
  version: number;
  result: RecordCommandResult<null> | null;
  error: string | null;
};
type TitleStore = {
  drafts: ReadonlyMap<string, Draft>;
  rename: (pageId: string, title: string) => Promise<void>;
};
let version = 0;

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
    const submitted = ++version;
    const operation = commands.pageRename(pageId, title);
    set({
      drafts: new Map(get().drafts).set(pageId, {
        title,
        version: submitted,
        result: null,
        error: null,
      }),
    });
    const result = await operation;
    const draft = get().drafts.get(pageId);
    if (draft?.version !== submitted) return;
    set({ drafts: new Map(get().drafts).set(pageId, { ...draft, result }) });
    reconcile();
  },
}));

function reconcile() {
  const { source } = usePageTreeStore.getState();
  const current = usePageTitlesStore.getState().drafts;
  const drafts = new Map(current);
  for (const [pageId, draft] of current) {
    const result = draft.result;
    if (
      result?.status === "ok" &&
      source.kind === "ready" &&
      source.byId.get(pageId)?.title === draft.title
    ) {
      drafts.delete(pageId);
      continue;
    }
    const error =
      result?.status === "error"
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
