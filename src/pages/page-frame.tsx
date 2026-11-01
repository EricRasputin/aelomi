import { lazyRouteComponent } from "@tanstack/react-router";
import { Suspense, useId, useLayoutEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import {
  pageTitleHistory,
  registerFieldHistory,
} from "../editor/field-history";
import { onQuitDrain } from "../editor/quit-drain";
import { openPageLink, recordPageLinkPointer } from "../editor/url-links";
import { retryNewPage } from "../shell/new-pages";
import { bodyReady, usePagePresence } from "../shell/page-presence";
import { useOpenPage } from "../shell/use-page-commands";
import {
  usePageTitle,
  usePageTitlesStore,
  useTitleRefusal,
} from "../stores/page-titles";
import { displayTitle } from "../stores/page-tree";
import { canEdit, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";

const PageView = lazyRouteComponent(() => import("./page-view"));
export const preloadPageBody = () => PageView.preload?.();
export function PageFrame({
  pageId,
  focusBlock,
}: {
  pageId: string;
  focusBlock?: string;
}) {
  const ready = bodyReady(usePagePresence(pageId));
  const gatedAtMount = useRef(!ready);
  return (
    <article
      className="page"
      aria-label="Page"
      onPointerDownCapture={recordPageLinkPointer}
      onClickCapture={openPageLink}
      onAuxClickCapture={openPageLink}
      onContextMenuCapture={openPageLink}
    >
      <PageTitle
        pageId={pageId}
        focusBody={() => {
          useShellStore
            .getState()
            .requestFocus({ pageId, target: "bodyStart" });
        }}
      />
      {ready ? (
        <Suspense fallback={<div className="page-body" />}>
          <PageView
            pageId={pageId}
            focusBlock={focusBlock}
            autoFocus={!gatedAtMount.current}
          />
        </Suspense>
      ) : (
        <div className="page-body" />
      )}
    </article>
  );
}

function PageTitle({
  pageId,
  focusBody,
}: {
  pageId: string;
  focusBody: () => void;
}) {
  const title = usePageTitle(pageId);
  const open = useOpenPage();
  const input = useRef<HTMLTextAreaElement>(null);
  const lastTitle = useRef(title ?? "");
  const shownTitle = title ?? lastTitle.current;
  const history = useRef(pageTitleHistory(pageId, shownTitle));
  const composing = useRef(false);
  const [frozen, setFrozen] = useState(
    () => useQuitStore.getState().decision.kind !== "open",
  );
  const decision = useQuitStore((state) => state.decision.kind);
  const refusal = useTitleRefusal(pageId);
  const presence = usePagePresence(pageId);
  const editable = !frozen && refusal === null;
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
  useLayoutEffect(() => {
    if (
      request?.pageId !== pageId ||
      request.target !== "title" ||
      !editable ||
      title === null
    )
      return;
    const field = input.current;
    field?.focus();
    field?.setSelectionRange(title.length, title.length);
    useShellStore.setState({ focusRequest: null });
  }, [request, pageId, editable, title]);
  const noticeId = useId();
  const notice =
    refusal === "notCreated"
      ? "This page couldn't be created. Its title is kept here."
      : refusal === "trashing"
        ? "Moving to trash…"
        : refusal === "stale" ||
            (refusal === "missing" &&
              presence.kind === "missing" &&
              !presence.fresh)
          ? "Titles can't be edited until pages can be refreshed."
          : refusal === "connecting"
            ? "Editing pauses while Aelomi connects to its shell."
            : refusal === "closing" || frozen
              ? "Aelomi is closing. New actions are paused while pending changes finish."
              : error;
  const canRetry = useQuitStore(canEdit);
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
        aria-describedby={notice ? noticeId : undefined}
        placeholder="Untitled"
        dir="auto"
        value={shownTitle}
        readOnly={!editable || title === null}
        onFocus={() => useShellStore.getState().titleFocused(pageId)}
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
      {notice && (
        <p id={noticeId} className="page-notice">
          {notice}
        </p>
      )}
      {refusal === "notCreated" && (
        <button
          type="button"
          className="shell-button page-retry"
          disabled={!canRetry}
          onClick={() => {
            retryNewPage(pageId);
            open(pageId, "title");
          }}
        >
          Try again
        </button>
      )}
    </header>
  );
}
