import { test as base, expect } from "@playwright/test";
import { isLocalUrl } from "../is-local-url";

export type { Page } from "@playwright/test";
export { expect };
export const test = base.extend<{ nonLocalRequests: string[] }>({
  nonLocalRequests: [
    async ({ context }, use) => {
      const requests: string[] = [];
      context.on("request", (request) => {
        if (!isLocalUrl(request.url())) {
          requests.push(`${request.method()} ${request.url()}`);
        }
      });
      await context.route("**/*", (route) =>
        isLocalUrl(route.request().url()) ? route.fallback() : route.abort(),
      );
      await context.routeWebSocket(/.*/, (socket) => {
        if (isLocalUrl(socket.url())) {
          socket.connectToServer();
        } else {
          requests.push(`WS ${socket.url()}`);
          void socket.close();
        }
      });
      await use(requests);
      expect(requests, "SHELL-13 non-local requests").toEqual([]);
    },
    { auto: true },
  ],
});
