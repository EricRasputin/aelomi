import { commands as realCommands, events as realEvents } from "./bindings";
import { installMock } from "./mock";

export { MAX_EXCLUDED_PAGES, MAX_SEARCH_LIMIT } from "./bindings";

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
  OpenPagePatch,
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
  Settings,
  SettingsError,
  SettingsOpenPage,
  SettingsPatch,
  StorageError,
  StorageStatus,
  TextPart,
  Theme,
  TrashedPage,
  TreePage,
} from "./bindings";
