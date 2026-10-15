use std::io;

use super::fs::{Fs, create_durable};
use super::layout::{Layout, RootFile};
use crate::settings::Settings;

pub(crate) fn load(fs: &dyn Fs, layout: &Layout) -> Settings {
    let result = fs.read(&layout.file(RootFile::Settings));
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

pub(crate) fn write(fs: &dyn Fs, layout: &Layout, settings: &Settings) -> io::Result<()> {
    let temp = layout.temp_file(RootFile::Settings);
    match fs.remove(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let bytes = serde_json::to_vec_pretty(&settings.to_json())?;
    create_durable(fs, &temp, &bytes)?;
    fs.replace(layout.root(), RootFile::Settings)?;
    fs.fsync(layout.root())?;
    fs.full_sync(layout.root())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
