import { test as base } from "@e2e-dev/web";
import { isLocalUrl } from "../tests/is-local-url";

export const test = base.extend<{ nonLocalRequests: string[] }>({
  nonLocalRequests: async ({ browser }, use) => {
    const requests: string[] = [];
    await browser.route("**", async (route) => {
      if (isLocalUrl(route.request.url)) return route.fallback();
      requests.push(`${route.request.method} ${route.request.url}`);
      await route.abort();
    });
    await use(requests);
    if (requests.length > 0) {
      throw new Error(`SHELL-13 non-local requests:\n${requests.join("\n")}`);
    }
  },
});
