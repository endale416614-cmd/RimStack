use super::{Failure, Transport};
use crate::rimworld::steamapi::{ApiKey, Call, query_type};
use crate::rimworld::workshopindex::{Entry, Index};
use crate::sources::browse::fixture::{self, COLLECTION};
use serde_json::{Value, json};

pub(super) fn index() -> Index {
    fixture::index()
}

pub struct Canned;

struct Item {
    id: String,
    title: String,
    tags: Vec<String>,
    creator: &'static str,
    children: Vec<String>,
    collection: bool,
    n: u64,
}

impl Item {
    fn updated(&self) -> u64 {
        1_760_000_000 + (self.n * 5 % 7) * 1000
    }

    fn subscriptions(&self) -> u64 {
        1000 + (self.n * 3 % 7) * 1000
    }

    fn up(&self) -> u64 {
        100 + (self.n * 11 % 13) * 10
    }

    fn down(&self) -> u64 {
        10 + (self.n * 7 % 5) * 20
    }

    fn score(&self) -> f64 {
        self.up() as f64 / (self.up() + self.down()) as f64
    }

    fn json(&self) -> Value {
        json!({
            "result": 1,
            "publishedfileid": self.id,
            "creator": self.creator,
            "consumer_appid": 294_100,
            "file_size": (1_048_576 + self.n).to_string(),
            "preview_url": "",
            "title": self.title,
            "file_description": format!("[b]{}[/b]\nA made-up item.", self.title),
            "short_description": "A made-up item.",
            "time_updated": self.updated(),
            "banned": false,
            "subscriptions": self.subscriptions(),
            "favorited": 100 + self.n,
            "file_type": if self.collection { 2 } else { 0 },
            "tags": self.tags.iter().map(|t| json!({"tag": t})).collect::<Vec<_>>(),
            "vote_data": {"score": self.score(), "votes_up": self.up(), "votes_down": self.down()},
            "previews": (1..=3).map(|k| json!({"url": format!("https://shared.fastly.steamstatic.com/fixture/{}/{k}.jpg", self.id), "preview_type": 0})).collect::<Vec<_>>(),
            "children": self.children.iter().map(|c| json!({"publishedfileid": c})).collect::<Vec<_>>(),
        })
    }
}

fn creator(author: Option<&str>) -> &'static str {
    match author {
        Some("Fixture Smith") => "76561198000000001",
        Some("Fixture Pardeike") => "76561198000000002",
        Some("Fixture Linguist") => "76561198000000003",
        _ => "76561198000000000",
    }
}

fn items() -> Vec<Item> {
    let of = |e: Entry| Item {
        n: e.id.parse::<u64>().unwrap_or(0) % 1000,
        creator: creator(e.author.as_deref()),
        children: e.requires,
        id: e.id,
        title: e.title,
        tags: e.tags,
        collection: false,
    };
    let mut all: Vec<Item> = fixture::entries().into_iter().map(of).collect();
    all.push(Item {
        id: COLLECTION.into(),
        title: "Fixture Collection".into(),
        tags: Vec::new(),
        creator: creator(Some("Fixture Smith")),
        children: vec!["3100000001".into(), "3100000003".into()],
        collection: true,
        n: 7,
    });
    all
}

fn strings(args: &Value, key: &str) -> Vec<String> {
    let list = args.get(key).and_then(Value::as_array);
    let text = |v: &Value| Some(v.as_str()?.to_string());
    list.map_or_else(Vec::new, |l| l.iter().filter_map(text).collect())
}

fn matches(item: &Item, args: &Value) -> bool {
    let wanted = args["filetype"] == 1;
    let text = args["search_text"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    let title = item.title.to_lowercase();
    let tagged = strings(args, "requiredtags")
        .iter()
        .all(|t| item.tags.iter().any(|have| have.eq_ignore_ascii_case(t)));
    item.collection == wanted && tagged && text.split_whitespace().all(|w| title.contains(w))
}

fn ordered(items: &mut [Item], kind: u32) {
    use std::cmp::Reverse;
    match kind {
        query_type::NEWEST => items.sort_by_key(|i| Reverse(i.id.parse::<u64>().unwrap_or(0))),
        query_type::TREND => items.sort_by_key(|i| Reverse(i.up())),
        query_type::MOST_SUBSCRIBED => items.sort_by_key(|i| Reverse(i.subscriptions())),
        query_type::LAST_UPDATED => items.sort_by_key(|i| Reverse(i.updated())),
        query_type::BY_VOTE => items.sort_by(|a, b| b.score().total_cmp(&a.score())),
        _ => items.sort_by_key(|i| i.n),
    }
}

fn page(mut found: Vec<Item>, args: &Value) -> Value {
    ordered(
        &mut found,
        args["query_type"]
            .as_u64()
            .map_or(query_type::TEXT, |k| k as u32),
    );
    let total = found.len();
    let start = args["cursor"]
        .as_str()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(0);
    let size = args["numperpage"].as_u64().unwrap_or(1) as usize;
    let shown: Vec<Value> = found
        .iter()
        .skip(start)
        .take(size)
        .map(Item::json)
        .collect();
    let mut response = json!({"total": total, "publishedfiledetails": shown});
    if start + size < total {
        response["next_cursor"] = json!((start + size).to_string());
    }
    json!({"response": response})
}

impl Transport for Canned {
    fn call(&self, call: Call, args: &Value, _: &ApiKey) -> Result<String, Failure> {
        let all = items();
        let answer = match call {
            Call::QueryFiles => page(all.into_iter().filter(|i| matches(i, args)).collect(), args),
            Call::GetDetails => {
                let ids = strings(args, "publishedfileids");
                let found: Vec<Value> = all
                    .iter()
                    .filter(|i| ids.contains(&i.id))
                    .map(Item::json)
                    .collect();
                json!({"response": {"publishedfiledetails": found}})
            }
            Call::GetUserFiles => {
                let who = args["steamid"].as_str().unwrap_or_default();
                let mut mine: Vec<Item> = all.into_iter().filter(|i| i.creator == who).collect();
                mine.sort_by_key(|i| std::cmp::Reverse(i.updated()));
                let total = mine.len();
                let shown: Vec<Value> = mine.iter().map(Item::json).collect();
                json!({"response": {"total": total, "publishedfiledetails": shown}})
            }
        };
        Ok(answer.to_string())
    }
}
