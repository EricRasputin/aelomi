import { isPageId } from "../editor/body-reader";
import {
  RECENT_PAGES_LIMIT,
  type Settings,
  type SettingsError,
  type SettingsPatch,
} from "./bindings";

export function defaultSettings(): Settings {
  return {
    theme: "System",
    openPage: null,
    recentPages: [],
    expandedRows: [],
  };
}

export function patchSettings(
  settings: Settings,
  patch: SettingsPatch,
):
  | { status: "ok"; data: Settings }
  | { status: "error"; error: SettingsError } {
  const openPage = patch.openPage;
  const pageIds = [
    ...(patch.recentPages ?? []),
    ...(patch.expandedRows ?? []),
    ...(openPage && typeof openPage === "object" ? [openPage.Set.pageId] : []),
  ];
  if (pageIds.some((id) => !isPageId(id)))
    return { status: "error", error: "InvalidPageId" };
  return {
    status: "ok",
    data: {
      theme: structuredClone(patch.theme ?? settings.theme),
      openPage:
        openPage === "Clear"
          ? null
          : structuredClone(openPage?.Set ?? settings.openPage),
      recentPages: [
        ...new Set(patch.recentPages ?? settings.recentPages),
      ].slice(0, RECENT_PAGES_LIMIT),
      expandedRows: [
        ...new Set(patch.expandedRows ?? settings.expandedRows),
      ].sort(),
    },
  };
}
