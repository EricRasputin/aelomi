import type { PingReply } from "./bindings";

export function formatPingReply(reply: PingReply): string {
  return `IPC ok · core ${reply.core_version} · seq ${reply.seq}`;
}
