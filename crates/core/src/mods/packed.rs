//! From 1.7 a load folder may hold `Assets.zip`, the mod's textures as the game
//! baked them: `Textures/<path>.<ext>.dds`, or `-premul.dds`, either with
//! `.split` after it. The game looks one up by the name of the file it loads,
//! and a loose `Textures/<path>.dds` in any loaded folder is loaded in place of
//! `<path>.<ext>`, so the baked one is never used. The baker never packs a `.dds`.
//! Names the pack lists only in its compressed manifest (pictures it found
//! identical to another) are not seen.

use crate::base::{walk, zipnames};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PACK: &str = "Assets.zip";

/// `textures/<path>` in lower case, as the game compares them.
fn packed_texture(entry: &str) -> Option<String> {
    let entry = entry.to_lowercase();
    let entry = entry.strip_suffix(".split").unwrap_or(&entry);
    let source = entry
        .strip_suffix("-premul.dds")
        .or_else(|| entry.strip_suffix(".dds"))?;
    let (path, extension) = source.rsplit_once('.')?;
    let picture = path.starts_with("textures/") && !extension.contains('/') && extension != "dds";
    picture.then(|| path.to_string())
}

fn textures_folder(folder: &Path) -> Option<PathBuf> {
    std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && crate::mods::textures::is_textures(p))
}

fn loose_texture(folder: &Path, file: &Path) -> Option<String> {
    let extension = file.extension()?;
    if !extension.eq_ignore_ascii_case("dds") {
        return None;
    }
    let relative = file.with_extension("");
    let relative = relative.strip_prefix(folder).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    Some(parts.join("/"))
}

/// The loose DDS files in `loaded`, the mod's folders the game reads, that hide
/// a texture packed in one of their `Assets.zip`.
pub fn hiding(loaded: &[PathBuf]) -> Vec<PathBuf> {
    let packed: HashSet<String> = loaded
        .iter()
        .filter_map(|folder| zipnames::names_in(&folder.join(PACK)).ok())
        .flatten()
        .filter_map(|entry| packed_texture(&entry))
        .collect();
    if packed.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<PathBuf> = loaded
        .iter()
        .filter_map(|folder| Some((folder, textures_folder(folder)?)))
        .flat_map(|(folder, textures)| {
            walk::files(&textures)
                .into_iter()
                .filter(|file| loose_texture(folder, file).is_some_and(|t| packed.contains(&t)))
                .collect::<Vec<_>>()
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn pack(at: &Path, entries: &[&str]) {
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, crate::base::zipnames::zip_of(entries, false)).unwrap();
    }

    fn file(at: &Path) {
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, "x").unwrap();
    }

    #[test]
    fn a_packed_entry_names_its_source_without_the_extension() {
        assert_eq!(
            packed_texture("Textures/Things/Rock.png.dds").as_deref(),
            Some("textures/things/rock")
        );
        assert_eq!(
            packed_texture("Textures/UI/Big.PNG-premul.dds.split").as_deref(),
            Some("textures/ui/big")
        );
        assert_eq!(packed_texture("Sounds/Hit.ogg"), None);
        assert_eq!(packed_texture("bake.manifest"), None);
        assert_eq!(packed_texture("Textures/Plain.dds"), None);
    }

    #[test]
    fn a_loose_dds_in_any_loaded_folder_hides_the_packed_texture_of_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let (root, newer) = (dir.path().join("mod"), dir.path().join("mod/1.7"));
        pack(
            &newer.join("Assets.zip"),
            &[
                "Textures/Things/Rock.png.dds",
                "Textures/Things/Tree.png.dds.split",
            ],
        );
        file(&root.join("Textures/Things/rock.dds"));
        file(&newer.join("Textures/Things/Tree.dds"));
        file(&newer.join("Textures/Things/Bush.dds"));
        file(&newer.join("Textures/Things/Rock.png"));
        file(&dir.path().join("mod/1.5/Textures/Things/Tree.dds"));

        let hidden = hiding(&[newer.clone(), root.clone()]);
        assert_eq!(
            hidden,
            [
                newer.join("Textures/Things/Tree.dds"),
                root.join("Textures/Things/rock.dds"),
            ]
        );
        assert!(hiding(&[root]).is_empty(), "no pack is loaded");
    }

    #[test]
    #[ignore = "needs RIMSTACK_PACKED_MODS: a folder of mods, some shipping Assets.zip"]
    fn real_packs_are_read_and_the_textures_they_hide_are_found() {
        let mods = PathBuf::from(std::env::var_os("RIMSTACK_PACKED_MODS").unwrap());
        let (mut packs, mut hiding_mods, mut hidden) = (0, 0, 0);
        for root in fs::read_dir(&mods).unwrap().flatten().map(|e| e.path()) {
            let loaded = crate::rimworld::loadfolders::every_folder(&root, "1.7");
            for pack in loaded.iter().map(|f| f.join(PACK)).filter(|p| p.is_file()) {
                let names = zipnames::names_in(&pack).unwrap();
                packs += usize::from(names.iter().any(|n| packed_texture(n).is_some()));
            }
            let found = hiding(&loaded).len();
            hiding_mods += usize::from(found > 0);
            hidden += found;
        }
        println!(
            "{packs} packs with textures; {hidden} loose DDS hide a packed texture in {hiding_mods} mods"
        );
        assert!(packs > 0, "no Assets.zip with textures found");
    }
}
