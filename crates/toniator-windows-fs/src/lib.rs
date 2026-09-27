//! Safe, handle-bound native Windows filesystem infrastructure.
//!
//! This leaf crate owns the Windows ABI boundary. Callers retain authority over
//! filenames, publication decisions, recovery, and which entries may be discarded.
//! Non-Windows builds contain no platform implementation or product fallback.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    Directory, Entry, Identity, PublishMode, available_space, open_regular_file, random_suffix,
};
