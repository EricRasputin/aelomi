use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum DataPathError {
    /// The data path resolves to `resolved`, inside the cloud-synced folder `root`.
    CloudFolder {
        resolved: PathBuf,
        root: PathBuf,
    },
    Io(io::Error),
}

/// Resolves every symlink in `path` and in each forbidden root, then refuses a path inside any
/// root. Returns the resolved path. The path need not exist yet. Comparing paths the
/// filesystem resolved, never raw strings, also handles APFS's case-insensitive names.
pub fn check_data_path(path: &Path, forbidden_roots: &[PathBuf]) -> Result<PathBuf, DataPathError> {
    let resolved = canonicalize_allowing_missing(path)?;
    for root in forbidden_roots {
        let root = canonicalize_allowing_missing(root)?;
        if resolved.starts_with(&root) {
            return Err(DataPathError::CloudFolder { resolved, root });
        }
    }
    Ok(resolved)
}

fn canonicalize_allowing_missing(path: &Path) -> io::Result<PathBuf> {
    let mut path = std::path::absolute(path)?;
    for _ in 0..40 {
        let mut existing = path.as_path();
        let mut missing = Vec::new();
        'walk: loop {
            match existing.canonicalize() {
                Ok(mut resolved) => {
                    let mut parts = missing.iter().rev();
                    while let Some(part) = parts.next() {
                        match part {
                            // Going up out of a missing folder lands in one that exists, so
                            // what follows may be a symlink and is resolved again.
                            Component::ParentDir => {
                                resolved.pop();
                                resolved.extend(parts);
                                path = resolved;
                                break 'walk;
                            }
                            Component::Normal(name) => resolved.push(name),
                            _ => {}
                        }
                    }
                    return Ok(resolved);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if let Ok(target) = std::fs::read_link(existing) {
                        let parent = existing.parent().unwrap_or(Path::new("/"));
                        let mut followed = parent.join(target);
                        followed.extend(missing.iter().rev());
                        path = followed;
                        break;
                    }
                    missing.push(existing.components().next_back().ok_or(e)?);
                    existing = existing
                        .parent()
                        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

impl From<io::Error> for DataPathError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for DataPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CloudFolder { resolved, root } => write!(
                f,
                "the data folder resolves to {}, inside the cloud-synced folder {}",
                resolved.display(),
                root.display()
            ),
            Self::Io(e) => write!(f, "could not resolve the data folder: {e}"),
        }
    }
}

impl std::error::Error for DataPathError {}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    struct Home {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Home {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().canonicalize().unwrap();
            for folder in [
                "Library/Application Support",
                "Library/Mobile Documents",
                "Library/CloudStorage/Dropbox",
            ] {
                std::fs::create_dir_all(root.join(folder)).unwrap();
            }
            Self { _dir: dir, root }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn cloud_roots(&self) -> Vec<PathBuf> {
            vec![
                self.path("Library/Mobile Documents"),
                self.path("Library/CloudStorage"),
            ]
        }

        fn check(&self, relative: &str) -> Result<PathBuf, (PathBuf, PathBuf)> {
            check_data_path(&self.path(relative), &self.cloud_roots()).map_err(|e| match e {
                DataPathError::CloudFolder { resolved, root } => (resolved, root),
                DataPathError::Io(e) => panic!("{e}"),
            })
        }
    }

    #[test]
    fn accepts_a_normal_path_and_one_that_does_not_exist_yet() {
        let home = Home::new();
        std::fs::create_dir(home.path("Library/Application Support/Aelomi")).unwrap();
        assert_eq!(
            home.check("Library/Application Support/Aelomi"),
            Ok(home.path("Library/Application Support/Aelomi"))
        );
        assert_eq!(
            home.check("Library/Application Support/Aelomi Dev/data"),
            Ok(home.path("Library/Application Support/Aelomi Dev/data"))
        );
        assert_eq!(
            home.check("Library/Application Support/missing/../Aelomi"),
            Ok(home.path("Library/Application Support/Aelomi"))
        );
    }

    #[test]
    fn rejects_a_symlinked_data_folder_that_resolves_into_icloud() {
        let home = Home::new();
        std::fs::create_dir(home.path("Library/Mobile Documents/Aelomi")).unwrap();
        symlink(
            home.path("Library/Mobile Documents/Aelomi"),
            home.path("Library/Application Support/Aelomi"),
        )
        .unwrap();
        assert_eq!(
            home.check("Library/Application Support/Aelomi"),
            Err((
                home.path("Library/Mobile Documents/Aelomi"),
                home.path("Library/Mobile Documents")
            ))
        );
    }

    #[test]
    fn rejects_a_symlinked_parent_that_resolves_into_cloud_storage() {
        let home = Home::new();
        symlink(
            home.path("Library/CloudStorage/Dropbox"),
            home.path("Library/Support"),
        )
        .unwrap();
        assert_eq!(
            home.check("Library/Support/Aelomi"),
            Err((
                home.path("Library/CloudStorage/Dropbox/Aelomi"),
                home.path("Library/CloudStorage")
            ))
        );
    }

    #[test]
    fn rejects_a_path_inside_a_forbidden_root_that_is_itself_a_symlink() {
        let home = Home::new();
        std::fs::create_dir(home.path("icloud")).unwrap();
        std::fs::remove_dir(home.path("Library/Mobile Documents")).unwrap();
        symlink(home.path("icloud"), home.path("Library/Mobile Documents")).unwrap();
        assert_eq!(
            home.check("icloud/Aelomi"),
            Err((home.path("icloud/Aelomi"), home.path("icloud")))
        );
    }

    #[test]
    fn rejects_a_dangling_symlink_into_a_cloud_folder() {
        let home = Home::new();
        symlink(
            home.path("Library/CloudStorage/Dropbox/Later"),
            home.path("Library/Application Support/Aelomi"),
        )
        .unwrap();
        assert_eq!(
            home.check("Library/Application Support/Aelomi/data"),
            Err((
                home.path("Library/CloudStorage/Dropbox/Later/data"),
                home.path("Library/CloudStorage")
            ))
        );
    }

    #[test]
    fn resolves_a_symlink_reached_through_dot_dot_out_of_a_missing_folder() {
        let home = Home::new();
        std::fs::create_dir(home.path("local")).unwrap();
        symlink(
            home.path("Library/CloudStorage/Dropbox"),
            home.path("local/link"),
        )
        .unwrap();
        for path in [
            "local/missing/../link/data",
            "local/missing/deeper/../../link/data",
            "local/./missing/./../link/./data",
        ] {
            assert_eq!(
                home.check(path),
                Err((
                    home.path("Library/CloudStorage/Dropbox/data"),
                    home.path("Library/CloudStorage")
                )),
                "{path}"
            );
        }
        assert_eq!(
            home.check("local/missing/../other/data"),
            Ok(home.path("local/other/data"))
        );
    }

    #[test]
    fn compares_whole_components_not_string_prefixes() {
        let home = Home::new();
        assert_eq!(
            home.check("Library/CloudStorageNotReally/Aelomi"),
            Ok(home.path("Library/CloudStorageNotReally/Aelomi"))
        );
    }
}
