use super::super::App;
use rayon::prelude::*;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::sizes;
use rimstack_core::mods::spare::{self, Spare};
use rimstack_core::rimworld::engine::Engine;
use rimstack_core::rimworld::paths::Source;
use rimstack_core::say_n;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, PartialEq)]
pub struct SpareFile {
    /// Inside the mod's folder.
    pub shown: String,
    pub spare: Spare,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpareMod {
    pub name: String,
    pub bytes: u64,
    pub files: Vec<SpareFile>,
}

/// The mods with the most room to free first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpareFound {
    pub mods: Vec<SpareMod>,
}

impl SpareFound {
    pub fn files(&self) -> usize {
        self.mods.iter().map(|m| m.files.len()).sum()
    }

    pub fn bytes(&self) -> u64 {
        self.mods.iter().map(|m| m.bytes).sum()
    }

    pub fn first_file(&self) -> Option<&std::path::Path> {
        Some(self.mods.first()?.files.first()?.spare.file.as_path())
    }
}

impl App {
    /// From 1.7, in Workshop mods only: Steam brings back a mod's own files.
    pub fn spare_dds(&self, stop: &AtomicBool) -> SpareFound {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return SpareFound::default();
        };
        if library.engine() != Engine::Waylith {
            return SpareFound::default();
        }
        let manifest = self.texture_manifest();
        let mut mods: Vec<SpareMod> = library
            .installed
            .par_iter()
            .filter(|m| m.source == Source::Workshop)
            .filter_map(|m| {
                let found = spare::find(&m.path, &|dds| manifest.lists(dds), stop);
                let files: Vec<SpareFile> = found
                    .into_iter()
                    .map(|spare| SpareFile {
                        shown: spare
                            .file
                            .strip_prefix(&m.path)
                            .unwrap_or(&spare.file)
                            .to_string_lossy()
                            .into_owned(),
                        spare,
                    })
                    .collect();
                (!files.is_empty()).then(|| SpareMod {
                    name: m.display_name().to_string(),
                    bytes: files.iter().map(|f| f.spare.bytes).sum(),
                    files,
                })
            })
            .collect();
        mods.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
        SpareFound { mods }
    }

    pub fn delete_spare_dds(&self, found: &SpareFound) -> Sentence {
        let spare: Vec<Spare> = found
            .mods
            .iter()
            .flat_map(|m| m.files.iter().map(|f| f.spare.clone()))
            .collect();
        let (freed, left) = spare::delete(&spare);
        self.textures_changed();
        let deleted = spare.len() - left.len();
        let done = say_n!(
            deleted,
            "{} DDS file deleted, {} freed",
            "{} DDS files deleted, {} freed",
            sizes::of_bytes(freed).inside()
        );
        if left.is_empty() {
            return done;
        }
        for file in &left {
            rimstack_core::problem!("{} was not deleted", file.display());
        }
        let kept = say_n!(
            left.len(),
            "{} file had changed or could not be deleted",
            "{} files had changed or could not be deleted"
        );
        Sentence::listed(vec![done, kept])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn from_1_7_a_workshop_mod_s_dds_showing_its_png_is_found_and_deleted() {
        let (tmp, app) = installed();
        let root = tmp.path();
        let mod_root = root.join("workshop/2000/Textures");
        std::fs::create_dir_all(root.join("workshop/2000/About")).unwrap();
        std::fs::write(
            root.join("workshop/2000/About/About.xml"),
            "<ModMetaData><packageId>Ann.Hats</packageId><name>Hats</name></ModMetaData>",
        )
        .unwrap();
        std::fs::create_dir_all(&mod_root).unwrap();
        let picture = image::RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([(x * 4) as u8, (y * 4) as u8, 90, 255])
        });
        picture.save(mod_root.join("hat.png")).unwrap();
        let png = std::fs::read(mod_root.join("hat.png")).unwrap();
        let dds = rimstack_core::mods::textures::encode(&png).unwrap();
        std::fs::write(mod_root.join("hat.dds"), &dds).unwrap();
        let local = root.join("game/Mods/roads/Textures");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("road.png"), &png).unwrap();
        std::fs::write(local.join("road.dds"), &dds).unwrap();
        std::fs::write(root.join("game/Version.txt"), "1.7.5005 rev1082").unwrap();
        app.rescan();
        let found = app.spare_dds(&AtomicBool::new(false));
        assert_eq!((found.files(), found.mods.len()), (1, 1));
        assert_eq!(found.first_file(), Some(mod_root.join("hat.dds").as_path()));
        let hats = &found.mods[0];
        assert_eq!(
            (hats.name.as_str(), hats.files[0].shown.as_str()),
            ("Hats", "Textures/hat.dds")
        );
        assert_eq!(found.bytes(), dds.len() as u64);

        let said = app.delete_spare_dds(&found).to_string();
        assert!(said.starts_with("1 DDS file deleted"), "{said}");
        assert!(!mod_root.join("hat.dds").exists() && mod_root.join("hat.png").exists());
        assert!(local.join("road.dds").exists(), "a local mod keeps its DDS");
    }
}
