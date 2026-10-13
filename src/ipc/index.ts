import { commands as realCommands, events as realEvents } from "./bindings";
import { installMock } from "./mock";

export const { commands, events } =
  import.meta.env.MODE === "mock"
    ? installMock()
    : { commands: realCommands, events: realEvents };
export type {
  FailReason,
  Health,
  PingReply,
  PingRequest,
  QuitStatus,
  StorageError,
  StorageStatus,
} from "./bindings";
