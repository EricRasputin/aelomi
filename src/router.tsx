import { Tooltip } from "@base-ui/react/tooltip";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  lazyRouteComponent,
  Outlet,
  useNavigate,
  useRouterState,
} from "@tanstack/react-router";
import { useEffect, useId, useLayoutEffect } from "react";
import { flushSync } from "react-dom";
import { CommandPalette } from "./components/command-palette";
import {
  QuitConfirmation,
  SaveFailureBanner,
} from "./components/storage-notices";
import { formatPingReply } from "./ipc/format-ping";
import { markIndexCommit, resolveEntry } from "./perf/cold-start";
import { shortcutFor } from "./shortcuts";
import { usePageTreeStore } from "./stores/page-tree";
import { usePaletteStore } from "./stores/palette";
import { usePingStore } from "./stores/ping";

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
  if (!landmark) return null;
  return (
    <p role="status" className="sr-only">
      {pingText(status)}
    </p>
  );
}

function useGlobalShortcuts() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (shortcutFor(event) !== "openPalette") return;
      event.preventDefault();
      event.stopPropagation();
      flushSync(() => usePaletteStore.getState().show(document.activeElement));
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);
}

function RootLayout() {
  const onIndex = useRouterState({
    select: (state) => state.location.pathname === "/",
  });
  useGlobalShortcuts();
  return (
    <Tooltip.Provider delay={700} timeout={300}>
      <div className="flex h-screen flex-col">
        <header className="shrink-0 border-b border-[var(--border)] px-8 py-5 text-sm font-medium">
          Aelomi
        </header>
        <ShellStatus landmark={!onIndex} />
        <SaveFailureBanner />
        <main
          tabIndex={-1}
          className="flex min-h-0 flex-1 flex-col overflow-y-auto"
        >
          <Outlet />
        </main>
        <CommandPalette />
        <QuitConfirmation />
      </div>
    </Tooltip.Provider>
  );
}

function useEntryPage() {
  const navigate = useNavigate();
  const source = usePageTreeStore((state) => state.source);
  useEffect(() => {
    if (source.kind === "loading") return;
    const first = source.kind === "ready" ? source.tree.pages[0] : undefined;
    if (!first) {
      resolveEntry("index");
      return;
    }
    resolveEntry("page");
    void navigate({
      to: "/page/$pageId",
      params: { pageId: first.id },
      search: {},
      replace: true,
    });
  }, [source, navigate]);
}

function IndexScreen() {
  const tooltipId = useId();
  const status = usePingStore((state) => state.status);
  const ping = usePingStore((state) => state.ping);

  useLayoutEffect(markIndexCommit, []);
  useEntryPage();

  return (
    <section className="m-auto w-full max-w-md px-8 py-12">
      <p role="status" className="text-sm text-[var(--muted)] tabular-nums">
        {pingText(status)}
      </p>
      <Tooltip.Root>
        <Tooltip.Trigger
          disabled={status.kind === "pending"}
          aria-describedby={tooltipId}
          render={
            <button
              type="button"
              disabled={status.kind === "pending"}
              onClick={() => void ping()}
              className="mt-4 rounded-md border border-[var(--border)] bg-[var(--button)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--button-hover)] focus-visible:outline-2 focus-visible:outline-offset-4 disabled:opacity-50"
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
      {import.meta.env.DEV && (
        <Link
          to="/dev/storage"
          className="mt-4 block text-sm text-[var(--muted)] underline"
        >
          Storage probe
        </Link>
      )}
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

const devRoutes = import.meta.env.DEV
  ? [
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
