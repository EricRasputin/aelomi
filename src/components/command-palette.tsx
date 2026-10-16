import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useLayoutEffect, useRef } from "react";
import { createPortal, flushSync } from "react-dom";
import { commandDispatcher } from "../command-dispatcher";
import { restoreFocus } from "../editor/active-editor";
import type { PageIssue, SearchHit } from "../ipc";
import { HighlightedText } from "../pages/highlighted-text";
import { type ContentResult, shownContent } from "../palette/content";
import type { PageCommand } from "../palette/page-commands";
import type { TitleMatch } from "../palette/title-match";
import { useNewPagesStore } from "../shell/new-pages";
import { canOpenPage } from "../shell/page-presence";
import { useOpenPage } from "../shell/use-page-commands";
import { useIndexStatusStore } from "../stores/index-status";
import {
  displayTitle,
  linkText,
  type SourceTree,
  usePageTreeStore,
} from "../stores/page-tree";
import { usePaletteStore } from "../stores/palette";
import { acceptsUserCommands, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";

export const paletteCopy = {
  indexing: "Indexing…",
  failed: "Search is unavailable because the index couldn't be rebuilt.",
  noContent: "No page content matches.",
  pages: "Pages",
  content: "Content",
};

type Row =
  | { kind: "header"; label: string }
  | { kind: "title"; index: number; match: TitleMatch }
  | { kind: "content"; index: number; hit: SearchHit }
  | { kind: "command"; index: number; command: PageCommand };

function rowsOf(
  titles: readonly TitleMatch[],
  commands: readonly PageCommand[],
  content: ContentResult,
): Row[] {
  const rows: Row[] = [];
  if (titles.length > 0)
    rows.push({ kind: "header", label: paletteCopy.pages });
  for (const [index, match] of titles.entries())
    rows.push({ kind: "title", index, match });
  if (commands.length > 0) rows.push({ kind: "header", label: "Commands" });
  for (const [i, command] of commands.entries())
    rows.push({ kind: "command", index: titles.length + i, command });
  if (content.kind === "ready" && content.hits.length > 0) {
    rows.push({ kind: "header", label: paletteCopy.content });
    for (const [i, hit] of content.hits.entries())
      rows.push({
        kind: "content",
        index: titles.length + commands.length + i,
        hit,
      });
  }
  return rows;
}

const rowHeight: Record<Row["kind"], number> = {
  header: 28,
  title: 34,
  command: 34,
  content: 52,
};

function issueNotice(issues: readonly PageIssue[], source: SourceTree) {
  if (issues.length === 0) return null;
  const names = issues.map((issue) => linkText(source, issue.page_id));
  return `${issues.length === 1 ? "1 page is" : `${issues.length} pages are`} only partly searchable: ${names.join(", ")}.`;
}

export function CommandPalette() {
  const open = usePaletteStore((state) => state.open);
  const query = usePaletteStore((state) => state.query);
  const pageCommands = usePaletteStore((state) => state.pageCommands);
  const titles = usePaletteStore((state) => state.titles);
  const content = usePaletteStore((state) => state.content);
  const selected = usePaletteStore((state) => state.selected);
  const settled = usePaletteStore((state) => state.answered === state.request);
  const status = useIndexStatusStore((state) => state.status);
  const source = usePageTreeStore((state) => state.source);
  const newPages = useNewPagesStore((s) => s.pages);
  useShellStore((s) => s.trashed);
  useQuitStore((s) => s.decision.kind);
  const openPage = useOpenPage();
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  // The one owner of what opening and closing do to the page: the store only
  // flips `open`, and every close commits synchronously, so focus is back
  // where it was before the next key arrives.
  useLayoutEffect(() => {
    if (!open) return;
    const { returnTo } = usePaletteStore.getState();
    const page = document.querySelector<HTMLElement>("[data-workspace]");
    if (page) page.inert = true;
    input.current?.focus();
    void usePageTreeStore.getState().refresh();
    return () => {
      if (page) page.inert = false;
      restoreFocus(returnTo);
    };
  }, [open]);

  const close = () => flushSync(() => usePaletteStore.getState().hide());

  const shown = shownContent(status, content);
  const rows = rowsOf(titles, pageCommands, shown);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => list.current,
    estimateSize: (i) => rowHeight[rows[i]?.kind ?? "title"],
    overscan: 6,
  });
  const selectedRow = rows.findIndex(
    (row) => row.kind !== "header" && row.index === selected,
  );
  useEffect(() => {
    if (selectedRow >= 0) virtualizer.scrollToIndex(selectedRow);
  }, [selectedRow, virtualizer]);

  const activate = (row: Row | undefined) => {
    if (!row || row.kind === "header") return;
    if (row.kind === "command") {
      if (!acceptsUserCommands(useQuitStore.getState().decision)) return;
      close();
      commandDispatcher.dispatch(row.command.command, {
        source: "palette",
        target: document.activeElement,
        isComposing: false,
      });
      return;
    }
    const id = row.kind === "title" ? row.match.id : row.hit.page_id;
    if (canOpenPage(id)) close();
    if (row.kind === "title") openPage(row.match.id, "body");
    else openPage(row.hit.page_id, "body", { block: row.hit.block?.block_id });
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    const palette = usePaletteStore.getState();
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      palette.move(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      activate(rows[selectedRow]);
    } else if (event.key === "Escape" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Tab") {
      event.preventDefault();
    }
  };

  const issues =
    shown.kind === "ready" ? issueNotice(shown.issues, source) : null;
  const items = virtualizer.getVirtualItems();
  const activeId = selectedRow >= 0 ? `palette-row-${selectedRow}` : undefined;

  if (!open) return null;
  return createPortal(
    <>
      <div
        className="palette-backdrop"
        aria-hidden="true"
        onMouseDown={(event) => {
          event.preventDefault();
          close();
        }}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        className="palette"
      >
        <input
          ref={input}
          className="palette-input"
          aria-label="Search pages"
          role="combobox"
          aria-expanded={rows.length > 0}
          aria-controls="palette-results"
          aria-activedescendant={activeId}
          autoComplete="off"
          spellCheck={false}
          placeholder="Search pages…"
          value={query}
          onChange={(event) =>
            usePaletteStore
              .getState()
              .setQuery(event.target.value, usePageTreeStore.getState().source)
          }
          onKeyDown={onKeyDown}
        />
        <div
          ref={list}
          id="palette-results"
          role="listbox"
          aria-label="Results"
          className="palette-list"
          data-empty={rows.length === 0 || undefined}
          data-query={query}
          data-settled={settled}
        >
          <div
            style={{
              height: virtualizer.getTotalSize(),
              position: "relative",
            }}
          >
            {items.map((item) => {
              const row = rows[item.index];
              if (!row) return null;
              const style = {
                position: "absolute" as const,
                top: 0,
                left: 0,
                right: 0,
                height: item.size,
                transform: `translateY(${item.start}px)`,
              };
              if (row.kind === "header")
                return (
                  <div
                    key={item.key}
                    role="presentation"
                    className="palette-header"
                    style={style}
                  >
                    {row.label}
                  </div>
                );
              const isSelected = row.index === selected;
              return (
                <div
                  key={item.key}
                  id={`palette-row-${item.index}`}
                  role="option"
                  aria-selected={isSelected}
                  aria-disabled={
                    row.kind === "title"
                      ? !canOpenPage(row.match.id)
                      : row.kind === "content"
                        ? !canOpenPage(row.hit.page_id)
                        : undefined
                  }
                  tabIndex={-1}
                  className="palette-row"
                  data-kind={row.kind}
                  data-page-id={
                    row.kind === "title"
                      ? row.match.id
                      : row.kind === "content"
                        ? row.hit.page_id
                        : undefined
                  }
                  style={style}
                  onMouseMove={() => {
                    if (!isSelected)
                      usePaletteStore.getState().select(row.index);
                  }}
                  onClick={() => activate(row)}
                  onKeyDown={() => {}}
                >
                  {row.kind === "command" ? (
                    <span className="palette-title">{row.command.label}</span>
                  ) : row.kind === "title" ? (
                    <span className="palette-title">
                      <HighlightedText parts={row.match.parts} />
                      {newPages.get(row.match.id)?.stage === "failed" && (
                        <span className="sidebar-failure" aria-hidden="true">
                          Not created
                        </span>
                      )}
                    </span>
                  ) : (
                    <>
                      <span className="palette-title">
                        {row.hit.title.length > 0 ? (
                          <HighlightedText parts={row.hit.title} />
                        ) : (
                          displayTitle("")
                        )}
                      </span>
                      {row.hit.block && (
                        <span className="palette-snippet">
                          <HighlightedText parts={row.hit.block.text} />
                        </span>
                      )}
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </div>
        {shown.kind === "failed" && (
          <div role="alert" className="palette-alert">
            <p>{paletteCopy.failed}</p>
            <p className="palette-alert-reason">{shown.reason}</p>
          </div>
        )}
        <p
          role="status"
          aria-label="Search status"
          className="palette-status"
          data-testid="palette-status"
        >
          {shown.kind === "indexing"
            ? paletteCopy.indexing
            : shown.kind === "error"
              ? `Search couldn't run: ${shown.message}`
              : shown.kind === "ready" &&
                  shown.hits.length === 0 &&
                  query.trim() !== ""
                ? paletteCopy.noContent
                : ""}
        </p>
        {issues && (
          <p className="palette-notice" data-testid="palette-issues">
            {issues}
          </p>
        )}
      </div>
    </>,
    document.body,
  );
}
