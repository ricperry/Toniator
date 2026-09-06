//! Personal export destinations remain outside documents and reusable Presets.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);

/// Stores optional personal folders; absence prompts the artist at export time.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportDefaults {
    pub destination: Option<PathBuf>,
    pub temporary_directory: Option<PathBuf>,
}

impl ExportDefaults {
    /// Loads bounded personal metadata, treating a missing settings file as unset defaults.
    ///
    /// # Errors
    /// Rejects oversized, malformed or relative paths and reports filesystem errors.
    pub fn load(path: &Path) -> io::Result<Self> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(16_385).read_to_end(&mut bytes)?;
        if bytes.len() > 16_384 {
            return Err(io::Error::other("Export settings are too large."));
        }
        let value: Self = serde_json::from_slice(&bytes)?;
        value.validate()?;
        Ok(value)
    }

    /// Atomically saves personal folders without opening artwork or creating export destinations.
    ///
    /// # Errors
    /// Reports invalid paths, serialization and filesystem failures; preserves the old file on failure.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        if bytes.len() > 16_384 {
            return Err(io::Error::other("Export settings are too large."));
        }
        let parent = path
            .parent()
            .filter(|path| path.is_absolute())
            .ok_or_else(|| io::Error::other("Export settings need an absolute parent folder."))?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(
            ".export-defaults-{}-{}",
            std::process::id(),
            NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Validates portable local absolute-path intent while allowing expired grants to be displayed.
    ///
    /// # Errors
    /// Rejects relative destinations instead of interpreting them against a future working directory.
    fn validate(&self) -> io::Result<()> {
        if [&self.destination, &self.temporary_directory]
            .into_iter()
            .flatten()
            .any(|path| !path.is_absolute())
        {
            return Err(io::Error::other("Export folders must use absolute paths."));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Checks that saved defaults survive reopening and invalid edits leave the previous file intact.
    ///
    /// # Panics
    /// Panics if personal persistence loses paths or writes an invalid replacement.
    #[test]
    fn personal_defaults_roundtrip_and_reject_relative_replacement() {
        let root = std::env::temp_dir().join(format!(
            "toniator-export-defaults-{}-{}",
            std::process::id(),
            NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("defaults.json");
        assert_eq!(
            ExportDefaults::load(&path).unwrap(),
            ExportDefaults::default()
        );
        let defaults = ExportDefaults {
            destination: Some(root.join("output")),
            temporary_directory: Some(root.join("temporary")),
        };
        defaults.save(&path).unwrap();
        assert_eq!(ExportDefaults::load(&path).unwrap(), defaults);
        assert!(
            ExportDefaults {
                destination: Some("relative".into()),
                temporary_directory: None
            }
            .save(&path)
            .is_err()
        );
        assert_eq!(ExportDefaults::load(&path).unwrap(), defaults);
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
