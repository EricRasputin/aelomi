import { Button } from "@base-ui/react/button";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef } from "react";
import { flattenPages } from "../shell/page-navigation";
import { newPage } from "../shell/use-page-commands";
import { usePageTitle } from "../stores/page-titles";
import { displayTitle, usePageTreeStore } from "../stores/page-tree";
import { canEdit, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";
import { useTreeExpansionStore } from "../stores/tree-expansion";

function RowTitle({ pageId }: { pageId: string }) {
  const title = displayTitle(usePageTitle(pageId));
  return (
    <span className="sidebar-title" dir="auto" title={title}>
      {title}
    </span>
  );
}

export function NewPageButton() {
  const editable = useQuitStore(canEdit);
  const creating = useShellStore((state) => state.creating);
  const ready = usePageTreeStore((state) => state.source.kind === "ready");
  return (
    <Button
      className="shell-button"
      disabled={!editable || !ready || creating}
      onClick={newPage}
    >
      New page
    </Button>
  );
}

export function Sidebar({ openPage }: { openPage: (pageId: string) => void }) {
  const source = usePageTreeStore((state) => state.source);
  const expansion = useTreeExpansionStore((state) => state.expansion);
  const editable = useQuitStore(canEdit);
  const activePageId = useShellStore((state) => state.activePageId);
  const scroll = useRef<HTMLDivElement>(null);
  const rows = useMemo(
    () =>
      source.kind === "ready" && expansion.kind === "ready"
        ? flattenPages(source.tree.pages, expansion.rows)
        : [],
    [source, expansion],
  );
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 30,
    getItemKey: (index) => rows[index]?.page.id ?? index,
    overscan: 8,
  });
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-heading">
        <span>Pages</span>
        <NewPageButton />
      </div>
      {expansion.kind === "error" && (
        <div className="sidebar-notice" role="alert">
          <p>{expansion.message}</p>
          <Button onClick={() => void useTreeExpansionStore.getState().load()}>
            Try again
          </Button>
        </div>
      )}
      <nav
        ref={scroll}
        className="sidebar-scroll"
        aria-label="Pages"
        aria-busy={source.kind === "loading" || expansion.kind === "loading"}
      >
        <div
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const row = rows[item.index];
            if (!row) return null;
            const expanded =
              expansion.kind === "ready" && expansion.rows.has(row.page.id);
            const title = displayTitle(row.page.title);
            return (
              <div
                key={item.key}
                className="sidebar-row"
                data-active={activePageId === row.page.id || undefined}
                data-depth={row.depth}
                style={{
                  position: "absolute",
                  top: 0,
                  insetInline: 0,
                  height: item.size,
                  transform: `translateY(${item.start}px)`,
                  paddingInlineStart: row.depth * 12 + 6,
                  width: `calc(100% + ${Math.max(0, row.depth - 8) * 12}px)`,
                }}
              >
                {row.hasChildren ? (
                  <Button
                    className="sidebar-disclosure"
                    aria-label={`${expanded ? "Collapse" : "Expand"} ${title}`}
                    aria-expanded={expanded}
                    disabled={!editable}
                    onClick={() =>
                      void useTreeExpansionStore.getState().toggle(row.page.id)
                    }
                  >
                    <svg
                      viewBox="0 0 16 16"
                      aria-hidden="true"
                      width="14"
                      height="14"
                    >
                      <path d={expanded ? "m4 6 4 4 4-4" : "m6 4 4 4-4 4"} />
                    </svg>
                  </Button>
                ) : (
                  <span className="sidebar-leaf" aria-hidden="true" />
                )}
                <Button
                  className="sidebar-page"
                  aria-current={
                    activePageId === row.page.id ? "page" : undefined
                  }
                  onClick={() => openPage(row.page.id)}
                >
                  <RowTitle pageId={row.page.id} />
                </Button>
              </div>
            );
          })}
        </div>
      </nav>
    </aside>
  );
}
