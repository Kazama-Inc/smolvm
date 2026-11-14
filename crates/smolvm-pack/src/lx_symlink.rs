//! Linux symlinks on a Windows host, stored the way WSL stores them: an NTFS
//! reparse point tagged `IO_REPARSE_TAG_LX_SYMLINK` whose data is the target's
//! bytes.
//!
//! A native NTFS symlink needs `SeCreateSymbolicLinkPrivilege` (an elevated
//! process, or Developer Mode), so a guest root filesystem unpacked by an
//! ordinary user came out without its links and the VM could not boot. An LX
//! symlink needs no privilege, keeps the target byte for byte (`/usr/bin/x`
//! stays a guest path instead of becoming `C:\usr\bin\x`), and is not
//! followed by Windows programs. libkrun's Windows virtio-fs server presents
//! it to the guest as a symlink.

use std::fs;
use std::io::{self, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

const IO_REPARSE_TAG_LX_SYMLINK: u32 = 0xA000_001D;
// winioctl.h
const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00A4;
const FSCTL_GET_REPARSE_POINT: u32 = 0x0009_00A8;
const MAXIMUM_REPARSE_DATA_BUFFER_SIZE: usize = 16 * 1024;

/// Create `link` as an LX symlink to `target`. `link` must not exist.
pub fn create(link: &Path, target: &[u8]) -> io::Result<()> {
    // Header (tag, data length, reserved), then a version word of 2 and the
    // target, as WSL writes it and libkrun reads it.
    let data_len = 4 + target.len();
    if 8 + data_len > MAXIMUM_REPARSE_DATA_BUFFER_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symlink target too long",
        ));
    }
    let mut buf = Vec::with_capacity(8 + data_len);
    buf.extend_from_slice(&IO_REPARSE_TAG_LX_SYMLINK.to_le_bytes());
    buf.extend_from_slice(&(data_len as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(target);

    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(link)?;
    let mut returned = 0u32;
    // SAFETY: `buf` is a complete reparse data buffer that outlives the call;
    // no output buffer is passed.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as _,
            FSCTL_SET_REPARSE_POINT,
            buf.as_ptr().cast(),
            buf.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        let err = io::Error::last_os_error();
        drop(file);
        let _ = fs::remove_file(link);
        return Err(err);
    }
    Ok(())
}

/// The target of the LX symlink at `link`.
pub fn read(link: &Path) -> io::Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(link)?;
    let mut buf = vec![0u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE];
    let mut returned = 0u32;
    // SAFETY: `buf` is a writable buffer of the stated size.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as _,
            FSCTL_GET_REPARSE_POINT,
            std::ptr::null(),
            0,
            buf.as_mut_ptr().cast(),
            buf.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let returned = returned as usize;
    let tag = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let data_len = u16::from_le_bytes(buf[4..6].try_into().unwrap()) as usize;
    if tag != IO_REPARSE_TAG_LX_SYMLINK || data_len < 4 || 8 + data_len > returned {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an LX symlink",
        ));
    }
    Ok(buf[12..8 + data_len].to_vec())
}

/// `path` as a relative path below an extraction root, or an error if it
/// names anything outside it (absolute, a drive or `..`). `None` for the root
/// itself.
fn below_root(path: &Path) -> io::Result<Option<PathBuf>> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::CurDir => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("archive entry '{}' leaves the destination", path.display()),
                ))
            }
        }
    }
    Ok((!out.as_os_str().is_empty()).then_some(out))
}

/// Unpack a Linux root filesystem tarball into `dest` (an empty directory)
/// without symlink privilege: symlinks become LX symlinks, everything else is
/// unpacked as usual. Unix modes and owners are not kept; libkrun's Windows
/// server presents rootfs files as root's and executable.
pub fn unpack_rootfs<R: Read>(reader: R, dest: &Path) -> io::Result<()> {
    let mut archive = tar::Archive::new(reader);
    archive.set_overwrite(true);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let Some(rel) = below_root(&path)? else {
            continue;
        };
        let full = dest.join(&rel);
        match entry.header().entry_type() {
            tar::EntryType::Symlink => {
                let target = entry.link_name_bytes().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("symlink '{}' has no target", path.display()),
                    )
                })?;
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                match fs::symlink_metadata(&full) {
                    Ok(meta) if meta.is_dir() => fs::remove_dir(&full)?,
                    Ok(_) => fs::remove_file(&full)?,
                    Err(_) => {}
                }
                create(&full, &target).map_err(|e| {
                    io::Error::new(e.kind(), format!("symlink '{}': {e}", path.display()))
                })?;
            }
            tar::EntryType::Link => {
                let target = entry.link_name()?.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("hard link '{}' has no target", path.display()),
                    )
                })?;
                let Some(target_rel) = below_root(&target)? else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("hard link '{}' to the root", path.display()),
                    ));
                };
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                let _ = fs::remove_file(&full);
                fs::hard_link(dest.join(target_rel), &full)?;
            }
            _ => {
                if !entry.unpack_in(dest)? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("archive entry '{}' leaves the destination", path.display()),
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        build(&mut b);
        b.into_inner().unwrap()
    }

    fn header(kind: tar::EntryType, size: u64) -> tar::Header {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(kind);
        h.set_size(size);
        h.set_mode(0o755);
        h
    }

    #[test]
    fn a_rootfs_unpacks_with_lx_symlinks() {
        let dest = tempfile::tempdir().unwrap();
        let data = archive(|b| {
            let mut h = header(tar::EntryType::Directory, 0);
            b.append_data(&mut h, "usr/local/bin/", &[][..]).unwrap();
            let mut h = header(tar::EntryType::Regular, 5);
            b.append_data(&mut h, "usr/local/bin/agent", &b"agent"[..])
                .unwrap();
            let mut h = header(tar::EntryType::Symlink, 0);
            b.append_link(&mut h, "sbin/init", "/usr/local/bin/agent")
                .unwrap();
            let mut h = header(tar::EntryType::Symlink, 0);
            b.append_link(&mut h, "bin/sh", "busybox").unwrap();
            let mut h = header(tar::EntryType::Link, 0);
            b.append_link(&mut h, "usr/local/bin/agent2", "usr/local/bin/agent")
                .unwrap();
        });
        unpack_rootfs(&data[..], dest.path()).unwrap();

        let init = dest.path().join("sbin/init");
        assert!(fs::symlink_metadata(&init).is_ok(), "sbin/init missing");
        assert_eq!(read(&init).unwrap(), b"/usr/local/bin/agent");
        assert_eq!(read(&dest.path().join("bin/sh")).unwrap(), b"busybox");
        assert_eq!(
            fs::read(dest.path().join("usr/local/bin/agent2")).unwrap(),
            b"agent"
        );
        // Windows does not follow it to C:\usr\local\bin\agent.
        assert!(fs::read(&init).is_err());
    }

    #[test]
    fn entries_outside_the_destination_are_refused() {
        for (kind, name, link) in [
            (tar::EntryType::Link, "x", "../outside"),
            (tar::EntryType::Link, "x", "/etc/passwd"),
        ] {
            let dest = tempfile::tempdir().unwrap();
            let data = archive(|b| {
                let mut h = header(kind, 0);
                b.append_link(&mut h, name, link).unwrap();
            });
            assert!(
                unpack_rootfs(&data[..], dest.path()).is_err(),
                "{name} -> {link}"
            );
        }
        assert!(below_root(Path::new("../x")).is_err());
        assert!(below_root(Path::new("/x")).is_err());
        assert!(below_root(Path::new("C:x")).is_err());
        assert_eq!(below_root(Path::new("./")).unwrap(), None);
    }
}
