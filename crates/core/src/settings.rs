use std::collections::{BTreeSet, HashSet};
use std::io;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::storage::fs::{Fs, create_durable};
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
    pub theme: Option<Theme>,
    pub open_page: Option<Option<OpenPage>>,
    pub recent_pages: Option<Vec<PageId>>,
    pub expanded_rows: Option<BTreeSet<PageId>>,
}

impl Settings {
    pub fn apply(&mut self, patch: SettingsPatch) {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(open_page) = patch.open_page {
            self.open_page = open_page;
        }
        if let Some(recent_pages) = patch.recent_pages {
            self.recent_pages = recent_pages;
        }
        if let Some(expanded_rows) = patch.expanded_rows {
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
        let theme = match extra.remove("theme") {
            None => Theme::System,
            Some(Value::String(theme)) => match theme.as_str() {
                "System" => Theme::System,
                "Light" => Theme::Light,
                "Dark" => Theme::Dark,
                _ => Theme::Unknown(theme),
            },
            Some(_) => return Err(invalid()),
        };
        let open_page = match extra.remove("openPage") {
            None | Some(Value::Null) => None,
            Some(Value::Object(mut page)) => {
                let page_id = page.remove("pageId").ok_or_else(invalid)?;
                let page_id = page_id.as_str().ok_or_else(invalid)?;
                let page_id = PageId::new(page_id).map_err(|_| invalid())?;
                let top_visible_block_id = match page.remove("topVisibleBlockId") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(id)) => Some(id),
                    Some(_) => return Err(invalid()),
                };
                Some(OpenPage {
                    page_id,
                    top_visible_block_id,
                    extra: page,
                })
            }
            Some(_) => return Err(invalid()),
        };
        let mut settings = Self {
            theme,
            open_page,
            recent_pages: page_ids(extra.remove("recentPages"))?,
            expanded_rows: page_ids(extra.remove("expandedRows"))?
                .into_iter()
                .collect(),
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
        object.insert("theme".into(), json!(theme));
        object.insert(
            "openPage".into(),
            self.open_page.as_ref().map_or(Value::Null, |page| {
                let mut object = page.extra.clone();
                object.insert("pageId".into(), json!(page.page_id.as_str()));
                object.insert("topVisibleBlockId".into(), json!(page.top_visible_block_id));
                Value::Object(object)
            }),
        );
        object.insert(
            "recentPages".into(),
            json!(
                self.recent_pages
                    .iter()
                    .map(PageId::as_str)
                    .collect::<Vec<_>>()
            ),
        );
        object.insert(
            "expandedRows".into(),
            json!(
                self.expanded_rows
                    .iter()
                    .map(PageId::as_str)
                    .collect::<Vec<_>>()
            ),
        );
        Value::Object(object)
    }
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid settings.json")
}

fn page_ids(value: Option<Value>) -> io::Result<Vec<PageId>> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::Array(ids)) => ids
            .into_iter()
            .map(|id| PageId::new(id.as_str().ok_or_else(invalid)?).map_err(|_| invalid()))
            .collect(),
        Some(_) => Err(invalid()),
    }
}

pub(crate) fn load(fs: &dyn Fs, root: &Path) -> Settings {
    let result = fs.read(&root.join("settings.json"));
    if matches!(&result, Err(error) if error.kind() == io::ErrorKind::NotFound) {
        return Settings::default();
    }
    match result.and_then(|bytes| Settings::from_json(&bytes)) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("settings load failed, using defaults: {error}");
            Settings::default()
        }
    }
}

pub(crate) fn write(fs: &dyn Fs, root: &Path, settings: &Settings) -> io::Result<()> {
    let temp = root.join("settings.json.tmp");
    match fs.remove(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let bytes = serde_json::to_vec_pretty(&settings.to_json())?;
    create_durable(fs, &temp, &bytes)?;
    fs.replace_settings(root)?;
    fs.fsync(root)?;
    fs.full_sync(root)
}

#[cfg(test)]
mod tests;
