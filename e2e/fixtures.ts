import { test as base } from "@e2e-dev/web";
import { isLocalUrl } from "../tests/is-local-url";

// e2e starts every extended fixture, even when the test does not request its value.
export const test = base.extend<{ networkGuard: undefined }>({
  networkGuard: async ({ browser }, use) => {
    const requests: string[] = [];
    await browser.route("**", async (route) => {
      if (isLocalUrl(route.request.url)) return route.fallback();
      requests.push(`${route.request.method} ${route.request.url}`);
      await route.abort();
    });
    await use(undefined);
    if (requests.length > 0) {
      throw new Error(`SHELL-13 non-local requests:\n${requests.join("\n")}`);
    }
  },
});
