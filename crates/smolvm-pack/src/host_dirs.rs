//! Where smolvm keeps its host-side state: the `dirs` directories, except that
//! on Windows `SMOLVM_DATA_DIR` moves all of them under one root.
//!
//! On Linux `SMOLVM_DATA_DIR` works by relocating `HOME` and clearing the
//! `XDG_*` variables, which `dirs` reads. On Windows `dirs` asks the Known
//! Folder API instead, so that does nothing, and state always landed in
//! `%LOCALAPPDATA%`. That is a problem for an embedder: a process started from
//! an MSIX-packaged app inherits its AppData redirection, under which AF_UNIX
//! sockets cannot connect and no machine ever becomes ready; and an AF_UNIX
//! path is limited to 108 bytes, so the root must not sit deep. Every engine
//! crate resolves these directories through here.
//!
//! With `SMOLVM_DATA_DIR=<root>` on Windows: the cache directory is
//! `<root>\cache`, the (local and roaming) data directories are `<root>\data`,
//! and there is no runtime directory, so callers fall back to the cache. Unix
//! hosts are unchanged.

use std::path::PathBuf;

/// The state root a Windows host was given, if any.
#[cfg(windows)]
pub fn windows_state_root() -> Option<PathBuf> {
    std::env::var_os("SMOLVM_DATA_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(windows)]
fn under_root(sub: &str) -> Option<Option<PathBuf>> {
    windows_state_root().map(|root| Some(root.join(sub)))
}

#[cfg(not(windows))]
fn under_root(_sub: &str) -> Option<Option<PathBuf>> {
    None
}

/// `dirs::cache_dir`, or `<SMOLVM_DATA_DIR>\cache` on Windows.
pub fn cache_dir() -> Option<PathBuf> {
    under_root("cache").unwrap_or_else(dirs::cache_dir)
}

/// `dirs::data_local_dir`, or `<SMOLVM_DATA_DIR>\data` on Windows.
pub fn data_local_dir() -> Option<PathBuf> {
    under_root("data").unwrap_or_else(dirs::data_local_dir)
}

/// `dirs::data_dir`, or `<SMOLVM_DATA_DIR>\data` on Windows.
pub fn data_dir() -> Option<PathBuf> {
    under_root("data").unwrap_or_else(dirs::data_dir)
}

/// `dirs::runtime_dir`; none on Windows under `SMOLVM_DATA_DIR`.
pub fn runtime_dir() -> Option<PathBuf> {
    match under_root("run") {
        Some(_) => None,
        None => dirs::runtime_dir(),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn the_root_moves_every_directory() {
        // One test owns the variable: tests in this crate run in parallel.
        let root = std::env::temp_dir().join("smolvm-host-dirs-test");
        std::env::set_var("SMOLVM_DATA_DIR", &root);
        assert_eq!(cache_dir(), Some(root.join("cache")));
        assert_eq!(data_local_dir(), Some(root.join("data")));
        assert_eq!(data_dir(), Some(root.join("data")));
        assert_eq!(runtime_dir(), None);
        std::env::remove_var("SMOLVM_DATA_DIR");
        assert_eq!(data_local_dir(), dirs::data_local_dir());
    }
}
