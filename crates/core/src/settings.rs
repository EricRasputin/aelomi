use std::collections::{BTreeSet, HashSet};
use std::io;

use serde_json::{Map, Value, json};

use crate::workspace::records::PageId;

pub const RECENT_PAGES_LIMIT: usize = 50;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
    Unknown(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl From<ThemeChoice> for Theme {
    fn from(theme: ThemeChoice) -> Self {
        match theme {
            ThemeChoice::System => Self::System,
            ThemeChoice::Light => Self::Light,
            ThemeChoice::Dark => Self::Dark,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenPage {
    pub page_id: PageId,
    pub top_visible_block_id: Option<String>,
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub theme: Theme,
    pub open_page: Option<OpenPage>,
    pub recent_pages: Vec<PageId>,
    pub expanded_rows: BTreeSet<PageId>,
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub open_page: Option<Option<OpenPage>>,
    pub recent_pages: Option<Vec<PageId>>,
    pub expanded_rows: Option<BTreeSet<PageId>>,
}

impl Settings {
    pub fn apply(&mut self, patch: SettingsPatch) {
        if let Some(theme) = patch.theme {
            self.extra.remove("theme");
            self.theme = theme.into();
        }
        if let Some(mut open_page) = patch.open_page {
            self.extra.remove("openPage");
            if let (Some(current), Some(next)) = (&self.open_page, &mut open_page)
                && current.page_id == next.page_id
            {
                let mut extra = current.extra.clone();
                extra.extend(std::mem::take(&mut next.extra));
                next.extra = extra;
            }
            self.open_page = open_page;
        }
        if let Some(recent_pages) = patch.recent_pages {
            self.extra.remove("recentPages");
            self.recent_pages = recent_pages;
        }
        if let Some(expanded_rows) = patch.expanded_rows {
            self.extra.remove("expandedRows");
            self.expanded_rows = expanded_rows;
        }
        self.normalize();
    }

    fn normalize(&mut self) {
        let mut seen = HashSet::new();
        self.recent_pages.retain(|id| seen.insert(id.clone()));
        self.recent_pages.truncate(RECENT_PAGES_LIMIT);
    }

    pub fn from_json(bytes: &[u8]) -> io::Result<Self> {
        let value: Value = serde_json::from_slice(bytes)?;
        let mut extra = value.as_object().cloned().ok_or_else(invalid)?;
        let theme = take_field(&mut extra, "theme", |value| {
            let theme = value.as_str()?;
            Some(match theme {
                "System" => Theme::System,
                "Light" => Theme::Light,
                "Dark" => Theme::Dark,
                _ => Theme::Unknown(theme.into()),
            })
        })
        .unwrap_or_default();
        let open_page = take_field(&mut extra, "openPage", |value| {
            if value.is_null() {
                return Some(None);
            }
            let mut page = value.as_object()?.clone();
            let page_id = PageId::new(page.remove("pageId")?.as_str()?).ok()?;
            let top_visible_block_id = match page.remove("topVisibleBlockId") {
                None | Some(Value::Null) => None,
                Some(Value::String(id)) => Some(id),
                Some(_) => return None,
            };
            Some(Some(OpenPage {
                page_id,
                top_visible_block_id,
                extra: page,
            }))
        })
        .flatten();
        let recent_pages = take_field(&mut extra, "recentPages", page_ids).unwrap_or_default();
        let expanded_rows = take_field(&mut extra, "expandedRows", page_ids)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut settings = Self {
            theme,
            open_page,
            recent_pages,
            expanded_rows,
            extra,
        };
        settings.normalize();
        Ok(settings)
    }

    pub fn to_json(&self) -> Value {
        let mut object = self.extra.clone();
        let theme = match &self.theme {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::Unknown(value) => value,
        };
        object.entry("theme").or_insert_with(|| json!(theme));
        object.entry("openPage").or_insert_with(|| {
            self.open_page.as_ref().map_or(Value::Null, |page| {
                let mut object = page.extra.clone();
                object.insert("pageId".into(), json!(page.page_id.as_str()));
                object.insert("topVisibleBlockId".into(), json!(page.top_visible_block_id));
                Value::Object(object)
            })
        });
        object.entry("recentPages").or_insert_with(|| {
            json!(
                self.recent_pages
                    .iter()
                    .map(PageId::as_str)
                    .collect::<Vec<_>>()
            )
        });
        object.entry("expandedRows").or_insert_with(|| {
            json!(
                self.expanded_rows
                    .iter()
                    .map(PageId::as_str)
                    .collect::<Vec<_>>()
            )
        });
        Value::Object(object)
    }
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid settings.json")
}

fn take_field<T>(
    extra: &mut Map<String, Value>,
    key: &str,
    parse: impl FnOnce(&Value) -> Option<T>,
) -> Option<T> {
    let parsed = parse(extra.get(key)?)?;
    extra.remove(key);
    Some(parsed)
}

fn page_ids(value: &Value) -> Option<Vec<PageId>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(|id| PageId::new(id.as_str()?).ok())
            .collect(),
    )
}
