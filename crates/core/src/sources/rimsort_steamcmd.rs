//! RimSort's SteamCMD downloads straight into the game's Mods folder: each instance links its
//! prefix's `steam/steamapps/workshop/content/294100` to the instance's `local_folder`, and SteamCMD
//! records what it downloaded in `steam/steamapps/workshop/appworkshop_294100.acf` there. The
//! instances are in RimSort's `settings.json`, `{"instances": {"<name>": {"local_folder": …,
//! "steamcmd_install_path": <prefix>}}}`.

use crate::rimworld::paths::APP_ID;
use crate::rimworld::workshop::{Version, installed_versions};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The records of the instances whose Mods folder is `local_mods`; one that names none counts.
pub fn records(settings: &str, local_mods: &Path) -> Vec<PathBuf> {
    let Ok(settings) = serde_json::from_str::<Value>(settings) else {
        return Vec::new();
    };
    let Some(instances) = settings.get("instances").and_then(Value::as_object) else {
        return Vec::new();
    };
    let real = |p: &Path| dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let local_mods = real(local_mods);
    let text = |instance: &Value, key: &str| {
        let text = instance
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default();
        Some(PathBuf::from(text)).filter(|p| !p.as_os_str().is_empty())
    };
    instances
        .values()
        .filter(|i| text(i, "local_folder").is_none_or(|own| real(&own) == local_mods))
        .filter_map(|i| text(i, "steamcmd_install_path"))
        .map(|prefix| prefix.join(format!("steam/steamapps/workshop/appworkshop_{APP_ID}.acf")))
        .collect()
}

/// By Workshop id; a record that cannot be read adds nothing.
pub fn versions(rimsort: &Path, local_mods: &Path) -> HashMap<String, Version> {
    let settings = std::fs::read_to_string(rimsort.join("settings.json")).unwrap_or_default();
    let read = |record: PathBuf| std::fs::read_to_string(record).ok();
    let acfs = records(&settings, local_mods).into_iter().filter_map(read);
    acfs.flat_map(|acf| installed_versions(&acf)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instances_of_this_mods_folder_give_what_their_steamcmd_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let (mods, other) = (tmp.path().join("game/Mods"), tmp.path().join("other/Mods"));
        std::fs::create_dir_all(&mods).unwrap();
        let prefix = |name: &str| tmp.path().join("RimSort/instances").join(name);
        let settings = serde_json::json!({"instances": {
            "Default": {"local_folder": mods, "steamcmd_install_path": prefix("Default")},
            "Other": {"local_folder": other, "steamcmd_install_path": prefix("Other")},
            "Unset": {"local_folder": "", "steamcmd_install_path": ""},
        }});
        let rimsort = tmp.path().join("RimSort");
        std::fs::create_dir_all(&rimsort).unwrap();
        std::fs::write(rimsort.join("settings.json"), settings.to_string()).unwrap();
        for (name, manifest) in [("Default", 7), ("Other", 8)] {
            let workshop = prefix(name).join("steam/steamapps/workshop");
            std::fs::create_dir_all(&workshop).unwrap();
            let acf = format!(
                "\"AppWorkshop\" {{ \"WorkshopItemsInstalled\" {{ \"123\" {{ \"timeupdated\" \"5\" \"manifest\" \"{manifest}\" }} }} }}"
            );
            std::fs::write(workshop.join("appworkshop_294100.acf"), acf).unwrap();
        }
        let found = versions(&rimsort, &mods);
        assert_eq!(
            found["123"],
            Version {
                manifest: 7,
                updated: 5
            }
        );
        assert_eq!(found.len(), 1);
        assert!(versions(&tmp.path().join("none"), &mods).is_empty());
        assert!(records("{", &mods).is_empty());
    }
}
