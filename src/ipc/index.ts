import { commands as realCommands, events as realEvents } from "./bindings";
import { installMock } from "./mock";

export const { commands, events } =
  import.meta.env.MODE === "mock"
    ? installMock()
    : { commands: realCommands, events: realEvents };
export type {
  Backlink,
  BacklinkBlock,
  BacklinksReply,
  BlockHit,
  FailReason,
  Health,
  IndexChanged,
  IndexStatus,
  InvalidQuery,
  PageIssue,
  PageProblem,
  PageTree,
  PingReply,
  PingRequest,
  QueryError,
  QuitPhase,
  QuitStatus,
  SearchHit,
  SearchReply,
  SearchScope,
  StorageError,
  StorageStatus,
  TextPart,
  TrashedPage,
  TreePage,
} from "./bindings";
