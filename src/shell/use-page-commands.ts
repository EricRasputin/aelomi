import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "../command-dispatcher";
import { commands } from "../ipc";
import { usePageTreeStore } from "../stores/page-tree";
import { usePaletteStore } from "../stores/palette";
import { canEdit, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";
import { activateNewPage, forgetEmptyFailuresExcept } from "./new-pages";
import { subtreeIds } from "./page-navigation";
import { canOpenPage, pagePresence } from "./page-presence";

export function useOpenPage() {
  const navigate = useNavigate();
  const open = useCallback(
    (
      pageId: string,
      target: "body" | "title" = "body",
      search: { block?: string } = {},
    ) => {
      if (!canOpenPage(pageId)) return;
      forgetEmptyFailuresExcept(pageId);
      flushSync(() => {
        useShellStore.getState().requestFocus({ pageId, target });
        useShellStore.setState({
          activePageId: pageId,
          message: null,
        });
      });
      void navigate({ to: "/page/$pageId", params: { pageId }, search });
    },
    [navigate],
  );
  return open;
}

export function usePageCommands() {
  const open = useOpenPage();
  useEffect(() => {
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
        enabled: newPageEnabled,
        run: () => {
          if (usePaletteStore.getState().open)
            flushSync(() => usePaletteStore.getState().hide());
          const current = useShellStore.getState().activePageId;
          const id = activateNewPage(current);
          open(id, "title");
        },
      }),
      commandDispatcher.register("trashPage", {
        enabled: () => {
          const { source } = usePageTreeStore.getState();
          const { activePageId } = useShellStore.getState();
          return (
            newPageEnabled() &&
            useShellStore.getState().trashRequest?.status !== "pending" &&
            activePageId !== null &&
            source.kind === "ready" &&
            pagePresence(activePageId).kind === "listed"
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
              if (reply.status === "error" && reply.error === "Abandoned")
                return;
              if (reply.status === "error" && reply.error !== "NotActive")
                throw new Error(JSON.stringify(reply.error));
              useShellStore.setState((state) => ({
                trashed: new Set([...state.trashed, ...affected]),
                trashRequest: null,
              }));
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
  commandDispatcher.dispatch("newPage", {
    source: "button",
    target: document.activeElement,
    isComposing: false,
  });
}

export function newPageEnabled() {
  return (
    canEdit(useQuitStore.getState()) &&
    usePageTreeStore.getState().source.kind === "ready"
  );
}
