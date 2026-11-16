use crate::base::error::Error;
use crate::base::sentence::Sentence;
use crate::rimworld::paths::APP_ID;
use crate::{said, say};

use crate::base::vdf::{Token, tokens};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

/// `timeupdated` in `appworkshop_294100.acf`: seconds since 1970, by Workshop id.
pub fn installed_times(acf: &str) -> HashMap<String, u64> {
    installed(acf, "timeupdated")
}

pub fn installed_sizes(acf: &str) -> HashMap<String, u64> {
    installed(acf, "size")
}

/// One upload of an item's files, as Steam numbers it: `manifest` is the `hcontent_file` its
/// details give and the `manifest` an `appworkshop_294100.acf` records, 0 where neither says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Version {
    pub manifest: u64,
    /// Seconds since 1970.
    pub updated: u64,
}

impl Version {
    /// By manifest where both are known, by time otherwise; a copy newer than the last answer
    /// is not behind it.
    pub fn behind(self, now: Version) -> bool {
        match (self.manifest, now.manifest) {
            (0, _) | (_, 0) => now.updated > self.updated,
            (have, offered) => have != offered && now.updated >= self.updated,
        }
    }
}

pub fn installed_manifests(acf: &str) -> HashMap<String, u64> {
    installed(acf, "manifest")
}

pub fn installed_versions(acf: &str) -> HashMap<String, Version> {
    let manifests = installed_manifests(acf);
    let version = |(id, updated): (String, u64)| {
        let manifest = manifests.get(&id).copied().unwrap_or(0);
        (id, Version { manifest, updated })
    };
    installed_times(acf).into_iter().map(version).collect()
}

fn installed(acf: &str, fact: &str) -> HashMap<String, u64> {
    let all = tokens(acf);
    let mut times = HashMap::new();
    let start = all
        .iter()
        .position(|t| matches!(t, Token::Text(s) if s == "WorkshopItemsInstalled"));
    let Some(start) = start else {
        return times;
    };
    // "<id>" { "key" "value" ... } repeated, inside one pair of braces.
    let mut depth = 0;
    let mut item: Option<&str> = None;
    let mut rest = all[start + 1..].iter();
    while let Some(token) = rest.next() {
        match token {
            Token::Open => depth += 1,
            Token::Close => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Token::Text(text) if depth == 1 => item = Some(text),
            Token::Text(key) => {
                let value = rest.next();
                if let (2, true, Some(id), Some(Token::Text(v))) = (depth, key == fact, item, value)
                    && let Ok(time) = v.parse()
                {
                    times.insert(id.to_string(), time);
                }
            }
        }
    }
    times
}

/// Steam reads each id up to its first non-digit and refuses the whole request when one comes
/// out as 0: one folder named `Backup` in the Workshop folder would sink every other answer.
pub fn is_id(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_digit()) && text.parse::<u64>().is_ok_and(|n| n != 0)
}

pub fn ids_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for piece in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let digits = |s: &str| -> String { s.chars().take_while(char::is_ascii_digit).collect() };
        let id = match piece.split_once("id=") {
            Some((_, after)) => digits(after),
            None if piece.chars().all(|c| c.is_ascii_digit()) => piece.to_string(),
            None => String::new(),
        };
        if (6..=12).contains(&id.len()) && is_id(&id) && !found.contains(&id) {
            found.push(id);
        }
    }
    found
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Details {
    pub id: String,
    pub title: String,
    /// Seconds since 1970.
    pub updated: u64,
    /// Its `hcontent_file`; 0 when Steam does not say.
    pub manifest: u64,
    /// Seconds since 1970; 0 when Steam does not say.
    pub created: u64,
    /// 0 when Steam does not say.
    pub size: u64,
    pub preview_url: String,
    pub description: String,
    pub subscriptions: u64,
    pub favorited: u64,
    /// Steam's unique visitors; 0 when it does not say.
    pub views: u64,
    pub banned: bool,
    /// A Steam id, not a name.
    pub creator: String,
    /// The game the item is for; 0 when Steam does not say.
    pub app_id: u64,
    pub collection: bool,
    /// Only where the key was used.
    pub votes: Option<Votes>,
    /// Image addresses of the extra previews, in Steam's order; only where the key was used.
    pub screenshots: Vec<String>,
    pub content_descriptors: Vec<u32>,
    /// A collection's items, or a mod's required items; only where the key was used.
    pub children: Vec<String>,
    pub tags: Vec<String>,
}

impl Details {
    pub fn refusal(&self) -> Option<Sentence> {
        if self.collection {
            Some(say!("A collection: add its mods instead"))
        } else if self.app_id.to_string() != APP_ID {
            Some(say!("Not an item of RimWorld"))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Votes {
    pub up: u64,
    pub down: u64,
    /// Steam's own, 0 to 1.
    pub score: f32,
}

const COLLECTION_FILE_TYPE: u64 = 2;

/// Steam writes 64-bit values as text and 32-bit ones as numbers, and the keyless call writes
/// everything as text.
pub(crate) fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn texts(file: &Value, key: &str, inner: &str) -> Vec<String> {
    let list = file.get(key).and_then(Value::as_array);
    let one = |item: &Value| item.get(inner).and_then(id_text);
    list.map_or_else(Vec::new, |list| list.iter().filter_map(one).collect())
}

/// An item Steam does not know has a `result` other than 1 and nothing else of use.
pub(crate) fn details_of(file: &Value) -> Option<Details> {
    if file.get("result").and_then(number).is_some_and(|r| r != 1) {
        return None;
    }
    let text = |key: &str| {
        file.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let count = |key: &str| file.get(key).and_then(number).unwrap_or(0);
    let children = texts(file, "children", "publishedfileid");
    let description = match text("file_description") {
        d if d.is_empty() => text("description"),
        d => d,
    };
    let votes = file.get("vote_data").map(|v| Votes {
        up: v.get("votes_up").and_then(number).unwrap_or(0),
        down: v.get("votes_down").and_then(number).unwrap_or(0),
        score: v.get("score").and_then(Value::as_f64).unwrap_or(0.0) as f32,
    });
    // `preview_type` 0 is an image; the others are videos and models.
    let image = |p: &Value| {
        p.get("preview_type").and_then(number).unwrap_or(0) == 0
            && p.get("youtubevideoid")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
    };
    let screenshots =
        file.get("previews")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |list| {
                let url = |p: &Value| Some(p.get("url")?.as_str()?.to_string());
                list.iter().filter(|p| image(p)).filter_map(url).collect()
            });
    let descriptors = file
        .get("content_descriptorids")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |list| {
            let one = |v: &Value| number(v).and_then(|n| u32::try_from(n).ok());
            list.iter().filter_map(one).collect()
        });
    Some(Details {
        id: id_text(file.get("publishedfileid")?)?,
        title: file.get("title")?.as_str()?.to_string(),
        updated: count("time_updated"),
        manifest: count("hcontent_file"),
        created: count("time_created"),
        size: count("file_size"),
        preview_url: text("preview_url"),
        description,
        subscriptions: count("subscriptions"),
        favorited: count("favorited"),
        views: count("views"),
        banned: file.get("banned").is_some_and(|b| {
            b.as_bool()
                .unwrap_or_else(|| number(b).is_some_and(|n| n != 0))
        }),
        creator: file.get("creator").and_then(id_text).unwrap_or_default(),
        app_id: count("consumer_app_id").max(count("consumer_appid")),
        collection: count("file_type") == COLLECTION_FILE_TYPE,
        votes,
        screenshots,
        content_descriptors: descriptors,
        children,
        tags: texts(file, "tags", "tag"),
    })
}

pub fn parse_details(json: &str) -> Result<Vec<Details>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let files = root
        .pointer("/response/publishedfiledetails")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::format(said!("Steam's answer has no file list")))?;
    Ok(files.iter().filter_map(details_of).collect())
}

/// One POST of up to 100 ids; those that are not ids are left out. Blocks on the network.
pub fn fetch_batch(agent: &ureq::Agent, batch: &[String]) -> Result<Vec<Details>, Error> {
    let batch: Vec<&String> = batch.iter().filter(|id| is_id(id)).collect();
    if batch.is_empty() {
        return Ok(Vec::new());
    }
    let mut form = vec![("itemcount".to_string(), batch.len().to_string())];
    for (i, id) in batch.iter().enumerate() {
        form.push((format!("publishedfileids[{i}]"), id.to_string()));
    }
    let pairs = form.iter().map(|(k, v)| (k.as_str(), v.as_str()));
    let mut response = agent
        .post(DETAILS_URL)
        .send_form(pairs)
        .map_err(crate::base::web::steam_failed)?;
    // Descriptions come along, so this is above the default limit.
    let text = crate::base::web::read_text(&mut response, 64 * 1024 * 1024)
        .map_err(crate::base::web::steam_failed)?;
    parse_details(&text)
}

/// Blocks on the network. Steam is slow per item, so chunks of 100 go out in parallel.
pub fn fetch_details(ids: &[String]) -> Result<Vec<Details>, Error> {
    let agent = crate::base::web::steam_agent();
    let answers: Vec<Result<Vec<Details>, Error>> = ids
        .par_chunks(100)
        .map(|batch| fetch_batch(&agent, batch))
        .collect();
    let mut all = Vec::new();
    for answer in answers {
        all.extend(answer?);
    }
    Ok(all)
}

const COLLECTIONS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/";

pub fn parse_collections(json: &str) -> Result<Vec<(String, Vec<String>)>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let all = root["response"]["collectiondetails"].as_array();
    let text = |v: &Value| v["publishedfileid"].as_str().map(String::from);
    let one = |c: &Value| {
        let children = c["children"].as_array();
        Some((
            text(c)?,
            children.map_or(Vec::new(), |list| list.iter().filter_map(text).collect()),
        ))
    };
    Ok(all
        .ok_or_else(|| Error::format(said!("Steam's answer has no collections")))?
        .iter()
        .filter_map(one)
        .collect())
}

/// Blocks on the network.
pub fn expand_collections(ids: &[String]) -> Result<Vec<String>, Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let agent = crate::base::web::agent(60);
    let mut form = vec![("collectioncount".to_string(), ids.len().to_string())];
    for (i, id) in ids.iter().enumerate() {
        form.push((format!("publishedfileids[{i}]"), id.clone()));
    }
    let failed = crate::base::web::steam_failed;
    let pairs = form.iter().map(|(k, v)| (k.as_str(), v.as_str()));
    let mut response = agent
        .post(COLLECTIONS_URL)
        .send_form(pairs)
        .map_err(failed)?;
    let text = response.body_mut().read_to_string().map_err(failed)?;
    let collections = parse_collections(&text)?;
    let mut all: Vec<String> = Vec::new();
    for id in ids {
        let items = collections
            .iter()
            .find(|(c, items)| c == id && !items.is_empty());
        let these = items.map_or(std::slice::from_ref(id), |(_, items)| items.as_slice());
        for item in these {
            if !all.contains(item) {
                all.push(item.clone());
            }
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = r#""AppWorkshop"
{
	"appid"		"294100"
	"TimeLastUpdated"		"1791750395"
	"WorkshopItemsInstalled"
	{
		"705924057"
		{
			"size"		"1372735"
			"timeupdated"		"1711286985"
			"manifest"		"7900175110102980905"
		}
		"712141500"
		{
			"size"		"5310730"
			"timeupdated"		"1710519878"
		}
	}
	"WorkshopItemDetails"
	{
		"705924057"
		{
			"timeupdated"		"5"
		}
	}
}"#;

    #[test]
    fn installed_times_come_from_the_installed_block_only() {
        let times = installed_times(ACF);
        assert_eq!(times.len(), 2);
        assert_eq!(times["705924057"], 1711286985);
        assert_eq!(times["712141500"], 1710519878);
        assert!(installed_times("garbage {").is_empty());
        assert_eq!(installed_sizes(ACF)["705924057"], 1372735);
    }

    #[test]
    fn a_version_is_told_by_its_manifest_and_by_its_time_where_one_is_missing() {
        let versions = installed_versions(ACF);
        let (known, unknown) = (versions["705924057"], versions["712141500"]);
        assert_eq!(known.manifest, 7900175110102980905);
        assert_eq!((unknown.manifest, unknown.updated), (0, 1710519878));
        let at = |manifest, updated| Version { manifest, updated };
        assert!(at(1, 10).behind(at(2, 10)), "a new upload, the same time");
        assert!(!at(1, 10).behind(at(1, 20)), "the same upload, a new time");
        assert!(at(0, 10).behind(at(2, 20)));
        assert!(!at(1, 20).behind(at(0, 10)));
        assert!(!at(2, 20).behind(at(1, 10)), "updated since the last check");
    }

    #[test]
    fn ids_are_taken_from_links_and_bare_numbers_once_each() {
        let pasted = "https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077&searchtext=x\n\
                      818773962, 2009463077 hello 12 steam://url/CommunityFilePage/1 ?id=abc";
        assert_eq!(ids_in(pasted), ["2009463077", "818773962"]);
    }

    #[test]
    fn only_what_steam_reads_as_an_id_is_one() {
        for id in ["2009463077", "0002009463077"] {
            assert!(is_id(id), "{id}");
        }
        let not = [
            "",
            "0",
            "000",
            "Backup",
            "\u{feff}2009463077",
            "+5",
            "2009463077 copy",
        ];
        for text in not {
            assert!(!is_id(text), "{text:?}");
        }
        assert!(ids_in("000000 https://x/?id=0000000").is_empty());
    }

    #[test]
    fn a_batch_of_folder_names_asks_steam_nothing() {
        let names = ["Backup".to_string(), "0".to_string(), String::new()];
        let offline = ureq::Agent::config_builder()
            .proxy(ureq::Proxy::new("http://127.0.0.1:9").ok())
            .build()
            .into();
        assert_eq!(fetch_batch(&offline, &names).unwrap(), []);
    }

    #[test]
    fn details_carry_what_the_browser_shows() {
        let json = r#"{"response":{"publishedfiledetails":[
            {"publishedfileid":"7","result":1,"creator":"76561198000000000","file_size":"12","preview_url":"https://x/p.jpg","title":"T","description":"[b]hi[/b]","time_updated":5,"hcontent_file":"88","banned":1,"subscriptions":300,"favorited":4,"tags":[{"tag":"Mod"}]}]}}"#;
        let d = &parse_details(json).unwrap()[0];
        assert_eq!(
            (
                d.preview_url.as_str(),
                d.description.as_str(),
                d.creator.as_str()
            ),
            ("https://x/p.jpg", "[b]hi[/b]", "76561198000000000")
        );
        assert_eq!(
            (d.subscriptions, d.favorited, d.banned, d.size),
            (300, 4, true, 12)
        );
        assert_eq!((d.manifest, d.updated), (88, 5));
    }

    #[test]
    fn details_skip_items_steam_does_not_know() {
        let json = r#"{"response":{"result":1,"resultcount":2,"publishedfiledetails":[
            {"publishedfileid":"2009463077","result":1,"title":"Harmony","time_updated":1756914561,"file_size":"5729526","consumer_app_id":294100,"file_type":2},
            {"publishedfileid":"1","result":9}]}}"#;
        let found = parse_details(json).unwrap();
        assert_eq!(
            found,
            [Details {
                id: "2009463077".into(),
                title: "Harmony".into(),
                updated: 1756914561,
                size: 5729526,
                preview_url: String::new(),
                description: String::new(),
                subscriptions: 0,
                favorited: 0,
                banned: false,
                creator: String::new(),
                app_id: 294100,
                collection: true,
                ..Details::default()
            }]
        );
        assert!(parse_details("<html>").is_err());
        assert!(parse_details("{}").is_err());
    }

    #[test]
    #[ignore = "needs the network and RIMSTACK_WORKSHOP: .../steamapps/workshop/content/294100"]
    fn real_workshop_folder_is_checked_for_updates() {
        let content = std::path::PathBuf::from(std::env::var_os("RIMSTACK_WORKSHOP").unwrap());
        let record = content
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("appworkshop_294100.acf");
        let on_disk = installed_times(&std::fs::read_to_string(record).unwrap());
        let ids: Vec<String> = std::fs::read_dir(&content)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let started = std::time::Instant::now();
        let now = fetch_details(&ids).unwrap();
        println!(
            "asked {} answered {} in {:?}",
            ids.len(),
            now.len(),
            started.elapsed()
        );
        let newer = |d: &&Details| on_disk.get(&d.id).is_some_and(|&have| d.updated > have);
        let behind: Vec<&Details> = now.iter().filter(newer).collect();
        println!("{} on record, {} behind", on_disk.len(), behind.len());
        for d in behind.iter().take(8) {
            println!("  {} have {} now {}", d.title, on_disk[&d.id], d.updated);
        }
        assert!(now.len() > ids.len() / 2);
    }

    #[test]
    #[ignore = "needs the network"]
    fn steam_answers_about_a_real_item() {
        let found = fetch_details(&["2009463077".to_string(), "1".to_string()]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Harmony");
    }

    #[test]
    fn a_collection_gives_its_items_and_a_mod_none() {
        let answer = r#"{"response":{"result":1,"resultcount":2,"collectiondetails":[
            {"publishedfileid":"111","result":1,"children":[{"publishedfileid":"5","sortorder":1,"filetype":0},{"publishedfileid":"6","sortorder":2,"filetype":0}]},
            {"publishedfileid":"222","result":9}]}}"#;
        let found = parse_collections(answer).unwrap();
        assert_eq!(
            found[0],
            ("111".to_string(), vec!["5".to_string(), "6".to_string()])
        );
        assert_eq!(found[1], ("222".to_string(), vec![]));
        assert!(parse_collections("{}").is_err());
    }
}
