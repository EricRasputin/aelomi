import type { WebSocketRoute } from "@playwright/test";
import { isLocalUrl } from "./is-local-url";

export function guardWebSocket(
  socket: Pick<WebSocketRoute, "url" | "connectToServer" | "close">,
  requests: string[],
) {
  if (isLocalUrl(socket.url())) socket.connectToServer();
  else {
    requests.push(`WS ${socket.url()}`);
    void socket.close();
  }
}
