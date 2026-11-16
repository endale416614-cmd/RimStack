//! Nothing here sends anything, and the key is never formatted into anything.

use super::workshop::{self, Details, number};
use crate::base::error::Error;
use crate::said;
use serde_json::{Value, json};

pub const RIMWORLD_APP: u64 = 294_100;

/// SteamKit's `EContentDescriptorID` for sexual content: 1 some nudity or sexual content, 3 adult
/// only sexual content, 4 frequent nudity or sexual content. Not yet confirmed against a live answer.
pub const SEXUAL_DESCRIPTORS: [u32; 3] = [1, 3, 4];

/// Prints as `***`; it has no `Serialize` and no `Clone` outside this crate's transport.
pub struct ApiKey(String);

impl ApiKey {
    /// `None` for a blank text.
    pub fn new(text: &str) -> Option<ApiKey> {
        let text = text.trim();
        (!text.is_empty()).then(|| ApiKey(text.to_string()))
    }

    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(***)")
    }
}

impl std::fmt::Display for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    QueryFiles,
    GetDetails,
    GetUserFiles,
}

impl Call {
    pub fn name(self) -> &'static str {
        match self {
            Call::QueryFiles => "QueryFiles",
            Call::GetDetails => "GetDetails",
            Call::GetUserFiles => "GetUserFiles",
        }
    }

    pub fn url(self) -> String {
        format!(
            "https://api.steampowered.com/IPublishedFileService/{}/v1/",
            self.name()
        )
    }
}

/// EPublishedFileQueryType.
pub mod query_type {
    pub const BY_VOTE: u32 = 0;
    pub const NEWEST: u32 = 1;
    pub const TREND: u32 = 3;
    pub const MOST_SUBSCRIBED: u32 = 9;
    pub const TEXT: u32 = 12;
    pub const LAST_UPDATED: u32 = 21;
}

/// EPublishedFileInfoMatchingFileType: a collection.
const COLLECTIONS: u32 = 1;

#[derive(Debug, Clone, Copy, Default)]
pub struct Search<'a> {
    pub query_type: u32,
    /// `*` for the first page, then what the page before gave.
    pub cursor: &'a str,
    pub per_page: usize,
    pub text: &'a str,
    pub required_tags: &'a [String],
    pub collections: bool,
    /// Only for the trend, 1 to 7.
    pub days: Option<u8>,
    pub excluded_descriptors: &'a [u32],
}

fn with_details(mut args: Value) -> Value {
    for flag in [
        "return_details",
        "return_tags",
        "return_vote_data",
        "return_previews",
        "return_children",
        "return_short_description",
    ] {
        args[flag] = json!(true);
    }
    args
}

pub fn query_files(search: &Search) -> Value {
    let mut args = with_details(json!({
        "query_type": search.query_type,
        "cursor": search.cursor,
        "numperpage": search.per_page,
        "appid": RIMWORLD_APP,
        "creator_appid": RIMWORLD_APP,
    }));
    if !search.text.is_empty() {
        args["search_text"] = json!(search.text);
    }
    if !search.required_tags.is_empty() {
        args["requiredtags"] = json!(search.required_tags);
        args["match_all_tags"] = json!(true);
    }
    if search.collections {
        args["filetype"] = json!(COLLECTIONS);
    }
    if let Some(days) = search.days {
        args["days"] = json!(days);
    }
    if !search.excluded_descriptors.is_empty() {
        args["excluded_content_descriptors"] = json!(search.excluded_descriptors);
    }
    args
}

pub fn get_details(ids: &[String]) -> Value {
    json!({
        "publishedfileids": ids,
        "includetags": true,
        "includeadditionalpreviews": true,
        "includechildren": true,
        "includevotes": true,
    })
}

pub fn user_files(steam_id: &str, page: usize, per_page: usize, excluded: &[u32]) -> Value {
    let mut args = with_details(json!({
        "steamid": steam_id,
        "appid": RIMWORLD_APP,
        "creator_appid": RIMWORLD_APP,
        "page": page,
        "numperpage": per_page,
    }));
    if !excluded.is_empty() {
        args["excluded_content_descriptors"] = json!(excluded);
    }
    args
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub total: usize,
    /// Absent on the last page, and on a call that pages by number.
    pub next_cursor: Option<String>,
    pub files: Vec<Details>,
}

fn parse_found(json: &str) -> Result<Found, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let response = root
        .get("response")
        .filter(|r| r.is_object())
        .ok_or_else(|| Error::format(said!("Steam's answer has no file list")))?;
    // A page with nothing leaves the list out.
    let files: Vec<Details> = response
        .get("publishedfiledetails")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |list| {
            list.iter().filter_map(workshop::details_of).collect()
        });
    Ok(Found {
        total: response
            .get("total")
            .and_then(number)
            .map_or(files.len(), |t| t as usize),
        next_cursor: response
            .get("next_cursor")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .map(String::from),
        files,
    })
}

pub fn parse_query(json: &str) -> Result<Found, Error> {
    parse_found(json)
}

pub fn parse_user_files(json: &str) -> Result<Found, Error> {
    parse_found(json)
}

pub fn parse_get_details(json: &str) -> Result<Vec<Details>, Error> {
    workshop::parse_details(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUERY: &str = r#"{"response":{"total":2,"publishedfiledetails":[
      {"result":1,"publishedfileid":"1111111111","creator":"76561198000000001","creator_appid":294100,"consumer_appid":294100,
       "file_size":"1048576","preview_url":"https://x/p","title":"Example Mod","file_description":"Adds [b]x[/b]",
       "time_updated":1700000000,"banned":false,"subscriptions":12345,"favorited":900,"num_children":1,"file_type":0,
       "content_descriptorids":[5],"tags":[{"tag":"Mod","display_name":"Mod"},{"tag":"1.5"}],
       "vote_data":{"score":0.93,"votes_up":800,"votes_down":12},
       "previews":[{"previewid":"5001","url":"https://x/s1","preview_type":0},{"url":"https://youtu.be/x","preview_type":1,"youtubevideoid":"abc"},{"url":"https://x/s2"}],
       "children":[{"publishedfileid":"2009463077","sortorder":0,"file_type":0}]},
      {"result":1,"publishedfileid":"3333333333","title":"Set","time_updated":"1710000000","file_type":2,
       "subscriptions":"500","vote_data":{"score":0.7,"votes_up":"30","votes_down":5},"previews":[],
       "children":[{"publishedfileid":"1111111111"},{"publishedfileid":4444444444}]}],
      "next_cursor":"AoIIPwAAAAB5mfQy"}}"#;

    #[test]
    fn a_query_answer_carries_the_votes_images_children_and_cursor() {
        let found = parse_query(QUERY).unwrap();
        assert_eq!((found.total, found.files.len()), (2, 2));
        assert_eq!(found.next_cursor.as_deref(), Some("AoIIPwAAAAB5mfQy"));
        let one = &found.files[0];
        assert_eq!(
            (one.id.as_str(), one.size, one.app_id),
            ("1111111111", 1_048_576, 294_100)
        );
        assert_eq!(one.description, "Adds [b]x[/b]");
        assert_eq!(one.votes.map(|v| (v.up, v.down)), Some((800, 12)));
        assert!((one.votes.unwrap().score - 0.93).abs() < 1e-6);
        assert_eq!(one.screenshots, ["https://x/s1", "https://x/s2"]);
        assert_eq!(
            (one.content_descriptors.as_slice(), one.tags.as_slice()),
            (&[5][..], &["Mod".to_string(), "1.5".to_string()][..])
        );
        assert_eq!(one.children, ["2009463077"]);
        assert!(!one.collection, "a mod's children are what it requires");
    }

    #[test]
    fn numbers_may_be_text_and_a_collection_is_known_by_its_type() {
        let set = &parse_query(QUERY).unwrap().files[1];
        assert_eq!((set.updated, set.subscriptions), (1_710_000_000, 500));
        assert_eq!(set.votes.map(|v| v.up), Some(30));
        assert!(set.collection);
        assert_eq!(set.children, ["1111111111", "4444444444"]);
        assert!(set.screenshots.is_empty() && set.content_descriptors.is_empty());
    }

    #[test]
    fn what_is_missing_is_left_empty_and_an_unknown_item_is_skipped() {
        let json = r#"{"response":{"publishedfiledetails":[
            {"publishedfileid":"7","title":"Bare"},{"publishedfileid":"8","result":9},{"title":"No id"}]}}"#;
        let found = parse_get_details(json).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].votes.is_none() && found[0].children.is_empty() && !found[0].collection);
        let none = parse_query(r#"{"response":{"total":0}}"#).unwrap();
        assert_eq!(
            (none.total, none.files.len(), none.next_cursor),
            (0, 0, None)
        );
        let last = parse_user_files(r#"{"response":{"total":1,"next_cursor":"","publishedfiledetails":[{"publishedfileid":"7","title":"T"}]}}"#).unwrap();
        assert_eq!((last.total, last.next_cursor), (1, None));
        assert!(parse_query("<html>").is_err() && parse_query("{}").is_err());
    }

    #[test]
    fn the_arguments_hold_what_the_search_asks_for() {
        let tags = vec!["Mod".to_string()];
        let args = query_files(&Search {
            query_type: query_type::TREND,
            cursor: "*",
            per_page: 20,
            text: "storage",
            required_tags: &tags,
            collections: true,
            days: Some(7),
            excluded_descriptors: &SEXUAL_DESCRIPTORS,
        });
        assert_eq!(args["query_type"], 3);
        assert_eq!(args["requiredtags"], json!(["Mod"]));
        assert_eq!(args["filetype"], 1);
        assert_eq!(args["days"], 7);
        assert_eq!(args["excluded_content_descriptors"], json!([1, 3, 4]));
        assert_eq!(args["return_vote_data"], true);
        let bare = query_files(&Search {
            cursor: "*",
            per_page: 1,
            ..Search::default()
        });
        for absent in [
            "search_text",
            "requiredtags",
            "filetype",
            "days",
            "excluded_content_descriptors",
        ] {
            assert!(bare.get(absent).is_none(), "{absent}");
        }
        assert_eq!(get_details(&["5".into()])["publishedfileids"], json!(["5"]));
        assert_eq!(
            user_files("76561198000000001", 1, 100, &[])["steamid"],
            "76561198000000001"
        );
    }

    #[test]
    fn a_key_prints_as_stars() {
        let key = ApiKey::new("  SECRETKEY123 \n").unwrap();
        assert_eq!(key.reveal(), "SECRETKEY123");
        for shown in [
            format!("{key}"),
            format!("{key:?}"),
            format!("{key:#?}"),
            format!("{:?}", Some(&key)),
        ] {
            assert!(!shown.contains("SECRET"), "{shown}");
        }
        assert!(ApiKey::new("   ").is_none());
    }
}
