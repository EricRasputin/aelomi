import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { flushSync } from "react-dom";
import { commandDispatcher } from "../command-dispatcher";
import { getActiveEditor } from "../editor/active-editor";
import { commands } from "../ipc";
import { usePageTreeStore } from "../stores/page-tree";
import { usePaletteStore } from "../stores/palette";
import { acceptsUserCommands, canEdit, useQuitStore } from "../stores/quit";
import { useShellStore } from "../stores/shell";

export function usePageCommands() {
  const navigate = useNavigate();
  const open = useCallback(
    (pageId: string, target: "body" | "title" = "body") => {
      useShellStore.getState().requestFocus({ pageId, target });
      if (
        useShellStore.getState().activePageId === pageId &&
        target === "body"
      ) {
        getActiveEditor()?.focus();
      }
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
        run: () => useShellStore.getState().toggleSidebar(),
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
              await usePageTreeStore.getState().refresh();
              open(reply.data, "title");
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
