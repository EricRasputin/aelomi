import { Tooltip } from "@base-ui/react/tooltip";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  lazyRouteComponent,
  Outlet,
  useMatch,
  useNavigate,
  useRouterState,
} from "@tanstack/react-router";
import { useEffect, useId, useLayoutEffect, useRef } from "react";
import { CommandPalette } from "./components/command-palette";
import { SettingsDialog } from "./components/settings-dialog";
import { NewPageButton, Sidebar } from "./components/sidebar";
import {
  QuitConfirmation,
  SaveFailureBanner,
} from "./components/storage-notices";
import { StorageRefusal } from "./components/storage-refusal";
import { formatPingReply } from "./ipc/format-ping";
import { markIndexCommit, resolveEntry } from "./perf/cold-start";
import { pageAfterTrash } from "./shell/page-navigation";
import { usePageCommands } from "./shell/use-page-commands";
import { usePageTreeStore } from "./stores/page-tree";
import { usePingStore } from "./stores/ping";
import { useQuitStore } from "./stores/quit";
import { useRecentPagesStore } from "./stores/recent-pages";
import { useShellStore } from "./stores/shell";
import { useStorageStartupStore } from "./stores/storage-startup";
import { useTreeExpansionStore } from "./stores/tree-expansion";
import {
  useShortcutCommands,
  useWorkspaceShortcutCommands,
} from "./use-shortcut-commands";

function pingText(status: ReturnType<typeof usePingStore.getState>["status"]) {
  return status.kind === "ready"
    ? formatPingReply(status.reply)
    : status.kind === "error"
      ? `IPC failed · ${status.message}`
      : "Connecting to the core…";
}

function ShellStatus({ landmark }: { landmark: boolean }) {
  const status = usePingStore((state) => state.status);
  useEffect(() => {
    if (usePingStore.getState().seq === 0) void usePingStore.getState().ping();
  }, []);
  return landmark ? (
    <p role="status" className="sr-only">
      {pingText(status)}
    </p>
  ) : null;
}

function RootLayout() {
  useShortcutCommands();
  const startup = useStorageStartupStore((state) => state.startup);
  if (startup.kind !== "ready")
    return (
      <>
        <StorageRefusal startup={startup} />
        <ShellStatus landmark />
      </>
    );
  return <WorkspaceLayout />;
}

function WorkspaceLayout() {
  useWorkspaceShortcutCommands();
  const stress = useRouterState({
    select: (state) => state.location.pathname === "/dev/sidebar-stress",
  });
  const diagnostic = useRouterState({
    select: (state) => state.location.pathname === "/dev/ipc",
  });
  const sidebarVisible = useShellStore((state) => state.sidebarVisible);
  const message = useShellStore((state) => state.message);
  const closing = useQuitStore((state) => state.decision.kind === "draining");
  const openPage = usePageCommands();
  useWorkspaceNavigation();
  useEffect(() => {
    void useTreeExpansionStore.getState().load();
  }, []);
  return (
    <Tooltip.Provider delay={700} timeout={300}>
      <div className="flex h-screen flex-col">
        <header className="shrink-0 border-b border-[var(--border)] px-8 py-5 text-sm font-medium">
          Aelomi
          {import.meta.env.DEV && (
            <Link
              to="/dev/settings"
              className="ml-4 text-xs text-[var(--muted)] underline"
            >
              Settings probe
            </Link>
          )}
          {import.meta.env.DEV && (
            <Link
              to="/dev/storage"
              className="ml-4 text-xs text-[var(--muted)] underline"
            >
              Storage probe
            </Link>
          )}
        </header>
        <ShellStatus landmark={!diagnostic} />
        <SaveFailureBanner />
        <main
          tabIndex={-1}
          className="flex min-h-0 flex-1 flex-col overflow-y-auto"
        >
          <Outlet />
        </main>
        <SettingsDialog />
        {(message || closing) && (
          <p role="status" className="shell-notice">
            {closing
              ? "Aelomi is closing. New actions are paused while pending changes finish."
              : message}
          </p>
        )}
        <div className="workspace" data-workspace>
          {!stress && <Sidebar openPage={openPage} hidden={!sidebarVisible} />}
          <main tabIndex={-1} className="editor-pane">
            <Outlet />
          </main>
        </div>
        <CommandPalette />
        <QuitConfirmation />
      </div>
    </Tooltip.Provider>
  );
}

function useWorkspaceNavigation() {
  const navigate = useNavigate();
  const match = useMatch({ from: "/page/$pageId", shouldThrow: false });
  const pageId = match?.params.pageId ?? null;
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const source = usePageTreeStore((state) => state.source);
  const previous = useRef(source);
  const entered = useRef(false);
  useLayoutEffect(() => {
    useShellStore.setState({ activePageId: pageId });
  }, [pageId]);
  useEffect(() => {
    if (source.kind !== "ready") return;
    const before = previous.current;
    if (!pageId || source.byId.has(pageId)) previous.current = source;
    if (!entered.current) {
      entered.current = true;
      if (pathname === "/") {
        const first = source.tree.pages[0];
        resolveEntry(first ? "page" : "index");
        if (first)
          void navigate({
            to: "/page/$pageId",
            params: { pageId: first.id },
            search: {},
            replace: true,
          });
        return;
      }
      resolveEntry(pageId ? "page" : "index");
    }
    if (!pageId || source.byId.has(pageId)) return;
    const next = pageAfterTrash({
      pageId,
      before: before.kind === "ready" ? before.tree.pages : [],
      active: source.tree.pages,
      openedAt: useRecentPagesStore.getState().openedAt,
    });
    if (next) {
      useShellStore.getState().requestFocus({ pageId: next, target: "body" });
      void navigate({
        to: "/page/$pageId",
        params: { pageId: next },
        search: {},
        replace: true,
      });
    } else void navigate({ to: "/", replace: true });
  }, [source, pageId, pathname, navigate]);
}

export const workspaceCopy = {
  unreadable:
    "Aelomi can't read this workspace's page list, so no page can open. Nothing on disk was changed.",
};

function IndexScreen() {
  const source = usePageTreeStore((state) => state.source);
  useLayoutEffect(() => {
    if (source.kind !== "loading") markIndexCommit();
  }, [source.kind]);
  return (
    <section className="empty-state" aria-label="No page open">
      {source.kind === "error" ? (
        <div role="alert" className="workspace-alert">
          <p>{workspaceCopy.unreadable}</p>
          <p className="workspace-alert-reason">{source.message}</p>
        </div>
      ) : source.kind === "ready" ? (
        <>
          <h1>No page open</h1>
          <p>Start with a new page.</p>
          <NewPageButton />
        </>
      ) : null}
    </section>
  );
}

function ConnectionProbe() {
  const tooltipId = useId();
  const status = usePingStore((state) => state.status);
  return (
    <section className="m-auto w-full max-w-md px-8 py-12">
      <p role="status" className="text-sm text-[var(--muted)] tabular-nums">
        {pingText(status)}
      </p>
      <Tooltip.Root>
        <Tooltip.Trigger
          aria-describedby={tooltipId}
          render={
            <button
              type="button"
              disabled={status.kind === "pending"}
              onClick={() => void usePingStore.getState().ping()}
              className="shell-button mt-4"
            />
          }
        >
          Ping again
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Positioner sideOffset={8}>
            <Tooltip.Popup
              id={tooltipId}
              role="tooltip"
              className="rounded-md border border-[var(--border)] bg-[var(--background)] px-3 py-2 text-xs shadow-sm"
            >
              Check the connection to the Rust core
            </Tooltip.Popup>
          </Tooltip.Positioner>
        </Tooltip.Portal>
      </Tooltip.Root>
    </section>
  );
}
const rootRoute = createRootRoute({ component: RootLayout });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: IndexScreen,
});

export type PageSearch = { block?: string };

const pageRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/page/$pageId",
  validateSearch: (search: Record<string, unknown>): PageSearch =>
    typeof search.block === "string" ? { block: search.block } : {},
  component: lazyRouteComponent(() => import("./pages/page-route")),
});

const mockRoutes =
  import.meta.env.MODE === "mock"
    ? [
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/dev/sidebar-stress",
          component: lazyRouteComponent(() => import("./dev/sidebar-stress")),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/perf/keystroke/empty",
          component: lazyRouteComponent(() => import("./perf/keystroke-empty")),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/perf/keystroke/page-5000-blocks",
          component: lazyRouteComponent(
            () => import("./perf/keystroke-page-5000"),
          ),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/perf/keystroke/layout-delay",
          component: lazyRouteComponent(
            () => import("./perf/keystroke-layout-delay"),
          ),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/perf/keystroke/editor-empty",
          component: lazyRouteComponent(
            () => import("./perf/keystroke-editor"),
            "KeystrokeEditorEmpty",
          ),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/perf/keystroke/editor-page-5000-blocks",
          component: lazyRouteComponent(
            () => import("./perf/keystroke-editor"),
            "KeystrokeEditorPage5000",
          ),
        }),
      ]
    : [];

const devRoutes =
  import.meta.env.DEV || import.meta.env.MODE === "mock"
    ? [
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/dev/ipc",
          component: ConnectionProbe,
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/dev/settings",
          component: lazyRouteComponent(() => import("./dev/settings-probe")),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: "/dev/storage",
          component: lazyRouteComponent(() => import("./dev/storage-probe")),
        }),
      ]
    : [];

export const router = createRouter({
  routeTree: rootRoute.addChildren([
    indexRoute,
    pageRoute,
    ...mockRoutes,
    ...devRoutes,
  ]),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
