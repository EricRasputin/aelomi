import "@blocknote/core/style.css";
import { SuggestionMenu } from "@blocknote/core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { setActiveEditor } from "../editor/active-editor";
import { renderSegments, type ShownBlock } from "../editor/body-reader";
import { inCode, type PickerKey, pickerTrigger } from "../editor/page-picker";
import { openPage, type PageSession } from "../editor/page-session";
import { markPageContent } from "../perf/cold-start";
import { type FocusRequest, resolvePageFocus } from "../shell/page-focus";
import { editingPausedCopy, storageErrorText } from "../storage-copy";
import { linkText, usePageTreeStore } from "../stores/page-tree";
import { usePickerStore } from "../stores/picker";
import { canEdit, useQuitStore } from "../stores/quit";
import { useRecentPagesStore } from "../stores/recent-pages";
import { useShellStore } from "../stores/shell";
import { PagePicker } from "./page-picker";

type ViewState = { kind: "loading" } | PageSession;

export const readOnlyCopy =
  "Read-only. This page holds content this version of Aelomi can't edit without changing it.";

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

export default function PageView({
  pageId,
  focusBlock,
  autoFocus,
}: {
  pageId: string;
  focusBlock: string | undefined;
  autoFocus: boolean;
}) {
  const [view, setView] = useState<ViewState>({ kind: "loading" });
  const host = useRef<HTMLDivElement>(null);
  const focusReady = useRef(false);
  const mountFocusConsumed = useRef(false);
  const editable = useQuitStore(canEdit);
  const registered = useQuitStore((state) => state.registered);
  const focusRequest = useShellStore((state) => state.focusRequest);

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

  useLayoutEffect(() => {
    if (!editor) return;
    editor.isEditable = editable;
    if (editable) markPageContent();
  }, [editor, editable]);

  useEffect(() => {
    if (!editor) return;
    const block = focusBlock && editor.getBlock(focusBlock);
    if (block) {
      const element = host.current?.querySelector<HTMLElement>(
        `[data-node-type="blockOuter"][data-id="${CSS.escape(focusBlock)}"]`,
      );
      element?.scrollIntoView({ block: "center" });
      if (element) flash(element);
    }
  }, [editor, focusBlock]);

  useLayoutEffect(() => {
    const deferred = !focusReady.current;
    focusReady.current = editor !== null && editable;
    if (!editor || !editable) return;
    const firstReady = !mountFocusConsumed.current;
    mountFocusConsumed.current = true;
    const pending = focusRequest;
    const intent: FocusRequest | null =
      pending?.pageId === pageId
        ? pending
        : firstReady && autoFocus
          ? focusBlock
            ? { pageId, target: "block", blockId: focusBlock }
            : { pageId, target: "body" }
          : null;
    if (!intent || intent.target === "title") return;
    const active = document.activeElement;
    const request = resolvePageFocus(intent, {
      kind: "bodyFocus",
      deferred,
      textFieldFocused:
        (active instanceof HTMLTextAreaElement ||
          active instanceof HTMLInputElement) &&
        host.current?.closest("article")?.contains(active) === true,
    });
    if (intent === pending) useShellStore.setState({ focusRequest: null });
    if (!request) return;
    if (request.target === "bodyStart") {
      const first = editor.document[0];
      if (first) editor.setTextCursorPosition(first, "start");
    } else if (request.target === "block") {
      const block = editor.getBlock(request.blockId);
      if (block) editor.setTextCursorPosition(block, "start");
    }
    editor.focus();
  }, [editor, editable, focusRequest, pageId, focusBlock, autoFocus]);

  return (
    <>
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
    </>
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
