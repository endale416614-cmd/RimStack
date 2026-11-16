//! Non-UTF-8 paths are kept as `bytes:` and hex, since `to_string_lossy` would name another file.

use std::path::{Path, PathBuf};

/// Absolute paths never start with it, so it cannot be mistaken for one.
const BYTES: &str = "bytes:";

/// The extension is compared in any case; `ext` has no dot.
pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(ext))
}

pub fn to_text(path: &Path) -> String {
    match path.to_str() {
        Some(text) => text.to_string(),
        None => raw(path),
    }
}

pub fn from_text(text: &str) -> PathBuf {
    text.strip_prefix(BYTES)
        .and_then(unhex)
        .unwrap_or_else(|| PathBuf::from(text))
}

#[cfg(unix)]
fn raw(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let hex: String = path
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{BYTES}{hex}")
}

#[cfg(not(unix))]
fn raw(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
fn unhex(hex: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect();
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes?)))
}

#[cfg(not(unix))]
fn unhex(_hex: &str) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_utf8_path_is_kept_as_itself() {
        let path = Path::new("/mods/Ann/Textures/wall é.png");
        assert_eq!(to_text(path), "/mods/Ann/Textures/wall é.png");
        assert_eq!(from_text(&to_text(path)), path);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_path_that_is_not_utf8_comes_back_whole() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/mods/wall\xff.png"));
        let text = to_text(path);
        assert!(text.starts_with(BYTES));
        assert_eq!(from_text(&text), path);
        assert_eq!(from_text("bytes:zz"), Path::new("bytes:zz"));
    }
}
