import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "../command-dispatcher";
import { commands } from "../ipc";
import { usePageTreeStore } from "../stores/page-tree";
import { usePaletteStore } from "../stores/palette";
import { acceptsUserCommands, canEdit, useQuitStore } from "../stores/quit";
import { useRecordCommandStore } from "../stores/record-commands";
import { useShellStore } from "../stores/shell";

export function usePageCommands() {
  const navigate = useNavigate();
  const pendingCreation = useShellStore((state) => state.pendingCreation);
  const retained = useRecordCommandStore((state) => state.retained);
  useEffect(() => {
    if (!pendingCreation || retained.has(pendingCreation.operationId)) return;
    let current = true;
    void usePageTreeStore
      .getState()
      .refresh()
      .then((source) => {
        if (!current || source.kind !== "ready") return;
        if (!source.byId.has(pendingCreation.pageId))
          useShellStore.setState({
            pendingCreation: null,
            message: "A new page couldn't be created. Try again.",
          });
      });
    return () => {
      current = false;
    };
  }, [pendingCreation, retained]);
  const open = useCallback(
    (pageId: string, target: "body" | "title" = "body") => {
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
        enabled: () => ready() && !useShellStore.getState().creating,
        run: () => {
          if (usePaletteStore.getState().open)
            flushSync(() => usePaletteStore.getState().hide());
          useShellStore.setState({ creating: true, message: null });
          void (async () => {
            try {
              const reply = await commands.pageCreate(null);
              if (reply.status === "error")
                throw new Error(JSON.stringify(reply.error));
              if (reply.status === "retrying") {
                useShellStore.setState({
                  pendingCreation: {
                    pageId: reply.pageId,
                    operationId: reply.operationId,
                  },
                });
                open(reply.pageId, "title");
              } else {
                await usePageTreeStore.getState().refresh();
                open(reply.data, "title");
              }
            } catch {
              useShellStore.setState({
                message: "A new page couldn't be created. Try again.",
              });
            } finally {
              useShellStore.setState({ creating: false });
            }
          })();
        },
      }),
      commandDispatcher.register("trashPage", {
        enabled: () => {
          const { source } = usePageTreeStore.getState();
          const { activePageId } = useShellStore.getState();
          return (
            ready() &&
            activePageId !== null &&
            source.kind === "ready" &&
            source.byId.has(activePageId)
          );
        },
        run: () => {
          const { activePageId } = useShellStore.getState();
          if (!activePageId) return;
          useShellStore.setState({ message: null });
          void commands
            .pageTrash(activePageId)
            .then((reply) => {
              if (reply.status === "error")
                useShellStore.setState({
                  message: "This page couldn't be moved to trash. Try again.",
                });
            })
            .catch(() =>
              useShellStore.setState({
                message: "This page couldn't be moved to trash. Try again.",
              }),
            );
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
