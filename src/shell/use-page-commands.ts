import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "../command-dispatcher";
import { commands } from "../ipc";
import { trackCommand } from "../ipc/pending-commands";
import { usePageTitlesStore } from "../stores/page-titles";
import { usePageTreeStore } from "../stores/page-tree";
import { usePaletteStore } from "../stores/palette";
import { acceptsUserCommands, canEdit, useQuitStore } from "../stores/quit";
import { pendingCreate } from "../stores/record-commands";
import { useShellStore } from "../stores/shell";
import { subtreeIds } from "./page-navigation";
import { canOpenPage } from "./page-opening";

export function usePageCommands() {
  const navigate = useNavigate();
  const open = useCallback(
    (pageId: string, target: "body" | "title" = "body") => {
      if (!canOpenPage(pageId)) return;
      useShellStore.setState({ revealedPageId: null });
      useShellStore.getState().requestFocus({ pageId, target });
      void navigate({ to: "/page/$pageId", params: { pageId }, search: {} });
    },
    [navigate],
  );
  useEffect(() => {
    const ready = () =>
      canEdit(useQuitStore.getState()) &&
      usePageTreeStore.getState().source.kind === "ready";
    const unregister = [
      commandDispatcher.register("toggleSidebar", {
        run: () => {
          const shell = useShellStore.getState();
          if (
            shell.sidebarVisible &&
            document.activeElement?.closest(".sidebar")
          ) {
            if (shell.activePageId)
              shell.requestFocus({
                pageId: shell.activePageId,
                target: "body",
              });
            else document.querySelector<HTMLElement>("main")?.focus();
          }
          shell.toggleSidebar();
        },
      }),
      commandDispatcher.register("newPage", {
        enabled: () =>
          ready() &&
          !pendingCreate(null) &&
          useShellStore.getState().createIntent?.stage.kind !== "opening",
        run: (context) => {
          if (usePaletteStore.getState().open)
            flushSync(() => usePaletteStore.getState().hide());
          const shell = useShellStore.getState();
          const intent = {
            originPageId: shell.activePageId,
            editorFocus: shell.editorFocus,
            title:
              shell.createIntent?.stage.kind === "failed"
                ? shell.createIntent.title
                : "",
            source: context.source,
            stage: { kind: "pending" as const },
          };
          flushSync(() =>
            useShellStore.setState({ message: null, createIntent: intent }),
          );
          void trackCommand(
            (async () => {
              try {
                const reply = await commands.pageCreate(null);
                if (reply.status === "error")
                  throw new Error(JSON.stringify(reply.error));
                await usePageTreeStore.getState().refresh();
                const buffered =
                  useShellStore.getState().createIntent?.title ?? "";
                useShellStore.setState((state) => ({
                  createIntent: state.createIntent && {
                    ...state.createIntent,
                    stage: { kind: "opening", pageId: reply.data },
                  },
                }));
                if (buffered)
                  await usePageTitlesStore
                    .getState()
                    .rename(reply.data, buffered);
                const current = useShellStore.getState();
                const stillHere =
                  current.activePageId === intent.originPageId &&
                  current.editorFocus === intent.editorFocus;
                if (
                  stillHere &&
                  acceptsUserCommands(useQuitStore.getState().decision)
                ) {
                  open(reply.data, "title");
                } else {
                  useShellStore.setState({
                    createIntent: null,
                    revealedPageId: reply.data,
                    message: "New page created.",
                  });
                }
              } catch {
                useShellStore.setState((state) => ({
                  createIntent: state.createIntent && {
                    ...state.createIntent,
                    stage: { kind: "failed" },
                  },
                  message: "A new page couldn't be created. Try again.",
                }));
              }
            })(),
          );
        },
      }),
      commandDispatcher.register("trashPage", {
        enabled: () => {
          const { source } = usePageTreeStore.getState();
          const { activePageId } = useShellStore.getState();
          return (
            ready() &&
            useShellStore.getState().trashRequest?.status !== "pending" &&
            activePageId !== null &&
            source.kind === "ready" &&
            source.byId.has(activePageId)
          );
        },
        run: () => {
          const { activePageId } = useShellStore.getState();
          if (!activePageId) return;
          const { source } = usePageTreeStore.getState();
          const affected = subtreeIds(
            source.kind === "ready" ? source.tree.pages : [],
            activePageId,
          );
          useShellStore.setState({
            message: null,
            trashRequest: { pageId: activePageId, affected, status: "pending" },
          });
          void (async () => {
            try {
              const reply = await commands.pageTrash(activePageId);
              if (reply.status === "error")
                throw new Error(JSON.stringify(reply.error));
              await usePageTreeStore.getState().refresh();
              useShellStore.setState({ trashRequest: null });
            } catch {
              useShellStore.setState({
                message: "This page couldn't be moved to trash. Try again.",
                trashRequest: {
                  pageId: activePageId,
                  affected,
                  status: "error",
                },
              });
            }
          })();
        },
      }),
    ];
    return () => {
      for (const stop of unregister) stop();
    };
  }, [open]);
  return open;
}

export function newPage() {
  if (!acceptsUserCommands(useQuitStore.getState().decision)) {
    useShellStore.setState({
      message: "Aelomi is closing. Keep the app open to create a page.",
    });
    return;
  }
  commandDispatcher.dispatch("newPage", {
    source: "button",
    target: document.activeElement,
    isComposing: false,
  });
}
