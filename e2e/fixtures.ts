import { test as base } from "@e2e-dev/web";
import { guardWebSocket } from "../tests/guard-websocket";
import { isLocalUrl } from "../tests/is-local-url";
import { acceptanceContext } from "./browser";

// e2e starts every extended fixture, even when the test does not request its value.
export const test = base.extend<{ networkGuard: undefined }>({
  networkGuard: async ({ browser }, use) => {
    const requests: string[] = [];
    await browser.route("**", async (route) => {
      if (isLocalUrl(route.request.url)) return route.fallback();
      requests.push(`${route.request.method} ${route.request.url}`);
      await route.abort();
    });
    const context = acceptanceContext();
    await context.routeWebSocket(/.*/, (socket) =>
      guardWebSocket(socket, requests),
    );
    await use(undefined);
    if (acceptanceContext() !== context)
      throw new Error(
        "SHELL-13 acceptance context was replaced. Acceptance tests cannot clear or restore browser state. Use a separate test with fresh setup.",
      );
    if (requests.length > 0) {
      throw new Error(`SHELL-13 non-local requests:\n${requests.join("\n")}`);
    }
  },
});
