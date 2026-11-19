//! Centralized path constants and helpers for the smolvm agent.
//!
//! All filesystem paths used by the agent are defined here for consistency
//! and easy modification.

use std::path::PathBuf;

// =============================================================================
// Binary Paths
// =============================================================================

/// Path to crun OCI runtime binary.
pub const CRUN_PATH: &str = "/usr/bin/crun";

/// crun state root directory.
/// Stored on the persistent storage disk instead of `/run/crun` because
/// `/run` may not be writable under the overlayfs rootfs.
pub const CRUN_ROOT_DIR: &str = "/storage/containers/crun";

/// crun cgroup manager setting.
/// Set to "disabled" because libkrun mounts cgroup2 as read-only.
/// Without this, crun create/start hang trying to create container cgroups.
pub const CRUN_CGROUP_MANAGER: &str = "disabled";

// =============================================================================
// Mount Paths
// =============================================================================

/// Root directory for virtiofs mounts from the host.
// Keep internal staging outside `/mnt`: users commonly mount a host directory
// at `/mnt`, and bind-mounting it there must not hide the virtio-fs source that
// backs the bind itself.
pub const VIRTIOFS_MOUNT_ROOT: &str = "/run/smolvm/virtiofs";

/// Root directory for the clones of host directories that map the directory's
/// owner to the workload user (see `mount_owner`). Internal staging like the
/// virtiofs root above, and outside `/mnt` for the same reason.
pub const IDMAPPED_MOUNT_ROOT: &str = "/run/smolvm/idmapped";

/// Guest path at which the shared workspace is exposed inside containers.
/// Used both when mounting /storage/workspace as the fallback workspace and
/// when checking whether a user-provided volume already claims this path.
pub const WORKSPACE_GUEST_PATH: &str = "/workspace";

// =============================================================================
// Storage Paths
// =============================================================================

/// Root directory for all persistent storage.
pub const STORAGE_ROOT: &str = "/storage";

/// Directory for overlay filesystems.
pub const OVERLAYS_DIR: &str = "/storage/overlays";

// =============================================================================
// Container Runtime Paths
// =============================================================================

/// Directory for per-container runtime state (pidfile, etc).
pub const CONTAINERS_RUN_DIR: &str = "/storage/containers/run";

/// Directory for container logs.
pub const CONTAINERS_LOGS_DIR: &str = "/storage/containers/logs";

/// Directory for container exit code files.
pub const CONTAINERS_EXIT_DIR: &str = "/storage/containers/exit";

// =============================================================================
// Path Helper Functions
// =============================================================================

/// Get the overlay directory for a workload.
pub fn overlay_dir(workload_id: &str) -> PathBuf {
    PathBuf::from(OVERLAYS_DIR).join(workload_id)
}

/// Path to the file recording the main container ID for a persistent overlay.
/// Written when a detached container is started; read on every subsequent exec.
pub fn main_container_id_path(workload_id: &str) -> PathBuf {
    overlay_dir(workload_id).join("main_container_id")
}

// =============================================================================
// Filesystem Helpers
// =============================================================================

/// Undo the octal escapes /proc/mounts and mountinfo write for space, tab,
/// newline and backslash in a path field (`/a\040b` is `/a b`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn unescape_mount_field(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)));
        match octal.and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 8).ok())
        {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `path` is a mount destination in a /proc/mounts listing.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn listed_as_mount_point(mounts: &str, path: &str) -> bool {
    mounts.lines().any(|line| {
        line.split_whitespace()
            .nth(1)
            .is_some_and(|field| unescape_mount_field(field) == path)
    })
}

/// Check if a path is a mountpoint by reading /proc/mounts.
///
/// Returns true if the path appears as a mount destination in /proc/mounts.
#[cfg(target_os = "linux")]
pub fn is_mount_point(path: &std::path::Path) -> bool {
    std::fs::read_to_string("/proc/mounts")
        .is_ok_and(|mounts| listed_as_mount_point(&mounts, &path.to_string_lossy()))
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn is_mount_point(_path: &std::path::Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_points_with_escaped_characters_are_found() {
        let mounts = "smolvm0 /run/smolvm/virtiofs/smolvm0 virtiofs rw 0 0\n\
                      smolvm0 /work/a\\040b virtiofs rw 0 0\n\
                      smolvm1 /work/tab\\011x\\134y virtiofs rw 0 0\n";
        assert!(listed_as_mount_point(mounts, "/work/a b"));
        assert!(listed_as_mount_point(mounts, "/work/tab\tx\\y"));
        assert!(!listed_as_mount_point(mounts, "/work/a\\040b"));
        assert!(!listed_as_mount_point(mounts, "/work/a"));
        // A literal backslash before digits is decoded once, not twice.
        assert_eq!(unescape_mount_field("/x\\134040"), "/x\\040");
        assert_eq!(unescape_mount_field("/x\\9"), "/x\\9");
    }

    #[test]
    fn test_overlay_paths() {
        let wl = "workload-123";
        assert_eq!(
            overlay_dir(wl),
            PathBuf::from("/storage/overlays/workload-123")
        );
    }

    #[test]
    fn test_main_container_id_path() {
        assert_eq!(
            main_container_id_path("persistent-myvm"),
            PathBuf::from("/storage/overlays/persistent-myvm/main_container_id")
        );
    }

    #[test]
    fn virtiofs_staging_is_not_below_the_user_mount_tree() {
        let staging = std::path::Path::new(VIRTIOFS_MOUNT_ROOT);
        assert!(staging.starts_with("/run/smolvm"));
        assert!(!staging.starts_with("/mnt"));
    }

    #[test]
    fn owner_mapped_staging_is_not_below_the_user_mount_tree_or_another_staging_root() {
        let staging = std::path::Path::new(IDMAPPED_MOUNT_ROOT);
        assert!(staging.starts_with("/run/smolvm"));
        assert!(!staging.starts_with("/mnt"));
        assert!(!staging.starts_with(VIRTIOFS_MOUNT_ROOT));
    }
}
