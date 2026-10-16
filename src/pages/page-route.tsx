import { getRouteApi } from "@tanstack/react-router";
import { PageView } from "./page-view";

const route = getRouteApi("/page/$pageId");

export default function PageRoute() {
  const { pageId } = route.useParams();
  const { block } = route.useSearch();
  return <PageView key={pageId} pageId={pageId} focusBlock={block} />;
}
