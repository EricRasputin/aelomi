import { getRouteApi } from "@tanstack/react-router";
import { pageCreatePending } from "../shell/page-opening";
import { useRecordCommandStore } from "../stores/record-commands";
import { PageView } from "./page-view";

const route = getRouteApi("/page/$pageId");

export default function PageRoute() {
  const { pageId } = route.useParams();
  const { block } = route.useSearch();
  const pending = useRecordCommandStore((state) =>
    pageCreatePending(state, pageId),
  );
  if (pending)
    return (
      <p className="page-notice" role="status">
        Creating page…
      </p>
    );
  return <PageView key={pageId} pageId={pageId} focusBlock={block} />;
}
