import { commands as realCommands, events as realEvents } from "./bindings";
import { installMock } from "./mock";
import { commandsAccepted, trackCommand } from "./pending-commands";

export { MAX_EXCLUDED_PAGES, MAX_SEARCH_LIMIT } from "./bindings";

const boundary =
  import.meta.env.MODE === "mock"
    ? installMock()
    : { commands: realCommands, events: realEvents };
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
