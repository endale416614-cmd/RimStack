//! Kept as `package id<tab>Workshop id<tab>name` lines, one per mod.

use super::workshopindex::{self, Index};
use crate::base::error::Error;
use crate::said;

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

const URL: &str =
    "https://raw.githubusercontent.com/RimSort/Steam-Workshop-Database/main/steamDB.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub workshop_id: String,
    pub name: String,
}

/// Keyed by folded package id.
pub type Names = HashMap<String, Item>;

#[derive(Deserialize)]
struct Database {
    database: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "packageId")]
    package_id: Option<String>,
    name: Option<String>,
    #[serde(rename = "steamName")]
    steam_name: Option<String>,
    authors: Option<String>,
    tags: Option<Vec<Tag>>,
    /// Id to `[name, link]`.
    dependencies: Option<BTreeMap<String, Vec<String>>>,
    unpublished: Option<bool>,
    appid: Option<bool>,
}

#[derive(Deserialize)]
struct Tag {
    tag: Option<String>,
}

const BASE_GAME: &str = "294100";
const STORE_APP: &str = "store.steampowered.com/app/";

fn indexed(workshop_id: &str, entry: &Entry) -> Option<workshopindex::Entry> {
    if entry.unpublished == Some(true) || entry.appid == Some(true) {
        return None;
    }
    let (mut requires, mut dlc) = (Vec::new(), Vec::new());
    for (id, info) in entry.dependencies.iter().flatten() {
        let store = info.get(1).is_some_and(|link| link.contains(STORE_APP));
        if store { &mut dlc } else { &mut requires }.push(id.clone());
    }
    let some = |text: &Option<String>| text.clone().filter(|t| !t.trim().is_empty());
    Some(workshopindex::Entry {
        id: workshop_id.into(),
        title: entry.steam_name.clone().or_else(|| entry.name.clone())?,
        author: some(&entry.authors),
        package_id: some(&entry.package_id),
        tags: entry
            .tags
            .iter()
            .flatten()
            .filter_map(|t| t.tag.clone())
            .collect(),
        requires,
        dlc,
    })
}

pub fn parse(json: impl std::io::Read) -> Result<(Names, Index), Error> {
    let all: Database = serde_json::from_reader(json)
        .map_err(|e| Error::format(said!("the database is not as expected: {}", e)))?;
    let apps = all
        .database
        .iter()
        .filter(|(id, entry)| entry.appid == Some(true) && id.as_str() != BASE_GAME)
        .filter_map(|(id, entry)| Some((id.clone(), entry.name.clone()?)))
        .collect();
    let index = Index::new(
        all.database
            .iter()
            .filter_map(|(id, entry)| indexed(id, entry))
            .collect(),
    )
    .with_apps(apps);
    let item = |(workshop_id, entry): (String, Entry)| {
        let name = entry.steam_name.or(entry.name)?;
        Some((
            crate::rimworld::id::fold(&entry.package_id?),
            Item { workshop_id, name },
        ))
    };
    Ok((all.database.into_iter().filter_map(item).collect(), index))
}

pub fn file_in(data_dir: &Path) -> PathBuf {
    data_dir.join("workshopNames.tsv")
}

pub fn store(names: &Names, to: &Path) -> Result<(), Error> {
    let mut lines: Vec<String> = names
        .iter()
        .map(|(id, item)| {
            format!(
                "{id}\t{}\t{}",
                item.workshop_id,
                item.name.replace(['\t', '\n'], " ")
            )
        })
        .collect();
    lines.sort();
    crate::base::atomic_file::write(to, lines.join("\n"))
}

pub fn read(path: &Path) -> Names {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let item = |line: &str| {
        let mut parts = line.splitn(3, '\t');
        let (id, workshop_id, name) = (parts.next()?, parts.next()?, parts.next()?);
        Some((
            id.to_string(),
            Item {
                workshop_id: workshop_id.into(),
                name: name.into(),
            },
        ))
    };
    text.lines().filter_map(item).collect()
}

pub fn download(to: &Path) -> Result<Names, Error> {
    let agent = crate::base::web::agent(300);
    let mut response = agent.get(URL).call().map_err(crate::base::web::failed)?;
    let (names, index) = parse(response.body_mut().with_config().limit(200 << 20).reader())?;
    store(&names, to)?;
    if let Some(dir) = to.parent() {
        index.store(&workshopindex::file_in(dir))?;
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"version": 1, "database": {
        "111": {"url": "u", "steamName": "Big Mod (Continued)", "name": "Big Mod", "packageId": "Author.BigMod", "authors": "Ann", "tags": [{"tag": "Mod", "display_name": "Mod"}, {"tag": "1.6"}], "dependencies": {"222": ["x", "https://steamcommunity.com/sharedfiles/filedetails/?id=222"], "1826140": ["Biotech", "https://store.steampowered.com/app/1826140/RimWorld__Biotech"]}},
        "222": {"name": "Only In About", "packageId": "a.b"},
        "333": {"steamName": "No id"},
        "444": {"steamName": "Gone", "packageId": "g.one", "unpublished": true},
        "294100": {"appid": true, "packageid": "ludeon.rimworld", "name": "RimWorld", "packageId": "ludeon.rimworld"},
        "1826140": {"appid": true, "name": "RimWorld - Biotech"}
    }}"#;

    #[test]
    fn mods_are_indexed_by_package_id_under_their_workshop_name() {
        let (names, _) = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(names.len(), 4);
        let big = &names["author.bigmod"];
        assert_eq!(
            (big.workshop_id.as_str(), big.name.as_str()),
            ("111", "Big Mod (Continued)")
        );
        assert_eq!(names["a.b"].name, "Only In About");
        assert!(parse("[]".as_bytes()).is_err());
    }

    #[test]
    fn the_index_holds_published_mods_with_dlc_kept_apart() {
        let (_, index) = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(index.len(), 3);
        assert!(index.get("294100").is_none() && index.get("444").is_none());
        assert_eq!(index.app_name("1826140"), Some("Biotech"));
        assert_eq!(index.app_name("294100"), None);
        let big = index.get("111").unwrap();
        assert_eq!(big.title, "Big Mod (Continued)");
        assert_eq!(big.author.as_deref(), Some("Ann"));
        assert_eq!(big.package_id.as_deref(), Some("Author.BigMod"));
        assert_eq!(big.tags, ["Mod", "1.6"]);
        assert_eq!(
            (big.requires.as_slice(), big.dlc.as_slice()),
            (
                ["222".to_string()].as_slice(),
                ["1826140".to_string()].as_slice()
            )
        );
        assert_eq!(index.get("222").unwrap().title, "Only In About");
        assert_eq!(index.get("333").unwrap().package_id, None);
    }

    #[test]
    fn the_names_read_back_as_stored() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        assert!(read(&path).is_empty());
        let mut names = parse(SAMPLE.as_bytes()).unwrap().0;
        names.get_mut("a.b").unwrap().name = "Tab\there".into();
        store(&names, &path).unwrap();
        names.get_mut("a.b").unwrap().name = "Tab here".into();
        assert_eq!(read(&path), names);
    }

    #[test]
    #[ignore = "needs RIMSTACK_STEAMDB: a downloaded steamDB.json"]
    fn the_real_database_is_read() {
        let file = std::fs::File::open(std::env::var("RIMSTACK_STEAMDB").unwrap()).unwrap();
        let started = std::time::Instant::now();
        let (names, index) = parse(std::io::BufReader::new(file)).unwrap();
        println!(
            "{} names, {} indexed in {:?}",
            names.len(),
            index.len(),
            started.elapsed()
        );
        assert!(names.len() > 10_000);
        let file = std::env::temp_dir().join("rimstack-index-measure.jsonl");
        index.store(&file).unwrap();
        println!(
            "index file {} bytes",
            std::fs::metadata(&file).unwrap().len()
        );
        let started = std::time::Instant::now();
        let back = Index::read(&file);
        println!("{} read back in {:?}", back.len(), started.elapsed());
        for words in ["harmony", "a"] {
            let query = super::super::workshopsearch::Query::parse(words);
            let started = std::time::Instant::now();
            let page = back.search(&query, &Default::default(), Default::default(), 0, 30);
            println!("{words:?}: {} hits in {:?}", page.total, started.elapsed());
        }
        let _ = std::fs::remove_file(&file);
    }
}
