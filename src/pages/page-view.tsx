import "@blocknote/core/style.css";
import { SuggestionMenu } from "@blocknote/core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { setActiveEditor } from "../editor/active-editor";
import { renderSegments, type ShownBlock } from "../editor/body-reader";
import { inCode, type PickerKey, pickerTrigger } from "../editor/page-picker";
import { openPage, type PageSession } from "../editor/page-session";
import { openPageLink, recordPageLinkPointer } from "../editor/url-links";
import { markPageContent } from "../perf/cold-start";
import { storageErrorText } from "../storage-copy";
import { displayTitle, linkText, usePageTreeStore } from "../stores/page-tree";
import { usePickerStore } from "../stores/picker";
import { canEdit, useQuitStore } from "../stores/quit";
import { useRecentPagesStore } from "../stores/recent-pages";
import { PagePicker } from "./page-picker";

type ViewState = { kind: "loading" } | PageSession;

export const readOnlyCopy =
  "Read-only. This page holds content this version of Aelomi can't edit without changing it.";
export const editingPausedCopy =
  "Editing pauses while Aelomi connects to its shell.";

function PageTitle({ pageId }: { pageId: string }) {
  const title = usePageTreeStore((state) => {
    if (state.source.kind !== "ready") return null;
    const page = state.source.byId.get(pageId);
    return page ? displayTitle(page.title) : null;
  });
  return (
    <h1 className="page-title" data-testid="page-title">
      {title ?? ""}
    </h1>
  );
}

function ReadOnlyBody({ blocks }: { blocks: readonly ShownBlock[] }) {
  const source = usePageTreeStore((state) => state.source);
  return (
    <div className="page-readonly" data-testid="page-readonly">
      {blocks.map((block, index) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: a block may have no ID, and this list never reorders
        <p key={index}>
          {renderSegments(block.segments, (pageId) => linkText(source, pageId))}
        </p>
      ))}
    </div>
  );
}

export function PageView({
  pageId,
  focusBlock,
}: {
  pageId: string;
  focusBlock: string | undefined;
}) {
  const [view, setView] = useState<ViewState>({ kind: "loading" });
  const host = useRef<HTMLDivElement>(null);
  const editable = useQuitStore(canEdit);
  const registered = useQuitStore((state) => state.registered);

  useEffect(() => {
    useRecentPagesStore.getState().opened(pageId);
    let cancelled = false;
    let session: PageSession | null = null;
    setView({ kind: "loading" });
    const picker = usePickerStore.getState();
    void openPage({
      pageId,
      onPickerKey: (key: PickerKey) => usePickerStore.getState().key(key),
      onTrip: (readOnly) => {
        if (!cancelled) setView(readOnly);
      },
    }).then((opened) => {
      session = opened;
      if (cancelled) {
        if (opened.kind === "editable") opened.close();
        return;
      }
      setView(opened);
    });
    return () => {
      cancelled = true;
      picker.detach();
      if (session?.kind === "editable") session.close();
    };
  }, [pageId]);

  const session = view.kind === "editable" ? view : null;
  const editor = session?.editor ?? null;

  useLayoutEffect(() => {
    if (!session || !host.current) return;
    const { editor } = session;
    editor.isEditable = false;
    const unmount = session.mount(host.current);
    const menu = editor.getExtension(SuggestionMenu);
    menu?.addSuggestionMenu({
      triggerCharacter: pickerTrigger,
      shouldOpen: (tr) => !inCode(tr),
    });
    const stop = usePickerStore.getState().attach(editor);
    const view = editor.prosemirrorView;
    if (view)
      setActiveEditor({
        dom: view.dom,
        focus: () => editor.focus(),
        isEditable: () => editor.isEditable,
        undo: () => editor.undo(),
        redo: () => editor.redo(),
      });
    return () => {
      setActiveEditor(null);
      stop();
      menu?.removeSuggestionMenu(pickerTrigger);
      unmount();
    };
  }, [session]);

  useEffect(() => {
    if (!editor) return;
    editor.isEditable = editable;
    if (editable) markPageContent();
  }, [editor, editable]);

  useEffect(() => {
    if (!editor) return;
    const block = focusBlock && editor.getBlock(focusBlock);
    if (block) {
      editor.setTextCursorPosition(block, "start");
      editor.focus();
      const element = host.current?.querySelector<HTMLElement>(
        `[data-node-type="blockOuter"][data-id="${CSS.escape(focusBlock)}"]`,
      );
      element?.scrollIntoView({ block: "center" });
      if (element) flash(element);
    } else if (editable) {
      editor.focus();
    }
  }, [editor, focusBlock, editable]);

  return (
    <article
      className="page"
      aria-label="Page"
      onPointerDownCapture={recordPageLinkPointer}
      onClickCapture={openPageLink}
      onAuxClickCapture={openPageLink}
      onContextMenuCapture={openPageLink}
    >
      <PageTitle pageId={pageId} />
      {view.kind === "readOnly" && (
        <>
          <p className="page-notice" data-reason={JSON.stringify(view.reason)}>
            {readOnlyCopy}
          </p>
          <ReadOnlyBody blocks={view.body.blocks} />
        </>
      )}
      {view.kind === "error" && (
        <p role="alert" className="page-notice">
          {storageErrorText(view.error)}
        </p>
      )}
      {view.kind === "editable" && !registered && (
        <p className="page-notice">{editingPausedCopy}</p>
      )}
      <div ref={host} className="page-body" />
      {view.kind === "editable" && <PagePicker />}
    </article>
  );
}

function flash(element: HTMLElement) {
  const page = element.closest("article");
  if (!page) return;
  const rect = element.getBoundingClientRect();
  const origin = page.getBoundingClientRect();
  const overlay = document.createElement("div");
  overlay.className = "block-flash";
  overlay.setAttribute("aria-hidden", "true");
  Object.assign(overlay.style, {
    top: `${rect.top - origin.top}px`,
    left: `${rect.left - origin.left}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
  });
  page.append(overlay);
  overlay.addEventListener("animationend", () => overlay.remove());
}
