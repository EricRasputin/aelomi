import { expect, test, vi } from "vitest";
import { guardWebSocket } from "./guard-websocket";

test("the acceptance socket route records and closes a remote socket while connecting loopback", () => {
  const requests: string[] = [];
  const connectToServer = vi.fn();
  const close = vi.fn().mockResolvedValue(undefined);
  guardWebSocket(
    { url: () => "wss://example.com/guard-canary", connectToServer, close },
    requests,
  );
  expect(requests).toEqual(["WS wss://example.com/guard-canary"]);
  expect(close.mock.calls).toEqual([[]]);
  expect(connectToServer.mock.calls).toEqual([]);
  guardWebSocket(
    { url: () => "ws://127.0.0.1:1420/hmr", connectToServer, close },
    requests,
  );
  expect(connectToServer.mock.calls).toEqual([[]]);
  expect(close.mock.calls).toEqual([[]]);
  expect(requests).toEqual(["WS wss://example.com/guard-canary"]);
});
