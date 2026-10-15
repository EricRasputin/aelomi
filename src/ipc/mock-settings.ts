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
