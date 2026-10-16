import "@blocknote/core/style.css";
import { SuggestionMenu } from "@blocknote/core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { setActiveEditor } from "../editor/active-editor";
import { renderSegments, type ShownBlock } from "../editor/body-reader";
import {
  pageTitleHistory,
  registerFieldHistory,
} from "../editor/field-history";
import { inCode, type PickerKey, pickerTrigger } from "../editor/page-picker";
import { openPage, type PageSession } from "../editor/page-session";
import { onQuitDrain } from "../editor/quit-drain";
import { openPageLink, recordPageLinkPointer } from "../editor/url-links";
import { markPageContent } from "../perf/cold-start";
import { editingPausedCopy, storageErrorText } from "../storage-copy";
import { usePageTitle, usePageTitlesStore } from "../stores/page-titles";
import { displayTitle, linkText, usePageTreeStore } from "../stores/page-tree";
import { usePickerStore } from "../stores/picker";
import { canEdit, useQuitStore } from "../stores/quit";
import { useRecentPagesStore } from "../stores/recent-pages";
import { useShellStore } from "../stores/shell";
import { PagePicker } from "./page-picker";

type ViewState = { kind: "loading" } | PageSession;

export const readOnlyCopy =
  "Read-only. This page holds content this version of Aelomi can't edit without changing it.";

function PageTitle({
  pageId,
  focusBody,
}: {
  pageId: string;
  focusBody: () => void;
}) {
  const title = usePageTitle(pageId);
  const input = useRef<HTMLTextAreaElement>(null);
  const lastTitle = useRef(title ?? "");
  const shownTitle = title ?? lastTitle.current;
  const history = useRef(pageTitleHistory(pageId, shownTitle));
  const composing = useRef(false);
  const [frozen, setFrozen] = useState(
    () => useQuitStore.getState().decision.kind !== "open",
  );
  const decision = useQuitStore((state) => state.decision.kind);
  const registered = useQuitStore((state) => state.registered);
  const movingToTrash = useShellStore(
    (state) =>
      state.trashRequest?.status === "pending" &&
      state.trashRequest.affected.has(pageId),
  );
  const sourceError = usePageTreeStore(
    (state) =>
      state.source.kind === "error" ||
      (state.source.kind === "ready" && Boolean(state.source.error)),
  );
  const editable =
    registered &&
    !frozen &&
    !movingToTrash &&
    !sourceError &&
    (decision === "open" || decision === "draining");
  useLayoutEffect(() => {
    if (decision === "open") setFrozen(false);
  }, [decision]);
  useLayoutEffect(() => {
    const field = input.current;
    if (!field) return;
    return onQuitDrain({
      finish: () => {
        if (composing.current) field.blur();
      },
      freeze: () => {
        if (!field.readOnly && field.value !== history.current.value()) {
          history.current.record(field.value);
          void usePageTitlesStore.getState().rename(pageId, field.value);
        }
        field.readOnly = true;
        setFrozen(true);
      },
    });
  }, [pageId]);
  useLayoutEffect(() => {
    const field = input.current;
    if (field) {
      field.style.height = "auto";
      field.style.height = `${field.scrollHeight}px`;
    }
    if (title !== null) {
      lastTitle.current = title;
      if (history.current.value() !== title) history.current.reset(title);
    }
  }, [title]);
  useLayoutEffect(() => {
    const field = input.current;
    if (!field) return;
    let width = field.clientWidth;
    const observer = new ResizeObserver(() => {
      if (width === field.clientWidth) return;
      width = field.clientWidth;
      field.style.height = "auto";
      field.style.height = `${field.scrollHeight}px`;
    });
    observer.observe(field);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const field = input.current;
    if (!field) return;
    const move = (action: "undo" | "redo") => {
      if (field.readOnly) return;
      const value = history.current.move(action);
      if (value === field.value) return;
      flushSync(() => {
        void usePageTitlesStore.getState().rename(pageId, value);
      });
      field.setSelectionRange(value.length, value.length);
    };
    const stop = registerFieldHistory(field, move);
    const beforeinput = (event: InputEvent) => {
      if (
        event.inputType !== "historyUndo" &&
        event.inputType !== "historyRedo"
      )
        return;
      event.preventDefault();
      move(event.inputType === "historyUndo" ? "undo" : "redo");
    };
    field.addEventListener("beforeinput", beforeinput);
    return () => {
      stop();
      field.removeEventListener("beforeinput", beforeinput);
    };
  }, [pageId]);
  const error = usePageTitlesStore((state) => state.drafts.get(pageId)?.error);
  const request = useShellStore((state) => state.focusRequest);
  const createIntent = useShellStore((state) => state.createIntent);
  useLayoutEffect(() => {
    if (
      request?.pageId === pageId &&
      request.target === "title" &&
      editable &&
      title !== null
    ) {
      if (
        createIntent?.stage.kind === "opening" &&
        createIntent.stage.pageId === pageId &&
        createIntent.title !== title
      ) {
        void usePageTitlesStore.getState().rename(pageId, createIntent.title);
        return;
      }
      input.current?.focus();
      useShellStore.setState((state) => ({
        focusRequest: null,
        createIntent:
          state.createIntent?.stage.kind === "opening" &&
          state.createIntent.stage.pageId === pageId
            ? null
            : state.createIntent,
      }));
    }
  }, [request, pageId, editable, title, createIntent]);
  return (
    <header className="page-header">
      <h1 className="sr-only" dir="auto">
        {displayTitle(shownTitle)}
      </h1>
      <textarea
        ref={input}
        rows={1}
        className="page-title"
        data-testid="page-title"
        aria-label="Page title"
        placeholder="Untitled"
        dir="auto"
        value={shownTitle}
        readOnly={!editable || title === null}
        onCompositionStart={() => {
          composing.current = true;
        }}
        onCompositionEnd={() => {
          composing.current = false;
        }}
        onChange={(event) => {
          if (event.currentTarget.readOnly || title === null) return;
          history.current.record(event.target.value);
          void usePageTitlesStore.getState().rename(pageId, event.target.value);
        }}
        onKeyDown={(event) => {
          if (
            event.key === "Enter" &&
            !event.nativeEvent.isComposing &&
            event.keyCode !== 229 &&
            !composing.current
          ) {
            event.preventDefault();
            useShellStore.setState({ focusRequest: null });
            focusBody();
          }
        }}
      />
      {movingToTrash && (
        <p className="page-notice" role="status">
          Moving to Trash…
        </p>
      )}
      {sourceError && (
        <p className="page-notice">
          Titles can't be edited until pages can be refreshed.
        </p>
      )}
      {error && (
        <p className="page-notice" role="alert">
          {error}
        </p>
      )}
    </header>
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
    }
  }, [editor, focusBlock]);

  useEffect(() => {
    if (!editor || !editable) return;
    const request = useShellStore.getState().focusRequest;
    if (request?.pageId === pageId && request.target === "title") return;
    if (
      !focusBlock &&
      !(
        document.activeElement instanceof HTMLTextAreaElement ||
        document.activeElement instanceof HTMLInputElement
      )
    )
      editor.focus();
  }, [editor, editable, pageId, focusBlock]);

  useEffect(() => {
    if (
      !editor ||
      !editable ||
      focusRequest?.pageId !== pageId ||
      focusRequest.target === "title"
    )
      return;
    if (focusRequest.target === "bodyStart") {
      const first = editor.document[0];
      if (first) editor.setTextCursorPosition(first, "start");
    }
    editor.focus();
    useShellStore.setState({ focusRequest: null });
  }, [editor, editable, focusRequest, pageId]);

  return (
    <article
      className="page"
      aria-label="Page"
      onFocusCapture={(event) => {
        if (event.target.closest('[contenteditable="true"], textarea, input'))
          useShellStore.setState((state) => ({
            editorFocus: state.editorFocus + 1,
          }));
      }}
      onPointerDownCapture={recordPageLinkPointer}
      onClickCapture={openPageLink}
      onAuxClickCapture={openPageLink}
      onContextMenuCapture={openPageLink}
    >
      <PageTitle
        key={pageId}
        pageId={pageId}
        focusBody={() =>
          useShellStore.getState().requestFocus({ pageId, target: "bodyStart" })
        }
      />
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
