use crate::base::error::Error;
use std::fs::Permissions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Synced to the disk before it returns: a slow wait.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    crate::base::waits::slow(&format!("{} written", path.display()));
    let failed = |e: std::io::Error| Error::file(path, e);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    let real = through_links(path);
    put(&partial_beside(&real), &real, contents.as_ref()).map_err(failed)
}

pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), Error> {
    let text = serde_json::to_vec(value)
        .map_err(|e| Error::format(crate::said!("{}: {}", path.display(), e)))?;
    write(path, text)
}

/// One JSON value per line.
pub fn write_lines<T: serde::Serialize>(
    path: &Path,
    lines: impl IntoIterator<Item = T>,
) -> Result<(), Error> {
    let mut text = String::new();
    for line in lines {
        let one = serde_json::to_string(&line)
            .map_err(|e| Error::format(crate::said!("{}: {}", path.display(), e)))?;
        text.push_str(&one);
        text.push('\n');
    }
    write(path, text)
}

/// Two writes must not share a partial name: each would write over the other's bytes.
pub(crate) fn partial_beside(real: &Path) -> PathBuf {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let mut partial = real.as_os_str().to_os_string();
    let nth = WRITES.fetch_add(1, Ordering::Relaxed);
    partial.push(format!(".{}-{nth}.part", std::process::id()));
    partial.into()
}

/// A link is followed: renaming onto the link itself would replace it.
pub(crate) fn through_links(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The sync before the rename is needed: without it a power cut can leave an empty file.
pub(crate) fn put(partial: &Path, path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let written = std::fs::File::create(partial).and_then(|mut file| {
        file.write_all(contents)?;
        Ok(file)
    });
    match written {
        Ok(file) => settle(file, partial, path),
        Err(problem) => {
            let _ = std::fs::remove_file(partial);
            Err(problem)
        }
    }
}

/// `file`, written in full at `partial`, takes the place of `path`.
pub(crate) fn settle(file: std::fs::File, partial: &Path, path: &Path) -> std::io::Result<()> {
    let before = std::fs::metadata(path).ok().map(|m| m.permissions());
    let written = file.sync_all().and_then(|()| {
        drop(file);
        replace(partial, path, before.as_ref())
    });
    if written.is_err() && partial.is_file() {
        let _ = std::fs::remove_file(partial);
    }
    written?;
    sync_folder(path);
    Ok(())
}

#[cfg(unix)]
fn replace(partial: &Path, path: &Path, before: Option<&Permissions>) -> std::io::Result<()> {
    if let Some(before) = before {
        std::fs::set_permissions(partial, before.clone())?;
    }
    std::fs::rename(partial, path)
}

#[cfg(not(unix))]
#[expect(
    clippy::permissions_set_readonly_false,
    reason = "on Windows this clears the read-only attribute only; nothing becomes writable by others"
)]
fn replace(partial: &Path, path: &Path, before: Option<&Permissions>) -> std::io::Result<()> {
    let Some(before) = before.filter(|before| before.readonly()) else {
        return std::fs::rename(partial, path);
    };
    let mut writable = before.clone();
    writable.set_readonly(false);
    std::fs::set_permissions(path, writable)?;
    if let Err(problem) = std::fs::rename(partial, path) {
        let _ = std::fs::set_permissions(path, before.clone());
        return Err(problem);
    }
    std::fs::set_permissions(path, before.clone())
}

#[cfg(unix)]
fn sync_folder(path: &Path) {
    let folder = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Ok(folder) = std::fs::File::open(folder.unwrap_or(Path::new("."))) {
        let _ = folder.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_folder(_path: &Path) {}

fn damaged(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".damaged");
    name.into()
}

/// Damaged JSON is copied to `<name>.damaged` first: the next write would replace it.
pub fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            crate::problem!("{}: {e}", path.display());
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            let kept = damaged(path);
            match std::fs::copy(path, &kept) {
                Ok(_) => crate::problem!("{}: {e}; kept as {}", path.display(), kept.display()),
                Err(not_kept) => crate::problem!("{}: {e}; not kept: {not_kept}", path.display()),
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_json_reads_as_nothing_and_is_kept_beside_itself() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        assert_eq!(read_json(&path), None);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

        write(&path, r#"{ "a.b": 1 }"#).unwrap();
        assert_eq!(read_json(&path), Some(serde_json::json!({ "a.b": 1 })));
        assert!(!damaged(&path).exists());

        std::fs::write(&path, r#"{ "a.b": { "note": "two years of no"#).unwrap();
        assert_eq!(read_json(&path), None);
        write(&path, "{}").unwrap();
        let kept = std::fs::read_to_string(damaged(&path)).unwrap();
        assert!(kept.ends_with("two years of no"));
    }

    #[test]
    fn the_file_and_its_folder_are_made_and_nothing_else_is_left() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("RimStack").join("notes.v2.json");
        write(&path, "first").unwrap();
        write(&path, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn a_file_that_cannot_be_written_keeps_what_it_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        write(&path.join("inside"), "kept").unwrap();
        let lost = write(&path, "lost").unwrap_err();
        assert_eq!(lost.kind(), crate::base::error::Kind::File);
        assert!(lost.to_string().contains("settings.json"));
        assert_eq!(
            std::fs::read_to_string(path.join("inside")).unwrap(),
            "kept"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn two_writers_of_one_file_both_succeed_and_one_text_is_kept_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let file = &tmp.path().join("notes.json");
        let texts = ["a".repeat(256 * 1024), "b".repeat(64 * 1024)];
        for _ in 0..200 {
            let written: Vec<bool> = std::thread::scope(|scope| {
                let writers: Vec<_> = texts
                    .iter()
                    .map(|text| scope.spawn(move || write(file, text).is_ok()))
                    .collect();
                writers.into_iter().map(|w| w.join().unwrap()).collect()
            });
            assert_eq!(written, [true, true]);
            let kept = std::fs::read_to_string(file).unwrap();
            assert!(texts.contains(&kept), "a file of {} bytes", kept.len());
        }
    }
}
