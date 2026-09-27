//! Private platform filesystem mechanics without domain, filename, or recovery policy.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{
    Directory, Entry, PublishMode, available_space, random_suffix, sync_directory,
};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use toniator_windows_fs::{
    Directory, Entry, PublishMode, available_space, open_regular_file, random_suffix,
};
#[cfg(windows)]
pub(crate) use windows::Staging;

/// Synchronizes an existing Windows directory through the full native metadata-flush boundary.
/// # Errors
/// Returns the real no-follow directory-open or full-flush failure; callers choose fatal versus warning.
#[cfg(windows)]
pub(crate) fn sync_directory(path: &std::path::Path) -> std::io::Result<()> {
    Directory::open(path)?.sync()
}
