import { commands as realCommands, events as realEvents } from "./bindings";
import { installMock } from "./mock";
import { commandsAccepted, trackCommand } from "./pending-commands";
import { withRecordCommands } from "./record-commands";

export { MAX_EXCLUDED_PAGES, MAX_SEARCH_LIMIT } from "./bindings";

const transport =
  import.meta.env.MODE === "mock"
    ? installMock()
    : { commands: realCommands, events: realEvents };
const boundary = {
  ...transport,
  commands: withRecordCommands(transport.commands),
};
export const events = boundary.events;
const quitTrackedCommands = ["settingsUpdate"] as const;
export const commands = { ...boundary.commands };
for (const name of quitTrackedCommands) {
  commands[name] = (
    ...args: Parameters<(typeof boundary.commands)[typeof name]>
  ) =>
    commandsAccepted()
      ? trackCommand(boundary.commands[name](...args))
      : Promise.resolve({ status: "error", error: "Quitting" });
}

export type {
  FailedRecordOp,
  OperationId,
  PageId,
  RecordCommandError,
  RecordCommandStore,
  RecordOp,
  RetainedRecordOp,
} from "../stores/record-commands";
export {
  failing,
  pendingCreate,
  useRecordCommandStore,
} from "../stores/record-commands";
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
  MenuCommand,
  OpenPagePatch,
  PageError,
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
export type { TransportFailure } from "./command-retry";
export type { RecordCommandResult } from "./record-commands";
