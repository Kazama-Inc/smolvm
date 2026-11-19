//! Guest paths in the boot env the host hands the agent.
//!
//! Boot mounts travel as `SMOLVM_MOUNT_<i>=<tag>:<guest path>:<mode>` and disk
//! volumes as `:`-joined targets in [`crate::guest_env::DISK_VOLUMES`]. These
//! variables reach the guest on its kernel command line, which takes only
//! printable ASCII inside double quotes, and their fields are split on `:`. A
//! guest path such as `/work/项目`, `/work/a:b` or `/work/"x"` would break one or
//! the other (libkrun refuses the command line, or the agent misreads the
//! fields), so each guest path is percent-encoded: every byte outside a
//! conservative set, `%` included, becomes `%XX`. A path made only of that set
//! is unchanged, so ordinary paths read the same either way.

/// Bytes a guest path keeps as they are.
fn keep(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"/._-~+,@=".contains(&byte)
}

/// Percent-encode a guest path for the boot env.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for &byte in path.as_bytes() {
        if keep(byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Decode a guest path [`encode_path`] produced. `None` when an escape is
/// malformed or the bytes are not UTF-8.
pub fn decode_path(field: &str) -> Option<String> {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_paths_are_unchanged() {
        for path in [
            "/data",
            "/work/my-app_v1.2",
            "/home/user/.cache",
            "/a+b,c@d=e~f",
        ] {
            assert_eq!(encode_path(path), path);
            assert_eq!(decode_path(path).as_deref(), Some(path));
        }
    }

    #[test]
    fn any_path_survives_as_printable_ascii_without_colons_or_quotes() {
        for path in [
            "/work/项目",
            "/work/a b",
            "/work/a:b",
            "/work/\"q\"",
            "/work/back\\slash",
            "/work/100%",
            "/work/tab\there",
        ] {
            let encoded = encode_path(path);
            assert!(
                encoded
                    .bytes()
                    .all(|b| b.is_ascii_graphic() && !b":\"\\ ".contains(&b)),
                "{encoded}"
            );
            assert_eq!(decode_path(&encoded).as_deref(), Some(path));
        }
        assert_eq!(encode_path("/work/项目"), "/work/%E9%A1%B9%E7%9B%AE");
    }

    #[test]
    fn malformed_escapes_are_refused() {
        for field in ["/a%", "/a%4", "/a%zz", "/a%FF"] {
            assert_eq!(decode_path(field), None, "{field}");
        }
    }
}
