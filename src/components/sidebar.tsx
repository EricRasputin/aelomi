import { Button } from "@base-ui/react/button";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { flattenPages } from "../shell/page-navigation";
import { pageCreatePending } from "../shell/page-opening";
import { newPage } from "../shell/use-page-commands";
import { usePageTitle } from "../stores/page-titles";
import { displayTitle, usePageTreeStore } from "../stores/page-tree";
import { canEdit, useQuitStore } from "../stores/quit";
import { useRecordCommandStore } from "../stores/record-commands";
import { useShellStore } from "../stores/shell";
import { useTreeExpansionStore } from "../stores/tree-expansion";

function RowTitle({ title }: { title: string }) {
  const label = useRef<HTMLSpanElement>(null);
  const [truncated, setTruncated] = useState(false);
  useLayoutEffect(() => {
    const element = label.current;
    if (!element) return;
    const measure = () =>
      setTruncated(element.scrollWidth > element.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  });
  return (
    <span
      ref={label}
      className="sidebar-title"
      dir="auto"
      title={truncated ? title : undefined}
    >
      {title}
    </span>
  );
}

function PageRow({
  pageId,
  fallback,
  expanded,
  disclosureDisabled,
  hasChildren,
  active,
  openPage,
}: {
  pageId: string;
  fallback: string;
  expanded: boolean;
  disclosureDisabled: boolean;
  hasChildren: boolean;
  active: boolean;
  openPage: (pageId: string) => void;
}) {
  const title = displayTitle(usePageTitle(pageId) ?? fallback);
  const editable = useQuitStore(canEdit);
  const pending = useRecordCommandStore((state) =>
    pageCreatePending(state, pageId),
  );
  return (
    <>
      {hasChildren ? (
        <Button
          className="sidebar-disclosure"
          aria-label={`${title} children`}
          aria-expanded={expanded}
          disabled={disclosureDisabled}
          onClick={() => void useTreeExpansionStore.getState().toggle(pageId)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true" width="14" height="14">
            <path d={expanded ? "m4 6 4 4 4-4" : "m6 4 4 4-4 4"} />
          </svg>
        </Button>
      ) : (
        <span className="sidebar-leaf" aria-hidden="true" />
      )}
      <Button
        className="sidebar-page"
        aria-current={active ? "page" : undefined}
        aria-disabled={!editable || pending}
        aria-busy={pending}
        onClick={() => openPage(pageId)}
      >
        <RowTitle title={title} />
      </Button>
    </>
  );
}

export function NewPageButton() {
  const editable = useQuitStore(canEdit);
  const creating = useRecordCommandStore((state) =>
    state.pendingCreates.has(null),
  );
  const opening = useShellStore(
    (state) => state.createIntent?.stage.kind === "opening",
  );
  const busy = creating || opening;
  const ready = usePageTreeStore((state) => state.source.kind === "ready");
  return (
    <Button
      className="shell-button"
      disabled={!editable || !ready || busy}
      focusableWhenDisabled
      aria-busy={busy}
      onClick={(event) => {
        event.currentTarget.focus();
        newPage();
      }}
      onKeyDownCapture={(event) => {
        if (
          !busy ||
          event.metaKey ||
          event.ctrlKey ||
          event.altKey ||
          event.key.length !== 1
        )
          return;
        event.preventDefault();
        document.getElementById("new-page-title")?.focus();
        document.execCommand("insertText", false, event.key);
      }}
    >
      {busy ? "Creating page…" : "New page"}
    </Button>
  );
}

export function Sidebar({
  openPage,
  hidden = false,
}: {
  openPage: (pageId: string) => void;
  hidden?: boolean;
}) {
  const source = usePageTreeStore((state) => state.source);
  const expansion = useTreeExpansionStore((state) => state.expansion);
  const editable = useQuitStore(canEdit);
  const activePageId = useShellStore((state) => state.activePageId);
  const revealedPageId = useShellStore((state) => state.revealedPageId);
  const revealPageId = revealedPageId ?? activePageId;
  const scroll = useRef<HTMLDivElement>(null);
  const rows = useMemo(
    () =>
      source.kind === "ready"
        ? flattenPages(
            source.tree.pages,
            expansion.kind === "ready" ? expansion.rows : new Set(),
          )
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
  const revealed = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (
      source.kind !== "ready" ||
      expansion.kind !== "ready" ||
      !revealPageId ||
      revealed.current === revealPageId
    )
      return;
    revealed.current = revealPageId;
    const ancestors: string[] = [];
    let parent = source.byId.get(revealPageId)?.parent_id;
    while (parent && !ancestors.includes(parent)) {
      ancestors.push(parent);
      parent = source.byId.get(parent)?.parent_id;
    }
    useTreeExpansionStore.getState().reveal(ancestors);
  }, [source, expansion.kind, revealPageId]);
  const scrolled = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (hidden || scrolled.current === revealPageId) return;
    const index = rows.findIndex((row) => row.page.id === revealPageId);
    if (index < 0) return;
    virtualizer.scrollToIndex(index, { align: "auto" });
    scrolled.current = revealPageId;
  }, [revealPageId, rows, virtualizer, hidden]);
  return (
    <aside className="sidebar" aria-label="Sidebar" hidden={hidden}>
      <div className="sidebar-heading">
        <span>Pages</span>
        <NewPageButton />
      </div>
      {(source.kind === "error" ||
        (source.kind === "ready" && source.error)) && (
        <div className="sidebar-notice" role="alert">
          <p>
            {source.kind === "ready"
              ? "Pages couldn't be refreshed. Showing the last available list."
              : "Pages couldn't be loaded."}
          </p>
          <Button
            className="shell-button"
            disabled={!editable}
            onClick={() => void usePageTreeStore.getState().refresh()}
          >
            Try again
          </Button>
        </div>
      )}
      {expansion.kind === "error" && (
        <div className="sidebar-notice" role="alert">
          <p>{expansion.message}</p>
          <Button
            className="shell-button"
            onClick={() => void useTreeExpansionStore.getState().load()}
          >
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
                <PageRow
                  pageId={row.page.id}
                  fallback={row.page.title}
                  expanded={expanded}
                  disclosureDisabled={!editable || expansion.kind !== "ready"}
                  hasChildren={row.hasChildren}
                  active={activePageId === row.page.id}
                  openPage={openPage}
                />
              </div>
            );
          })}
        </div>
      </nav>
    </aside>
  );
}
