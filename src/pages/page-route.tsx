import { getRouteApi } from "@tanstack/react-router";
import { usePageTreeStore } from "../stores/page-tree";
import { useRecordCommandStore } from "../stores/record-commands";
import { useShellStore } from "../stores/shell";
import { PageView } from "./page-view";

const route = getRouteApi("/page/$pageId");

export default function PageRoute() {
  const { pageId } = route.useParams();
  const { block } = route.useSearch();
  const pending = useShellStore((state) => state.pendingCreation);
  const known = usePageTreeStore(
    (state) => state.source.kind === "ready" && state.source.byId.has(pageId),
  );
  const retrying = useRecordCommandStore((state) =>
    pending ? state.retained.has(pending.operationId) : false,
  );
  if (pending?.pageId === pageId && !known)
    return (
      <section className="empty-state" aria-label="Creating page">
        <p role="status">
          {retrying
            ? "Creating page… Aelomi will keep trying."
            : "Opening page…"}
        </p>
      </section>
    );
  return <PageView key={pageId} pageId={pageId} focusBlock={block} />;
}
