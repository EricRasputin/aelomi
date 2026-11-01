import { isPageId } from "../editor/body-reader";
import {
  type commands,
  type FailReason,
  RECENT_PAGES_LIMIT,
  type Settings,
  type SettingsError,
  type SettingsPatch,
  type StorageStatus,
} from "./bindings";

export function defaultSettings(): Settings {
  return {
    theme: "System",
    open_page: null,
    recent_pages: [],
    expanded_rows: [],
  };
}

export function patchSettings(
  settings: Settings,
  patch: SettingsPatch,
):
  | { status: "ok"; data: Settings }
  | { status: "error"; error: SettingsError } {
  const open_page = patch.open_page;
  const page_ids = [
    ...(patch.recent_pages ?? []),
    ...(patch.expanded_rows ?? []),
    ...(open_page && typeof open_page === "object"
      ? [open_page.Set.page_id]
      : []),
  ];
  if (page_ids.some((id) => !isPageId(id)))
    return { status: "error", error: "InvalidPageId" };
  return {
    status: "ok",
    data: seedSettings({
      theme: structuredClone(patch.theme ?? settings.theme),
      open_page:
        open_page === "Clear"
          ? null
          : structuredClone(open_page?.Set ?? settings.open_page),
      recent_pages: patch.recent_pages ?? settings.recent_pages,
      expanded_rows: patch.expanded_rows ?? settings.expanded_rows,
    }),
  };
}

export function seedSettings(seed: Partial<Settings>): Settings {
  const settings = { ...defaultSettings(), ...structuredClone(seed) };
  return {
    theme: settings.theme,
    open_page:
      settings.open_page && isPageId(settings.open_page.page_id)
        ? settings.open_page
        : null,
    recent_pages: [...new Set(settings.recent_pages.filter(isPageId))].slice(
      0,
      RECENT_PAGES_LIMIT,
    ),
    expanded_rows: [...new Set(settings.expanded_rows.filter(isPageId))].sort(),
  };
}

type SettingsCommands = Pick<typeof commands, "settingsGet" | "settingsUpdate">;
type SettingsCall = {
  [Name in keyof SettingsCommands]: {
    command: Name;
    args: Parameters<SettingsCommands[Name]>;
  };
}[keyof SettingsCommands];

export function createSettingsMock(deps: {
  ready: () => Promise<void>;
  status: () => StorageStatus;
  writeFailure: () => FailReason | null;
  changed: () => void;
  record: (call: SettingsCall) => void;
}) {
  let settings = defaultSettings();
  let pending = false;
  let transportFailures = 0;
  let failReads = false;
  let error: SettingsError | null = null;
  const commands = {
    settingsGet: async () => {
      deps.record({ command: "settingsGet", args: [] });
      await deps.ready();
      if (failReads)
        return {
          status: "error",
          error: { Io: "synthetic settings read failure" },
        };
      return deps.status() !== "Ready"
        ? { status: "error", error: "Unavailable" }
        : { status: "ok", data: structuredClone(settings) };
    },
    settingsUpdate: async (patch) => {
      deps.record({
        command: "settingsUpdate",
        args: [structuredClone(patch)],
      });
      await deps.ready();
      if (error !== null) return { status: "error", error };
      if (transportFailures > 0) {
        transportFailures -= 1;
        throw new Error("Settings transport unavailable");
      }
      if (deps.status() !== "Ready")
        return { status: "error", error: "Unavailable" };
      const result = patchSettings(settings, patch);
      if (result.status === "error") return result;
      settings = result.data;
      const failure = deps.writeFailure();
      pending = failure !== null;
      deps.changed();
      return failure === null
        ? structuredClone(result)
        : { status: "error", error: { Write: failure } };
    },
  } satisfies SettingsCommands;
  return {
    commands,
    get pending() {
      return pending;
    },
    seed(seed: Partial<Settings>) {
      settings = seedSettings(seed);
    },
    recover() {
      pending = false;
    },
    reset() {
      settings = defaultSettings();
      pending = false;
      transportFailures = 0;
      failReads = false;
      error = null;
    },
    control: {
      failReads(on: boolean) {
        failReads = on;
      },
      get: () => commands.settingsGet(),
      update: (patch: SettingsPatch) => commands.settingsUpdate(patch),
      refuseUpdates(value: SettingsError | null) {
        error = value;
      },
      failTransport(attempts: number) {
        transportFailures = attempts;
      },
    },
  };
}
