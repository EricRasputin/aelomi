use std::collections::BTreeSet;

use aelomi_core::settings as core;
use aelomi_core::workspace::records::PageId;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Theme {
    System,
    Light,
    Dark,
    Unknown(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Type)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SettingsOpenPage {
    pub page_id: String,
    pub top_visible_block_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
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
pub struct SettingsPatch {
    #[specta(optional)]
    pub theme: Option<ThemeChoice>,
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
    Quitting,
    InvalidPageId,
    Io(String),
    Write(crate::storage::FailReason),
    Internal,
}

impl From<crate::storage::StorageError> for SettingsError {
    fn from(_: crate::storage::StorageError) -> Self {
        Self::Unavailable
    }
}

impl From<std::io::Error> for SettingsError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<aelomi_core::workspace::SettingsError> for SettingsError {
    fn from(error: aelomi_core::workspace::SettingsError) -> Self {
        match error {
            aelomi_core::workspace::SettingsError::Read(error) => error.into(),
            aelomi_core::workspace::SettingsError::Write(reason) => Self::Write((&reason).into()),
        }
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
                ThemeChoice::System => core::ThemeChoice::System,
                ThemeChoice::Light => core::ThemeChoice::Light,
                ThemeChoice::Dark => core::ThemeChoice::Dark,
            }),
            open_page: patch
                .open_page
                .map(|page| match page {
                    OpenPagePatch::Clear => Ok::<_, SettingsError>(None),
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
