use crate::rimworld::id;
use roxmltree::Document;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// Relative to the mod's root; empty for the root itself.
    pub folder: String,
    /// The `IfModActive` attribute.
    pub any_of: Vec<String>,
    /// The `IfModActiveAll` attribute.
    pub all_of: Vec<String>,
    /// The `IfModNotActive` attribute.
    pub none_of: Vec<String>,
}

impl Entry {
    fn applies(&self, is_active: &dyn Fn(&str) -> bool) -> bool {
        (self.any_of.is_empty() || self.any_of.iter().any(|m| is_active(m)))
            && self.all_of.iter().all(|m| is_active(m))
            && !self.none_of.iter().any(|m| is_active(m))
    }
}

/// `None` when there is no such list or the file is not XML.
pub fn declared(xml: &str, game_version: &str) -> Option<Vec<Entry>> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let doc = Document::parse(xml).ok()?;
    let lists = || doc.root_element().children().filter(|n| n.is_element());
    let list_for = |wanted: &str| {
        lists().find(|n| {
            let tag = n.tag_name().name();
            tag.strip_prefix('v')
                .unwrap_or(tag)
                .eq_ignore_ascii_case(wanted)
        })
    };
    // A folder named only under `v1.6` loads.
    let number = |name: &str| -> Option<(u32, u32)> {
        let (major, minor) = name
            .strip_prefix(['v', 'V'])
            .unwrap_or(name)
            .split_once('.')?;
        Some((major.parse().ok()?, minor.parse().ok()?))
    };
    let below = || {
        let game = number(game_version)?;
        let older = lists().filter_map(|n| Some((number(n.tag_name().name())?, n)));
        older
            .filter(|(version, _)| *version < game)
            .max_by_key(|(version, _)| *version)
    };
    let list = list_for(game_version)
        .or_else(|| list_for("default"))
        .or_else(|| below().map(|(_, list)| list))?;
    let ids = |value: Option<&str>| -> Vec<String> {
        let parts = value.unwrap_or("").split(',');
        parts.map(id::fold).filter(|s| !s.is_empty()).collect()
    };
    let entries = list
        .children()
        .filter(|n| n.has_tag_name("li"))
        .filter_map(|li| {
            let folder = li.text().unwrap_or("").trim().replace('\\', "/");
            let folder = folder.trim_matches('/').to_string();
            // `..` or a drive prefix would lead out of the mod.
            let inside = Path::new(&folder)
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
            inside.then(|| Entry {
                folder,
                any_of: ids(li.attribute("IfModActive")),
                all_of: ids(li.attribute("IfModActiveAll")),
                none_of: ids(li.attribute("IfModNotActive")),
            })
        })
        .collect();
    Some(entries)
}

/// `is_active` takes folded package ids.
pub fn folders(root: &Path, game_version: &str, is_active: &dyn Fn(&str) -> bool) -> Vec<PathBuf> {
    folders_where(root, game_version, &|entry| entry.applies(is_active))
}

pub fn every_folder(root: &Path, game_version: &str) -> Vec<PathBuf> {
    folders_where(root, game_version, &|_| true)
}

fn folders_where(root: &Path, game_version: &str, takes: &dyn Fn(&Entry) -> bool) -> Vec<PathBuf> {
    let listed = std::fs::read_to_string(root.join("LoadFolders.xml"))
        .ok()
        .and_then(|xml| declared(&xml, game_version));
    let mut wanted: Vec<PathBuf> = match listed {
        Some(entries) => entries
            .iter()
            .filter(|e| takes(e))
            .map(|e| root.join(&e.folder))
            .collect(),
        None => Vec::new(),
    };
    if wanted.is_empty() {
        wanted = vec![root.to_path_buf(), root.join("Common")];
        wanted.extend(version_folder(root, game_version));
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for folder in wanted {
        if folder.is_dir() && !found.contains(&folder) {
            found.push(folder);
        }
    }
    found
}

fn version_folder(root: &Path, game_version: &str) -> Option<PathBuf> {
    let number = |name: &str| -> Option<(u32, u32)> {
        let (major, minor) = name.split_once('.')?;
        Some((major.parse().ok()?, minor.parse().ok()?))
    };
    let game = number(game_version)?;
    let mut versions: Vec<((u32, u32), PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| Some((number(&e.file_name().to_string_lossy())?, e.path())))
        .filter(|(version, _)| *version <= game)
        .collect();
    versions.sort();
    versions.pop().map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_without_a_list_takes_the_list_of_the_highest_version_below() {
        let xml = "<loadFolders><v1.5><li>old</li></v1.5><v1.6><li>newer</li></v1.6><v1.8><li>later</li></v1.8></loadFolders>";
        let folder =
            |version: &str| declared(xml, version).map(|entries| entries[0].folder.clone());
        assert_eq!(folder("1.6").as_deref(), Some("newer"));
        assert_eq!(folder("1.7").as_deref(), Some("newer"));
        assert_eq!(folder("1.8").as_deref(), Some("later"));
        assert_eq!(folder("1.4"), None);
    }
    use std::fs;

    fn a_mod(dirs: &[&str], load_folders: Option<&str>) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mod");
        fs::create_dir_all(&root).unwrap();
        for dir in dirs {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        if let Some(xml) = load_folders {
            fs::write(root.join("LoadFolders.xml"), xml).unwrap();
        }
        (tmp, root)
    }

    fn names(root: &Path, found: &[PathBuf]) -> Vec<String> {
        let name = |p: &PathBuf| p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
        found.iter().map(name).collect()
    }

    const NONE: &dyn Fn(&str) -> bool = &|_| false;

    #[test]
    fn without_a_file_the_root_common_and_the_version_folder_load() {
        let (_t, root) = a_mod(&["Common", "1.5", "1.6", "1.7", "Textures"], None);
        assert_eq!(
            names(&root, &folders(&root, "1.6", NONE)),
            ["", "Common", "1.6"]
        );
    }

    #[test]
    fn a_missing_version_folder_falls_back_to_the_highest_older_one() {
        let (_t, root) = a_mod(&["1.3", "1.4", "1.7"], None);
        assert_eq!(names(&root, &folders(&root, "1.6", NONE)), ["", "1.4"]);
        let (_t, root) = a_mod(&["1.7"], None);
        assert_eq!(names(&root, &folders(&root, "1.6", NONE)), [""]);
    }

    #[test]
    fn a_list_for_the_version_replaces_the_built_in_folders() {
        let xml = r"<loadFolders>
            <v1.5><li>/</li><li>Old</li></v1.5>
            <v1.6><li>/</li><li>New\Stuff</li><li>Missing</li><li>/</li></v1.6>
        </loadFolders>";
        let (_t, root) = a_mod(&["Old", "New/Stuff", "1.6", "Common"], Some(xml));
        assert_eq!(
            names(&root, &folders(&root, "1.6", NONE)),
            ["", "New/Stuff"]
        );
    }

    #[test]
    fn the_default_list_is_used_when_the_version_has_none() {
        let xml =
            "<loadFolders><default><li>Base</li></default><v1.5><li>Old</li></v1.5></loadFolders>";
        let (_t, root) = a_mod(&["Base", "Old"], Some(xml));
        assert_eq!(names(&root, &folders(&root, "1.6", NONE)), ["Base"]);
    }

    #[test]
    fn a_file_with_only_newer_lists_or_that_is_broken_uses_the_built_in_folders() {
        let (_t, root) = a_mod(
            &["1.6", "New"],
            Some("<loadFolders><v1.7><li>New</li></v1.7></loadFolders>"),
        );
        assert_eq!(names(&root, &folders(&root, "1.6", NONE)), ["", "1.6"]);
        let (_t, root) = a_mod(&["1.6"], Some("<loadFolders><v1.6>"));
        assert_eq!(names(&root, &folders(&root, "1.6", NONE)), ["", "1.6"]);
    }

    #[test]
    fn entries_follow_which_mods_are_active() {
        let xml = r#"<loadFolders><v1.6>
            <li>/</li>
            <li IfModActive="Some.Mod, other.mod">Any</li>
            <li IfModActiveAll="some.mod, other.mod">All</li>
            <li IfModNotActive="some.mod">Not</li>
            <li IfModActiveAny="never.heard.of.it">Unknown</li>
        </v1.6></loadFolders>"#;
        let dirs = ["Any", "All", "Not", "Unknown"];
        let (_t, root) = a_mod(&dirs, Some(xml));
        let with = |active: &'static [&'static str]| {
            let found = folders(&root, "1.6", &|m| active.contains(&m));
            names(&root, &found)
        };
        // The game does not know IfModActiveAny, so it always loads.
        assert_eq!(with(&[]), ["", "Not", "Unknown"]);
        assert_eq!(with(&["some.mod"]), ["", "Any", "Unknown"]);
        assert_eq!(
            with(&["some.mod", "other.mod"]),
            ["", "Any", "All", "Unknown"]
        );
    }

    #[test]
    fn a_mod_is_only_read_from_folders_inside_it() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mods/evil");
        fs::create_dir_all(tmp.path().join("mods/victim/Textures")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("LoadFolders.xml"),
            "<loadFolders><v1.6><li>../victim</li></v1.6></loadFolders>",
        )
        .unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let inside = |folder: &PathBuf| fs::canonicalize(folder).unwrap().starts_with(&root);
        assert!(folders(&root, "1.6", NONE).iter().all(inside));
    }
}
