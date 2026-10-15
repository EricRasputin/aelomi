use std::collections::BTreeSet;
use std::sync::{Arc, Condvar, Mutex};

use aelomi_core::settings as core;
use aelomi_core::workspace::{Workspace, records::PageId};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Theme {
    System,
    Light,
    Dark,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOpenPage {
    pub page_id: String,
    pub top_visible_block_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub open_page: Option<SettingsOpenPage>,
    pub recent_pages: Vec<String>,
    pub expanded_rows: Vec<String>,
}

#[derive(Clone, Deserialize, Type)]
pub enum OpenPagePatch {
    Set(SettingsOpenPage),
    Clear,
}

#[derive(Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    #[specta(optional)]
    pub theme: Option<Theme>,
    #[specta(optional)]
    pub open_page: Option<OpenPagePatch>,
    #[specta(optional)]
    pub recent_pages: Option<Vec<String>>,
    #[specta(optional)]
    pub expanded_rows: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Type)]
pub enum SettingsError {
    Unavailable,
    InvalidPageId,
    Io(String),
    Internal,
}

pub struct SettingsState(pub Option<Arc<SettingsOwner>>);

pub struct SettingsOwner {
    workspace: Arc<Workspace>,
    current: Mutex<Option<core::Settings>>,
    loaded: Condvar,
}

impl SettingsOwner {
    pub fn new(workspace: Arc<Workspace>) -> Arc<Self> {
        Arc::new(Self {
            workspace,
            current: Mutex::new(None),
            loaded: Condvar::new(),
        })
    }

    pub fn initialize(&self) {
        let settings = self.workspace.load_settings();
        *self.current.lock().unwrap() = Some(settings);
        self.loaded.notify_all();
    }

    pub fn get(&self) -> Settings {
        let current = self.current.lock().unwrap();
        let current = self
            .loaded
            .wait_while(current, |current| current.is_none())
            .unwrap();
        Settings::from(current.as_ref().unwrap())
    }

    pub fn update(&self, patch: SettingsPatch) -> Result<Settings, SettingsError> {
        let patch = core::SettingsPatch::try_from(patch)?;
        let current = self.current.lock().unwrap();
        let mut current = self
            .loaded
            .wait_while(current, |current| current.is_none())
            .unwrap();
        let settings = current.as_mut().unwrap();
        settings.apply(patch);
        self.workspace
            .write_settings(settings)
            .map_err(|error| SettingsError::Io(error.to_string()))?;
        Ok(Settings::from(&*settings))
    }
}

impl From<&core::Settings> for Settings {
    fn from(settings: &core::Settings) -> Self {
        Self {
            theme: match &settings.theme {
                core::Theme::System => Theme::System,
                core::Theme::Light => Theme::Light,
                core::Theme::Dark => Theme::Dark,
                core::Theme::Unknown(value) => Theme::Unknown(value.clone()),
            },
            open_page: settings.open_page.as_ref().map(|page| SettingsOpenPage {
                page_id: page.page_id.to_string(),
                top_visible_block_id: page.top_visible_block_id.clone(),
            }),
            recent_pages: settings
                .recent_pages
                .iter()
                .map(ToString::to_string)
                .collect(),
            expanded_rows: settings
                .expanded_rows
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

impl TryFrom<SettingsPatch> for core::SettingsPatch {
    type Error = SettingsError;

    fn try_from(patch: SettingsPatch) -> Result<Self, Self::Error> {
        let ids = |ids: Vec<String>| {
            ids.into_iter()
                .map(|id| PageId::new(id).map_err(|_| SettingsError::InvalidPageId))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            theme: patch.theme.map(|theme| match theme {
                Theme::System => core::Theme::System,
                Theme::Light => core::Theme::Light,
                Theme::Dark => core::Theme::Dark,
                Theme::Unknown(value) => core::Theme::Unknown(value),
            }),
            open_page: patch
                .open_page
                .map(|page| match page {
                    OpenPagePatch::Clear => Ok(None),
                    OpenPagePatch::Set(page) => Ok(Some(core::OpenPage {
                        page_id: PageId::new(page.page_id)
                            .map_err(|_| SettingsError::InvalidPageId)?,
                        top_visible_block_id: page.top_visible_block_id,
                        extra: Default::default(),
                    })),
                })
                .transpose()?,
            recent_pages: patch.recent_pages.map(ids).transpose()?,
            expanded_rows: patch
                .expanded_rows
                .map(ids)
                .transpose()?
                .map(BTreeSet::from_iter),
        })
    }
}

#[cfg(test)]
mod tests;
