import { commands as realCommands } from "./bindings";
import { installMock } from "./mock";

export const commands =
  import.meta.env.MODE === "mock" ? installMock() : realCommands;
export type { PingReply, PingRequest } from "./bindings";
