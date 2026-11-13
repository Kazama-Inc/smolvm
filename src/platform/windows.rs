//! Windows-specific platform implementations.
//!
//! Windows support compiles the host control plane; the VM-execution code
//! paths mirror the Linux executor (libkrun on Windows auto-mounts the guest's
//! virtiofs devices, so no mount-wrapper script is needed). Rosetta is a
//! macOS-only feature and is unavailable here.

use crate::error::{Error, Result};
use crate::platform::traits::{RosettaSupport, VmExecutor};
use std::ffi::CString;
use std::path::Path;

/// Windows VM executor implementation.
///
/// Like the Linux executor, this executes the user command directly without a
/// virtiofs mount-wrapper script.
pub struct WindowsExecutor;

impl VmExecutor for WindowsExecutor {
    fn requires_mount_wrapper(&self) -> bool {
        false
    }

    fn build_exec_command(
        &self,
        command: &Option<Vec<String>>,
        _mounts: &[(String, String)],
        _rootfs: &Path,
        _rosetta: bool,
    ) -> Result<(CString, Vec<*const libc::c_char>, Vec<CString>)> {
        let default_cmd = vec!["/bin/sh".to_string()];
        let cmd = command.as_ref().unwrap_or(&default_cmd);

        if cmd.is_empty() {
            return Err(Error::vm_creation("command cannot be empty"));
        }

        let exec_path = CString::new(cmd[0].as_str())
            .map_err(|_| Error::vm_creation("invalid command path"))?;

        let cstrings: Vec<CString> = cmd
            .iter()
            .skip(1)
            .map(|s| CString::new(s.as_str()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::vm_creation("invalid command argument"))?;

        let mut argv: Vec<*const libc::c_char> = cstrings.iter().map(|s| s.as_ptr()).collect();
        argv.push(std::ptr::null());

        Ok((exec_path, argv, cstrings))
    }

    fn tool_search_paths(&self) -> &'static [&'static str] {
        &[]
    }

    fn dylib_extension(&self) -> &'static str {
        "dll"
    }

    fn library_search_paths(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Windows Rosetta support (stub - always unavailable).
pub struct WindowsRosetta;

impl RosettaSupport for WindowsRosetta {
    fn is_available(&self) -> bool {
        false
    }

    fn runtime_path(&self) -> Option<&'static str> {
        None
    }
}

/// Get the Rosetta support instance for Windows.
pub fn rosetta_support() -> WindowsRosetta {
    WindowsRosetta
}

/// Check that AF_UNIX sockets work where smolvm puts each machine's sockets,
/// at the same depth: a bind and a connect in a directory like a machine's,
/// under [`crate::agent::vm_cache_root`]. The result is kept for the life of
/// the process.
///
/// Without this a broken state directory shows only as "agent did not become
/// ready within 40 seconds". Two causes are known: a process started from an
/// MSIX-packaged app inherits its AppData redirection, and AF_UNIX connects
/// fail with WSAEINVAL there; and a state root so deep that a machine's socket
/// path passes the 108-byte AF_UNIX limit. `SMOLVM_DATA_DIR` moves the root.
pub fn check_state_dir() -> std::result::Result<(), String> {
    static RESULT: std::sync::OnceLock<std::result::Result<(), String>> =
        std::sync::OnceLock::new();
    RESULT.get_or_init(probe_state_dir).clone()
}

fn probe_state_dir() -> std::result::Result<(), String> {
    /// sun_path is 108 bytes, NUL included.
    const MAX_SOCKET_PATH: usize = 107;
    let root = crate::agent::vm_cache_root();
    // A machine's directory name is 16 hex digits; its longest socket name is
    // port-forward.sock.
    let dir = root.join(format!(".probe-{:09x}", std::process::id()));
    let sock = dir.join("port-forward.sock");
    let hint = "set SMOLVM_DATA_DIR to a short directory outside AppData, such as one under %TEMP%";
    if sock.as_os_str().len() > MAX_SOCKET_PATH {
        return Err(format!(
            "smolvm's state directory {} is too deep for AF_UNIX socket paths (108 bytes); {hint}",
            root.display()
        ));
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create {}: {e}; {hint}", dir.display()))?;
    let _ = std::fs::remove_file(&sock);
    let result = crate::platform::uds::UdsListener::bind(&sock).and_then(|listener| {
        crate::platform::uds::UdsStream::connect_timeout(&sock, std::time::Duration::from_secs(2))
            .map(|_| drop(listener))
    });
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_dir(&dir);
    result.map_err(|e| {
        format!(
            "AF_UNIX sockets do not work in smolvm's state directory {} ({e}). A process \
             started from an MSIX-packaged app has its AppData redirected, where they never \
             do; {hint}",
            root.display()
        )
    })
}
