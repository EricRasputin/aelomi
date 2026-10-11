import * as Tooltip from "@radix-ui/react-tooltip";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  lazyRouteComponent,
  Outlet,
} from "@tanstack/react-router";
import { useEffect, useLayoutEffect } from "react";
import { formatPingReply } from "./ipc/format-ping";
import { markIndexCommit } from "./perf/cold-start";
import { usePingStore } from "./stores/ping";

function RootLayout() {
  return (
    <Tooltip.Provider>
      <div className="flex min-h-screen flex-col">
        <header className="border-b border-[var(--border)] px-8 py-5 text-sm font-medium">
          Aelomi
        </header>
        <main className="flex flex-1 items-center justify-center px-8 py-12">
          <Outlet />
        </main>
      </div>
    </Tooltip.Provider>
  );
}

function IndexScreen() {
  const status = usePingStore((state) => state.status);
  const ping = usePingStore((state) => state.ping);

  useLayoutEffect(markIndexCommit, []);

  useEffect(() => {
    void ping();
  }, [ping]);

  const text =
    status.kind === "ready"
      ? formatPingReply(status.reply)
      : status.kind === "error"
        ? `IPC failed · ${status.message}`
        : "Connecting to the core…";

  return (
    <section className="w-full max-w-md">
      <p role="status" className="text-sm text-[var(--muted)] tabular-nums">
        {text}
      </p>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <button
            type="button"
            disabled={status.kind === "pending"}
            onClick={() => void ping()}
            className="mt-4 rounded-md border border-[var(--border)] bg-[var(--button)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--button-hover)] focus-visible:outline-2 focus-visible:outline-offset-4 disabled:opacity-50"
          >
            Ping again
          </button>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            sideOffset={8}
            className="rounded-md border border-[var(--border)] bg-[var(--background)] px-3 py-2 text-xs shadow-sm"
          >
            Check the connection to the Rust core
          </Tooltip.Content>
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

const keystrokeRoutes =
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
    ...keystrokeRoutes,
    ...devRoutes,
  ]),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
