import { create } from "zustand";
import { commands, type RecordCommandResult } from "../ipc";
import { commandsAccepted } from "../ipc/pending-commands";
import { useNewPagesStore } from "../shell/new-pages";
import {
  type PagePresence,
  pagePresence,
  usePagePresence,
} from "../shell/page-presence";
import { usePageTreeStore } from "./page-tree";
import { type QuitDecision, useQuitStore } from "./quit";
import { useShellStore } from "./shell";

type Draft = {
  title: string;
  version: number;
  result: RecordCommandResult<null> | null;
  error: string | null;
};
type TitleStore = {
  drafts: ReadonlyMap<string, Draft>;
  rename: (pageId: string, title: string) => TitleRefusal | null;
};
let version = 0;

export const usePageTitlesStore = create<TitleStore>((set, get) => ({
  drafts: new Map(),
  rename: (pageId, title) => {
    const refusal = titleRefusal(pageId);
    if (refusal) return refusal;
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
    void operation.then((result) => {
      const draft = get().drafts.get(pageId);
      if (draft?.version !== submitted) return;
      set({ drafts: new Map(get().drafts).set(pageId, { ...draft, result }) });
      reconcile();
    });
    return null;
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

export type TitleRefusal =
  | "connecting"
  | "closing"
  | "trashing"
  | "notCreated"
  | "stale"
  | "missing";
export function decideTitle({
  registered,
  accepted,
  decision,
  presence,
  trashing,
}: {
  registered: boolean;
  accepted: boolean;
  decision: QuitDecision["kind"];
  presence: PagePresence;
  trashing: boolean;
}): TitleRefusal | null {
  if (!registered) return "connecting";
  if (!accepted || (decision !== "open" && decision !== "draining"))
    return "closing";
  if (trashing || presence.kind === "trashed") return "trashing";
  if (presence.kind === "new")
    return presence.stage === "failed" ? "notCreated" : null;
  if (presence.kind === "listed") return presence.fresh ? null : "stale";
  return "missing";
}
export function titleRefusal(id: string): TitleRefusal | null {
  const quit = useQuitStore.getState();
  const trash = useShellStore.getState().trashRequest;
  return decideTitle({
    registered: quit.registered,
    accepted: commandsAccepted(),
    decision: quit.decision.kind,
    presence: pagePresence(id),
    trashing: trash?.status === "pending" && trash.affected.has(id),
  });
}
export function useTitleRefusal(id: string) {
  const registered = useQuitStore((s) => s.registered);
  const decision = useQuitStore((s) => s.decision.kind);
  const presence = usePagePresence(id);
  const trashing = useShellStore(
    (s) =>
      s.trashRequest?.status === "pending" && s.trashRequest.affected.has(id),
  );
  return decideTitle({
    registered,
    accepted: commandsAccepted(),
    decision,
    presence,
    trashing,
  });
}
export function usePageTitle(pageId: string) {
  const title = usePageTreeStore((state) =>
    state.source.kind === "ready"
      ? (state.source.byId.get(pageId)?.title ?? null)
      : null,
  );
  const isNew = useNewPagesStore((s) => s.pages.has(pageId));
  const draft = usePageTitlesStore((state) => state.drafts.get(pageId));
  return title === null && !isNew ? null : (draft?.title ?? title ?? "");
}
