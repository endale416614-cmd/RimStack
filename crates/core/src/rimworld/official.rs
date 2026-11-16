use crate::rimworld::{about, id};
use std::path::Path;

const CORE: &str = "ludeon.rimworld";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Official {
    pub name: String,
    pub author: String,
    pub picture: Option<String>,
    #[serde(skip)]
    pub package_id: String,
}

/// Sorted with the game first, then the expansions by folder name.
pub fn installed(game: &Path) -> Vec<Official> {
    let mut found: Vec<(bool, String, Official)> = std::fs::read_dir(game.join("Data"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            let path = entry.path();
            let about = about::parse(&std::fs::read(about::locate(&path)?).ok()?).ok()?;
            let folder = entry.file_name().to_string_lossy().into_owned();
            let official = Official {
                name: about.name.unwrap_or_else(|| folder.clone()),
                author: about.authors.join(", "),
                picture: about::preview(&path).map(|p| p.to_string_lossy().into_owned()),
                package_id: id::fold(&about.package_id),
            };
            Some((official.package_id != CORE, folder, official))
        })
        .collect();
    found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    found.into_iter().map(|(_, _, official)| official).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn add(game: &Path, folder: &str, about: &str) {
        fs::create_dir_all(game.join("Data").join(folder).join("About")).unwrap();
        fs::write(
            game.join("Data").join(folder).join("About/About.xml"),
            about,
        )
        .unwrap();
    }

    #[test]
    fn the_game_comes_first_then_its_expansions_each_with_its_picture() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        add(
            game,
            "Royalty",
            "<ModMetaData><packageId>Ludeon.RimWorld.Royalty</packageId><author>Ludeon Studios</author></ModMetaData>",
        );
        add(
            game,
            "Anomaly",
            "<ModMetaData><packageId>Ludeon.RimWorld.Anomaly</packageId><name>Anomaly</name></ModMetaData>",
        );
        add(
            game,
            "Core",
            "<ModMetaData><packageId>Ludeon.RimWorld</packageId><name>Core</name></ModMetaData>",
        );
        add(game, "Broken", "<ModMetaData><name>");
        fs::write(game.join("Data/Royalty/About/Preview.png"), b"").unwrap();
        fs::write(game.join("Data/Place official expansions here.txt"), b"").unwrap();

        let found = installed(game);
        let names: Vec<&str> = found.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Core", "Anomaly", "Royalty"]);
        assert_eq!(found[2].author, "Ludeon Studios");
        let picture = game.join("Data/Royalty/About/Preview.png");
        assert_eq!(found[2].picture.as_deref(), picture.to_str());
        assert_eq!(found[0].picture, None);
        assert_eq!(found[2].package_id, "ludeon.rimworld.royalty");
    }

    #[test]
    fn no_data_folder_is_no_expansion() {
        assert!(installed(Path::new("/nowhere/at/all")).is_empty());
    }
}
