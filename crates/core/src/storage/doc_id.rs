use std::fmt;

/// A document's ID. Always 1 to 64 characters from `[0-9a-z-]`, so it is safe as a folder name.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidDocId(pub String);

impl DocId {
    pub const MAX_LEN: usize = 64;

    pub fn new(id: impl Into<String>) -> Result<Self, InvalidDocId> {
        let id = id.into();
        let valid = (1..=Self::MAX_LEN).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-');
        if valid {
            Ok(Self(id))
        } else {
            Err(InvalidDocId(id))
        }
    }

    pub fn workspace() -> Self {
        Self("workspace".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocId({})", self.0)
    }
}

impl fmt::Display for InvalidDocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid doc ID {:?}: use 1 to 64 of [0-9a-z-]", self.0)
    }
}

impl std::error::Error for InvalidDocId {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_safe_folder_names() {
        let accepted = ["workspace", "a", "0", "page-1f3a", &"z".repeat(64)];
        for id in accepted {
            assert_eq!(
                DocId::new(id).map(|d| d.as_str().to_owned()),
                Ok(id.to_owned())
            );
        }
        let rejected = [
            "",
            "A",
            "a b",
            ".",
            "..",
            "a/b",
            "a_b",
            "é",
            &"z".repeat(65),
        ];
        for id in rejected {
            assert_eq!(DocId::new(id), Err(InvalidDocId(id.to_owned())), "{id:?}");
        }
        assert_eq!(DocId::workspace().as_str(), "workspace");
    }
}
