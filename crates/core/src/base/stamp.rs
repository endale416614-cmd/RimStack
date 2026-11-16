use super::walk;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::ops::ControlFlow;
use std::path::Path;

/// The modification times of a folder and all its subfolders, which change
/// when a file or folder is added, removed or renamed inside one, and not when
/// a file is edited in place. So it vouches for lists of names, never contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderStamp {
    folders: usize,
    sum: u64,
}

impl FolderStamp {
    /// Whatever cannot be read counts as nothing.
    pub fn of(root: &Path, skip: impl Fn(&OsStr) -> bool) -> Self {
        let mut stamp = Self { folders: 0, sum: 0 };
        walk::visit(root, skip, |seen| {
            if let walk::Seen::Folder(folder) = seen {
                let mut hasher = DefaultHasher::new();
                folder.hash(&mut hasher);
                if let Ok(changed) = std::fs::metadata(folder).and_then(|m| m.modified()) {
                    changed.hash(&mut hasher);
                }
                stamp.folders += 1;
                stamp.sum = stamp.sum.wrapping_add(hasher.finish());
            }
            ControlFlow::Continue(())
        });
        stamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    pub fn touch_dir(dir: &Path, later: u64) {
        let file = std::fs::File::open(dir).unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(later))
            .unwrap();
    }

    #[test]
    fn a_change_in_a_subfolder_changes_the_stamp_and_a_skipped_one_does_not() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        std::fs::create_dir_all(tmp.path().join("Textures")).unwrap();
        let skip = |name: &OsStr| name == "Textures";
        let first = FolderStamp::of(tmp.path(), skip);
        assert_eq!(first, FolderStamp::of(tmp.path(), skip));
        touch_dir(&tmp.path().join("Textures"), 100);
        assert_eq!(first, FolderStamp::of(tmp.path(), skip));
        touch_dir(&tmp.path().join("a/b"), 200);
        assert_ne!(first, FolderStamp::of(tmp.path(), skip));
    }
}
