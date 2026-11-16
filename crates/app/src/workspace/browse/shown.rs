use super::sidebar::Sidebar;
use rimstack_core::base::markup::{Block, Piece, pieces};
use rimstack_core::base::{elapsed::ago, sentence::Sentence, sizes};
use rimstack_core::rimworld::workshop::{Details, Votes};
use rimstack_core::rimworld::workshopindex::Entry;
use rimstack_core::sources::browse::Browse;
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub struct Marks {
    pub installed: HashSet<String>,
    pub listed: HashSet<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub versions: Vec<String>,
    pub installed: bool,
    pub on_list: bool,
    pub badge: Option<Sentence>,
    pub can_add: bool,
    pub stars: Vec<bool>,
}

/// Steam shows its score of 0 to 1 as 0 to 5 stars.
pub fn stars_of(votes: Option<Votes>) -> Vec<bool> {
    let Some(votes) = votes.filter(|v| v.up + v.down > 0) else {
        return Vec::new();
    };
    let filled = (votes.score.clamp(0.0, 1.0) * 5.0).round() as usize;
    (0..5).map(|at| at < filled).collect()
}

impl Marks {
    pub fn row(&self, entry: &Entry) -> Row {
        let mut row = Row {
            id: entry.id.clone(),
            title: entry.title.clone(),
            author: entry.author.clone(),
            versions: entry
                .tags
                .iter()
                .filter(|tag| tag.starts_with(|c: char| c.is_ascii_digit()))
                .cloned()
                .collect(),
            installed: false,
            on_list: false,
            badge: None,
            can_add: true,
            stars: Vec::new(),
        };
        self.mark(&mut row);
        row
    }

    pub fn mark(&self, row: &mut Row) {
        row.installed = self.installed.contains(&row.id);
        row.on_list = self.listed.contains(&row.id);
        row.badge = match (row.installed, row.on_list) {
            (true, _) => Some(say!("Installed")),
            (false, true) => Some(say!("On the list")),
            _ => None,
        };
        row.can_add = !row.installed && !row.on_list;
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub rows: Vec<Row>,
    pub total: usize,
    pub more: bool,
    pub notice: Option<Sentence>,
    /// Only for the first page of a search.
    #[serde(skip)]
    pub sidebar: Option<Sidebar>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dlc {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub label: Sentence,
    pub value: Sentence,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(skip)]
    pub blocks: Vec<Block>,
    pub image: String,
    pub note: Option<Sentence>,
}

pub const PICTURES: usize = 12;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    #[serde(flatten)]
    pub row: Row,
    /// False when Steam could not be asked: only what the database holds is filled.
    pub loaded: bool,
    pub problem: Option<Sentence>,
    #[serde(skip)]
    pub pieces: Vec<Piece>,
    pub created_by: Option<Sentence>,
    pub stats: Vec<Stat>,
    pub created_said: Option<Sentence>,
    pub size: u64,
    pub size_said: Option<Sentence>,
    pub updated: u64,
    pub updated_said: Option<Sentence>,
    pub subscribers: u64,
    pub favorites: u64,
    pub banned: bool,
    pub preview_url: String,
    /// Only with the key.
    pub votes_said: Option<Sentence>,
    /// Only with the key.
    #[serde(skip)]
    pub screenshot_urls: Vec<String>,
    pub requires: Vec<Row>,
    pub unknown_requirements: Vec<String>,
    pub unknown_said: Option<Sentence>,
    pub dlc: Vec<Dlc>,
}

fn votes_said(votes: Votes) -> Option<Sentence> {
    let total = usize::try_from(votes.up + votes.down).unwrap_or(usize::MAX);
    let liked = (votes.score * 100.0).round() as u64;
    (total > 0).then(|| say_n!(total, "{} vote, {}% up", "{} votes, {}% up", liked.min(100)))
}

impl Shown {
    pub fn picture_urls(&self) -> Vec<String> {
        self.pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Image(url) => Some(url.clone()),
                Piece::Text(_) => None,
            })
            .collect()
    }

    pub fn description(&self, files: &HashMap<String, String>) -> Vec<Part> {
        self.pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text(blocks) => Part {
                    blocks: blocks.clone(),
                    image: String::new(),
                    note: None,
                },
                Piece::Image(url) => Part {
                    blocks: Vec::new(),
                    image: files.get(url).cloned().unwrap_or_default(),
                    note: (!files.contains_key(url)).then(|| say!("A picture is on its way…")),
                },
            })
            .collect()
    }

    pub fn of(
        mut row: Row,
        entry: &Entry,
        source: &dyn Browse,
        details: Option<&Details>,
        problem: Option<Sentence>,
        requires: Vec<Row>,
        now: u64,
    ) -> Self {
        let known: HashSet<&str> = requires.iter().map(|r| r.id.as_str()).collect();
        let unknown_requirements: Vec<String> = entry
            .requires
            .iter()
            .filter(|id| !known.contains(id.as_str()))
            .cloned()
            .collect();
        let unknown_said = (!unknown_requirements.is_empty()).then(|| {
            rimstack_core::say_n!(
                unknown_requirements.len(),
                "{} requirement is not in the database, so it is not added",
                "{} requirements are not in the database, so they are not added",
                unknown_requirements.len()
            )
        });
        row.stars = stars_of(details.and_then(|d| d.votes));
        let size = details.map_or(0, |d| d.size);
        let updated = details.map_or(0, |d| d.updated);
        let created = details.map_or(0, |d| d.created);
        let count = |n: u64| Sentence::Number(i64::try_from(n).unwrap_or(i64::MAX));
        let stats = details.map_or_else(Vec::new, |d| {
            vec![
                Stat {
                    label: say!("Unique visitors"),
                    value: count(d.views),
                },
                Stat {
                    label: say!("Current subscribers"),
                    value: count(d.subscriptions),
                },
                Stat {
                    label: say!("Current favourites"),
                    value: count(d.favorited),
                },
            ]
        });
        Shown {
            row,
            loaded: details.is_some(),
            problem,
            pieces: details.map_or_else(Vec::new, |d| pieces(&d.description, PICTURES)),
            created_by: entry
                .author
                .as_ref()
                .map(|author| say!("Created by {}", author)),
            stats,
            created_said: (created > 0)
                .then(|| say!("Created {}", ago(now.saturating_sub(created)).inside())),
            size,
            size_said: (size > 0).then(|| sizes::of_bytes(size)),
            updated,
            updated_said: (updated > 0)
                .then(|| say!("Updated {}", ago(now.saturating_sub(updated)).inside())),
            subscribers: details.map_or(0, |d| d.subscriptions),
            favorites: details.map_or(0, |d| d.favorited),
            banned: details.is_some_and(|d| d.banned),
            preview_url: details.map_or_else(String::new, |d| d.preview_url.clone()),
            votes_said: details.and_then(|d| d.votes).and_then(votes_said),
            screenshot_urls: details.map_or_else(Vec::new, |d| d.screenshots.clone()),
            requires,
            unknown_requirements,
            unknown_said,
            dlc: entry
                .dlc
                .iter()
                .map(|id| Dlc {
                    id: id.clone(),
                    name: source.app_name(id),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_are_five_fill_states_from_the_score_and_none_without_votes() {
        let stars = |up, down, score| stars_of(Some(Votes { up, down, score }));
        assert_eq!(stars(9, 1, 0.9), [true, true, true, true, true]);
        assert_eq!(stars(5, 5, 0.5), [true, true, true, false, false]);
        assert_eq!(stars(1, 9, 0.0), [false; 5]);
        assert!(stars(0, 0, 0.0).is_empty() && stars_of(None).is_empty());
    }

    #[test]
    fn votes_are_said_with_their_count_and_the_share_that_liked() {
        let said = |up, down, score| votes_said(Votes { up, down, score }).map(|s| s.to_string());
        assert_eq!(said(90, 10, 0.9).as_deref(), Some("100 votes, 90% up"));
        assert_eq!(said(1, 0, 1.0).as_deref(), Some("1 vote, 100% up"));
        assert_eq!(said(0, 0, 0.0), None);
    }
}
