import * as Tooltip from "@radix-ui/react-tooltip";
import {
  createRootRoute,
  createRoute,
  createRouter,
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
      <h1 className="text-3xl font-semibold tracking-tight">
        A space to think.
      </h1>
      <p className="mt-3 text-base text-[var(--muted)]">Welcome to Aelomi.</p>
      <p role="status" className="mt-8 text-base font-medium tabular-nums">
        {text}
      </p>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <button
            type="button"
            disabled={status.kind === "pending"}
            onClick={() => void ping()}
            className="mt-5 rounded-md border border-[var(--border)] bg-[var(--button)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--button-hover)] focus-visible:outline-2 focus-visible:outline-offset-4 disabled:opacity-50"
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
    </section>
  );
}

const rootRoute = createRootRoute({ component: RootLayout });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: IndexScreen,
});

export const router = createRouter({
  routeTree: rootRoute.addChildren([indexRoute]),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
