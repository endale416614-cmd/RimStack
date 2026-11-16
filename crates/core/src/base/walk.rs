//! A mod's links may loop: a folder is entered once however it is reached.

use std::ffi::OsStr;
use std::fs::FileType;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

pub struct Entry {
    pub path: PathBuf,
    pub within: PathBuf,
    pub is_dir: bool,
}

pub enum Seen<'a> {
    Folder(&'a Path),
    Item(&'a Path, FileType),
}

/// In no order. Whatever cannot be read counts as nothing. Returns whether `on` stopped it.
pub fn visit(
    root: &Path,
    skip: impl Fn(&OsStr) -> bool,
    mut on: impl FnMut(Seen) -> ControlFlow<()>,
) -> bool {
    let mut waiting = vec![root.to_path_buf()];
    while let Some(folder) = waiting.pop() {
        if on(Seen::Folder(&folder)).is_break() {
            return true;
        }
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if !skip(&entry.file_name()) {
                    waiting.push(path);
                }
            } else if on(Seen::Item(&path, kind)).is_break() {
                return true;
            }
        }
    }
    false
}

/// In no order. Links to folders are followed unless they lead into or above
/// a folder already walked. A folder that cannot be read is skipped.
pub fn tree(dir: &Path, deepest: usize) -> Vec<Entry> {
    walk(dir, deepest, false, true).unwrap_or_default()
}

/// Fails on an unreadable entry. Folder links are never followed: a mod's own link
/// could bring any of the user's files into a copy.
pub fn whole_tree(dir: &Path) -> std::io::Result<Vec<Entry>> {
    walk(dir, usize::MAX, true, false)
}

fn walk(dir: &Path, deepest: usize, strict: bool, follow: bool) -> std::io::Result<Vec<Entry>> {
    let mut found = Vec::new();
    let mut entered: Vec<PathBuf> = Vec::new();
    let mut file_links = Vec::new();
    let mut waiting = vec![(dir.to_path_buf(), PathBuf::new(), 0)];
    while let Some((folder, within, depth)) = waiting.pop() {
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(problem) if strict => return Err(problem),
            Err(_) => continue,
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(problem) if strict => return Err(problem),
                Err(_) => continue,
            };
            let (path, inside) = (entry.path(), within.join(entry.file_name()));
            let is_dir = match entry.file_type() {
                Ok(kind) if kind.is_dir() => true,
                Ok(kind) if kind.is_symlink() && path.is_dir() => {
                    if !follow || !may_enter(dir, &path, &mut entered) {
                        continue;
                    }
                    true
                }
                Ok(kind) if kind.is_symlink() => {
                    file_links.push(Entry {
                        path,
                        within: inside,
                        is_dir: false,
                    });
                    continue;
                }
                _ => false,
            };
            if is_dir && depth < deepest {
                waiting.push((path.clone(), inside.clone(), depth + 1));
            }
            found.push(Entry {
                path,
                within: inside,
                is_dir,
            });
        }
    }
    let within = file_links
        .into_iter()
        .filter(|link| leads_within(dir, &link.path, &mut entered));
    found.extend(within);
    Ok(found)
}

pub fn files(dir: &Path) -> Vec<PathBuf> {
    let all = tree(dir, usize::MAX).into_iter();
    all.filter(|entry| !entry.is_dir)
        .map(|entry| entry.path)
        .collect()
}

pub fn strict_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let all = walk(dir, usize::MAX, true, true)?.into_iter();
    Ok(all
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path)
        .collect())
}

/// A link to a file within it is copied as the file it points at.
pub fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let all = whole_tree(from)?;
    std::fs::create_dir_all(to)?;
    for entry in all {
        let target = to.join(&entry.within);
        match entry.is_dir {
            true => std::fs::create_dir_all(&target)?,
            false => drop(std::fs::copy(&entry.path, &target)?),
        }
    }
    Ok(())
}

fn may_enter(began: &Path, link: &Path, entered: &mut Vec<PathBuf>) -> bool {
    if entered.is_empty() {
        entered.extend(dunce::canonicalize(began));
    }
    let Ok(leads_to) = dunce::canonicalize(link) else {
        return false;
    };
    let apart = |walked: &PathBuf| !leads_to.starts_with(walked) && !walked.starts_with(&leads_to);
    let new = !entered.is_empty() && entered.iter().all(apart);
    if new {
        entered.push(leads_to);
    }
    new
}

fn leads_within(began: &Path, link: &Path, entered: &mut Vec<PathBuf>) -> bool {
    if entered.is_empty() {
        entered.extend(dunce::canonicalize(began));
    }
    dunce::canonicalize(link)
        .is_ok_and(|leads_to| entered.iter().any(|walked| leads_to.starts_with(walked)))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = files(dir)
            .iter()
            .map(|file| {
                file.strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        found.sort();
        found
    }

    #[test]
    fn files_are_found_at_any_depth_and_a_missing_folder_has_none() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        fs::write(tmp.path().join("top.txt"), "").unwrap();
        fs::write(tmp.path().join("a/b/deep.txt"), "").unwrap();
        assert_eq!(names(tmp.path()), ["a/b/deep.txt", "top.txt"]);
        assert!(files(&tmp.path().join("absent")).is_empty());
    }

    #[test]
    fn links_back_up_and_to_each_other_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/one.txt"), "").unwrap();
        symlink(&root, root.join("a/up")).unwrap();
        symlink(root.join("a"), root.join("beside")).unwrap();
        symlink(tmp.path(), root.join("above")).unwrap();
        assert_eq!(names(&root), ["a/one.txt"]);
    }

    #[test]
    fn a_link_to_a_folder_elsewhere_is_followed_once() {
        let tmp = tempfile::tempdir().unwrap();
        let (root, elsewhere) = (tmp.path().join("root"), tmp.path().join("elsewhere"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(elsewhere.join("inner")).unwrap();
        fs::write(elsewhere.join("inner/far.txt"), "").unwrap();
        symlink(&elsewhere, root.join("first")).unwrap();
        symlink(elsewhere.join("inner"), root.join("second")).unwrap();
        symlink(&elsewhere, elsewhere.join("inner/round")).unwrap();
        assert_eq!(files(&root).len(), 1);
    }

    #[test]
    fn a_walk_stops_at_its_depth_and_names_what_it_found_from_where_it_began() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();
        fs::write(tmp.path().join("a/b/c/deep.txt"), "").unwrap();
        let mut near: Vec<PathBuf> = tree(tmp.path(), 1).into_iter().map(|e| e.within).collect();
        near.sort();
        assert_eq!(near, [PathBuf::from("a"), PathBuf::from("a/b")]);
    }

    #[test]
    fn a_copy_takes_no_linked_folder_and_a_loop_ends() {
        let tmp = tempfile::tempdir().unwrap();
        let (from, private) = (tmp.path().join("mod"), tmp.path().join(".ssh"));
        std::fs::create_dir_all(from.join("About")).unwrap();
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("id_ed25519"), "key").unwrap();
        std::fs::write(from.join("About/About.xml"), "<ModMetaData/>").unwrap();
        symlink(&private, from.join("Textures")).unwrap();
        symlink(&from, from.join("itself")).unwrap();
        let to = tmp.path().join("copy");
        copy(&from, &to).unwrap();
        assert!(to.join("About/About.xml").is_file());
        assert!(!to.join("Textures").exists());
        assert!(!to.join("itself").exists());
        assert!(names(&from).contains(&"Textures/id_ed25519".to_string()));
    }

    #[test]
    fn a_link_to_a_file_is_followed_only_into_what_is_walked() {
        let tmp = tempfile::tempdir().unwrap();
        let (from, shared) = (tmp.path().join("mod"), tmp.path().join("shared"));
        fs::create_dir_all(from.join("About")).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(tmp.path().join("private.txt"), "not the mod's").unwrap();
        fs::write(from.join("About/About.xml"), "<ModMetaData/>").unwrap();
        fs::write(shared.join("b.png"), "png").unwrap();
        symlink(
            tmp.path().join("private.txt"),
            from.join("About/Preview.png"),
        )
        .unwrap();
        symlink(from.join("About/About.xml"), from.join("About.xml")).unwrap();
        symlink(&shared, from.join("Textures")).unwrap();
        symlink(shared.join("b.png"), from.join("Textures-b.png")).unwrap();
        symlink(tmp.path().join("gone"), from.join("broken")).unwrap();

        let mut found = names(&from);
        found.sort();
        assert_eq!(
            found,
            [
                "About.xml",
                "About/About.xml",
                "Textures-b.png",
                "Textures/b.png"
            ]
        );
        let to = tmp.path().join("copy");
        copy(&from, &to).unwrap();
        assert!(!to.join("About/Preview.png").exists());
        assert_eq!(
            fs::read_to_string(to.join("About.xml")).unwrap(),
            "<ModMetaData/>"
        );
        assert!(!to.join("Textures-b.png").exists());
    }
}
