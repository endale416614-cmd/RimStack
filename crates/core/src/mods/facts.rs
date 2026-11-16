use crate::base::path_text::has_extension;
use crate::base::walk;
use crate::mods::infer::{self, Scanned};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facts {
    pub folders: Vec<PathBuf>,
    /// Whole for a mod that is active, which the rules are inferred from; for the others only
    /// the def names and the names patched.
    pub scanned: Scanned,
    pub whole: bool,
    defs: bool,
    patches: bool,
    textures: bool,
    sounds: bool,
    languages: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Has {
    Defs,
    Patches,
    Textures,
    Sounds,
    Languages,
}

impl Has {
    pub fn named(word: &str) -> Option<Self> {
        Some(match word {
            "defs" => Self::Defs,
            "patches" => Self::Patches,
            "textures" => Self::Textures,
            "sounds" => Self::Sounds,
            "languages" => Self::Languages,
            _ => return None,
        })
    }
}

fn is_xml(path: &Path) -> bool {
    has_extension(path, "xml")
}

fn holds(dir: &Path, wanted: &dyn Fn(&Path) -> bool) -> bool {
    walk::visit(
        dir,
        |_| false,
        |seen| match seen {
            walk::Seen::Item(path, _) if wanted(path) => ControlFlow::Break(()),
            _ => ControlFlow::Continue(()),
        },
    )
}

fn any_in(folders: &[PathBuf], name: &str, wanted: &dyn Fn(&Path) -> bool) -> bool {
    let dirs = folders.iter().filter_map(|f| infer::subfolder(f, name));
    dirs.into_iter().any(|dir| holds(&dir, wanted))
}

impl Facts {
    pub fn read(load_folders: Vec<PathBuf>, game_assemblies: &HashSet<String>) -> Self {
        let scanned = infer::scan(&load_folders, game_assemblies);
        Self::with(load_folders, scanned, true)
    }

    pub fn read_names(load_folders: Vec<PathBuf>) -> Self {
        let found = infer::scan_xml(&load_folders);
        let mut patched: Vec<(String, PathBuf)> = found.patched;
        patched.sort();
        patched.dedup_by(|a, b| a.0 == b.0);
        let patched = patched.into_iter().map(|(name, _)| (name, PathBuf::new()));
        let scanned = Scanned {
            defines: found.defines,
            patched: patched.collect(),
            ..Scanned::default()
        };
        Self::with(load_folders, scanned, false)
    }

    fn with(load_folders: Vec<PathBuf>, scanned: Scanned, whole: bool) -> Self {
        let any = |_: &Path| true;
        Self {
            scanned,
            whole,
            defs: any_in(&load_folders, "Defs", &is_xml),
            patches: any_in(&load_folders, "Patches", &is_xml),
            textures: any_in(&load_folders, "Textures", &any),
            sounds: any_in(&load_folders, "Sounds", &any),
            languages: any_in(&load_folders, "Languages", &any),
            folders: load_folders,
        }
    }

    pub fn has(&self, what: Has) -> bool {
        match what {
            Has::Defs => self.defs,
            Has::Patches => self.patches,
            Has::Textures => self.textures,
            Has::Sounds => self.sounds,
            Has::Languages => self.languages,
        }
    }

    pub fn defines(&self, def_name: &str) -> bool {
        let same = |name: &String| name.eq_ignore_ascii_case(def_name);
        self.scanned.defines.iter().any(same)
    }

    pub fn patches_def(&self, def_name: &str) -> bool {
        let same = |(name, _): &(String, PathBuf)| name.eq_ignore_ascii_case(def_name);
        self.scanned.patched.iter().any(same)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::loadfolders;

    pub fn write(root: &Path, file: &str, text: &str) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn facts_of(root: &Path) -> Facts {
        let folders = loadfolders::every_folder(root, "1.6");
        Facts::read(folders, &HashSet::new())
    }

    #[test]
    fn a_mod_with_nothing_has_no_fact() {
        let tmp = tempfile::tempdir().unwrap();
        let facts = facts_of(tmp.path());
        for what in [
            Has::Defs,
            Has::Patches,
            Has::Textures,
            Has::Sounds,
            Has::Languages,
        ] {
            assert!(!facts.has(what), "{what:?}");
        }
        assert!(!facts.defines("Steel"));
    }

    #[test]
    fn each_has_word_comes_from_its_own_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "1.6/Defs/Things/a.xml",
            "<Defs><ThingDef><defName>Gold</defName></ThingDef></Defs>",
        );
        write(root, "1.6/Textures/Things/gold.png", "x");
        write(root, "Languages/French/Keyed/a.xml", "<LanguageData/>");
        let facts = facts_of(root);
        assert!(facts.has(Has::Defs) && facts.has(Has::Textures) && facts.has(Has::Languages));
        assert!(!facts.has(Has::Patches) && !facts.has(Has::Sounds));
        write(root, "Common/Patches/p.xml", "<Patch/>");
        write(root, "Common/Sounds/s.ogg", "x");
        let facts = facts_of(root);
        assert!(facts.has(Has::Patches) && facts.has(Has::Sounds));
    }

    #[test]
    fn a_folder_holding_no_xml_is_no_defs_and_no_patches() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "Defs/notes.txt", "x");
        write(tmp.path(), "Patches/readme.md", "x");
        let facts = facts_of(tmp.path());
        assert!(!facts.has(Has::Defs) && !facts.has(Has::Patches));
    }

    #[test]
    fn def_names_match_exactly_ignoring_case() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "Defs/a.xml",
            "<Defs><ThingDef><defName>Gold</defName></ThingDef></Defs>",
        );
        write(
            root,
            "Patches/p.xml",
            r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Steel"]/x</xpath></Operation></Patch>"#,
        );
        let facts = facts_of(root);
        assert!(facts.defines("gold") && facts.defines("GOLD"));
        assert!(!facts.defines("Gol") && !facts.defines("Steel"));
        assert!(facts.patches_def("steel") && !facts.patches_def("Gold"));
    }

    #[test]
    fn an_inactive_mod_is_read_with_every_version_folder_whatever_the_conditions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "LoadFolders.xml",
            r#"<loadFolders><v1.6><li>Plain</li><li IfModActive="other.mod">Needs</li><li IfModNotActive="x.y">Without</li></v1.6></loadFolders>"#,
        );
        for folder in ["Plain", "Needs", "Without"] {
            write(
                root,
                &format!("{folder}/Defs/a.xml"),
                &format!("<Defs><ThingDef><defName>{folder}Def</defName></ThingDef></Defs>"),
            );
        }
        let facts = facts_of(root);
        assert_eq!(facts.folders.len(), 3);
        assert!(facts.defines("NeedsDef") && facts.defines("WithoutDef"));
        let narrowed = loadfolders::folders(root, "1.6", &|_| false);
        assert_eq!(narrowed.len(), 2);
    }

    #[test]
    fn the_names_alone_are_kept_without_paths_or_repeats() {
        let tmp = tempfile::tempdir().unwrap();
        let patch = r#"<Patch><Operation><xpath>Defs/ThingDef[defName="Steel"]/a</xpath></Operation><Operation><xpath>Defs/ThingDef[defName="Steel"]/b</xpath></Operation></Patch>"#;
        write(tmp.path(), "Patches/p.xml", patch);
        write(
            tmp.path(),
            "Defs/a.xml",
            "<Defs><ThingDef ParentName=\"Base\"><defName>Gold</defName></ThingDef></Defs>",
        );
        let folders = loadfolders::every_folder(tmp.path(), "1.6");
        let whole = Facts::read(folders.clone(), &HashSet::new());
        let names = Facts::read_names(folders);
        assert!(whole.whole && !names.whole);
        assert_eq!(whole.scanned.patched.len(), 2);
        assert_eq!(
            names.scanned.patched,
            [("Steel".to_string(), PathBuf::new())]
        );
        assert!(names.scanned.parents.is_empty() && names.defines("gold"));
        assert!(names.has(Has::Defs) && names.has(Has::Patches));
    }

    #[test]
    fn facts_survive_being_written_and_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            "Defs/a.xml",
            "<Defs><ThingDef><defName>Gold</defName></ThingDef></Defs>",
        );
        let facts = facts_of(tmp.path());
        let text = serde_json::to_string(&facts).unwrap();
        assert_eq!(serde_json::from_str::<Facts>(&text).unwrap(), facts);
    }
}
