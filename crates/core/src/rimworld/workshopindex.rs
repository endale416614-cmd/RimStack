//! Kept as JSON lines, one published mod per line:
//! `[Workshop id, title, author|null, package id|null, [tags], [required Workshop ids], [required DLC app ids]]`.
//! After them, one line per expansion: `[Steam app id, name]`. A file without these lines reads as before.

use super::workshopsweep::Sweep;
use crate::base::error::Error;
use crate::base::versions;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub package_id: Option<String>,
    /// Steam's own: `Mod`, `Translation`, `Scenario`, and the game versions.
    pub tags: Vec<String>,
    pub requires: Vec<String>,
    /// Steam app ids of the DLC it needs, never Workshop ids.
    pub dlc: Vec<String>,
}

type Line = (
    String,
    String,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
);

/// Variable-length lists in one array: `at[i]..at[i + 1]` is the list of entry `i`.
struct Flat<T> {
    at: Vec<u32>,
    items: Vec<T>,
}

impl<T> Default for Flat<T> {
    fn default() -> Self {
        Flat {
            at: vec![0],
            items: Vec::new(),
        }
    }
}

impl<T> Flat<T> {
    fn push(&mut self, list: impl IntoIterator<Item = T>) {
        self.items.extend(list);
        self.at.push(self.items.len() as u32);
    }

    fn get(&self, i: usize) -> &[T] {
        &self.items[self.at[i] as usize..self.at[i + 1] as usize]
    }
}

/// Texts back to back in one string: entry `i` owns the three ends `ends[3i..3i + 3]`.
#[derive(Default)]
struct Trio {
    text: String,
    ends: Vec<u32>,
}

impl Trio {
    fn push(&mut self, parts: [&str; 3]) {
        for part in parts {
            self.text.push_str(part);
            self.ends.push(self.text.len() as u32);
        }
    }

    fn get(&self, i: usize, part: usize) -> &str {
        let end = self.ends[3 * i + part] as usize;
        let start = match 3 * i + part {
            0 => 0,
            n => self.ends[n - 1] as usize,
        };
        &self.text[start..end]
    }
}

/// What the sweep gave of one item; all zero for an item it has not reached.
#[derive(Clone, Copy, Default)]
pub(super) struct Stat {
    pub subscriptions: u32,
    pub created: u32,
    pub updated: u32,
    flags: u8,
}

const REMOVED: u8 = 1;
const BANNED: u8 = 2;

fn clamped(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

const TITLE: usize = 0;
const AUTHOR: usize = 1;
const PACKAGE: usize = 2;
const HAS_AUTHOR: u8 = 1;
const HAS_PACKAGE: u8 = 2;

/// Rows sorted by Workshop id.
#[derive(Default)]
pub struct Index {
    ids: Vec<u64>,
    shown: Trio,
    folded: Trio,
    has: Vec<u8>,
    tag_names: Vec<String>,
    tag_keys: Vec<String>,
    tags: Flat<u32>,
    requires: Flat<u64>,
    dlc: Flat<u64>,
    /// `(required id, entry)`, sorted: the mods that need an id are its range.
    needed_by: Vec<(u64, u32)>,
    by_author: Vec<u32>,
    apps: BTreeMap<String, String>,
    version_counts: Vec<(String, usize)>,
    /// One per entry once a sweep is merged in; empty before.
    stats: Vec<Stat>,
}

/// Measured on the real database: the mistyped tags (0.8 to 0.13, 1.18, 2.x) have 5 mods or
/// fewer, the smallest real version (0.19) has 543.
pub const REAL_VERSION: usize = 100;

fn is_version_tag(tag: &str) -> bool {
    let mut parts = tag.split('.');
    let digits =
        |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    digits(parts.next()) && digits(parts.next()) && parts.next().is_none()
}

pub fn file_in(data_dir: &Path) -> PathBuf {
    data_dir.join("workshopIndex.jsonl")
}

fn expansion_name(database_name: &str) -> &str {
    let name = database_name
        .strip_prefix("RimWorld - ")
        .unwrap_or(database_name);
    match name.trim() {
        "" => database_name,
        short => short,
    }
}

fn numbers(texts: &[String]) -> impl Iterator<Item = u64> + '_ {
    texts.iter().filter_map(|t| t.parse().ok())
}

#[derive(Default)]
struct Builder {
    index: Index,
    interned: HashMap<String, u32>,
}

impl Builder {
    /// An entry whose id is not a number is left out; so are a requirement's and a DLC's.
    fn push(&mut self, entry: Entry) {
        let Some(id) = entry.id.parse().ok() else {
            return;
        };
        let title = entry.title.trim();
        let author = entry.author.as_deref();
        let package = entry.package_id.as_deref();
        let tags: Vec<u32> = entry.tags.iter().map(|t| self.intern(t)).collect();
        let index = &mut self.index;
        index.ids.push(id);
        index.shown.push([
            title,
            author.unwrap_or_default(),
            package.unwrap_or_default(),
        ]);
        let fold = |text: Option<&str>| text.unwrap_or_default().to_lowercase();
        index
            .folded
            .push([&title.to_lowercase(), &fold(author), &fold(package)]);
        index.has.push(
            u8::from(author.is_some()) * HAS_AUTHOR + u8::from(package.is_some()) * HAS_PACKAGE,
        );
        index.tags.push(tags);
        index.requires.push(numbers(&entry.requires));
        index.dlc.push(numbers(&entry.dlc));
    }

    fn intern(&mut self, tag: &str) -> u32 {
        if let Some(&at) = self.interned.get(tag) {
            return at;
        }
        let at = self.interned.len() as u32;
        self.interned.insert(tag.to_string(), at);
        self.index.tag_names.push(tag.to_string());
        self.index.tag_keys.push(tag.to_lowercase());
        at
    }

    fn finish(self) -> Index {
        let mut index = self.index;
        if !index.ids.windows(2).all(|w| w[0] <= w[1]) {
            index = index.sorted();
        }
        index.derive();
        index
    }
}

impl Index {
    pub fn new(entries: Vec<Entry>) -> Index {
        let mut builder = Builder::default();
        entries.into_iter().for_each(|entry| builder.push(entry));
        builder.finish()
    }

    fn sorted(self) -> Index {
        let mut order: Vec<usize> = (0..self.ids.len()).collect();
        order.sort_by_key(|&at| self.ids[at]);
        let mut builder = Builder::default();
        order
            .into_iter()
            .for_each(|at| builder.push(self.entry(at)));
        builder.index
    }

    fn derive(&mut self) {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for at in 0..self.ids.len() {
            let own: BTreeSet<u32> = self.tags.get(at).iter().copied().collect();
            for tag in own.iter().map(|&t| self.tag_names[t as usize].as_str()) {
                if is_version_tag(tag) {
                    *counts.entry(tag).or_default() += 1;
                }
            }
        }
        self.version_counts = counts
            .into_iter()
            .map(|(t, n)| (t.to_string(), n))
            .collect();
        self.needed_by = (0..self.ids.len())
            .flat_map(|at| self.requires.get(at).iter().map(move |&r| (r, at as u32)))
            .collect();
        self.needed_by.sort_unstable();
        let mut authored: Vec<u32> = (0..self.ids.len() as u32)
            .filter(|&at| self.has[at as usize] & HAS_AUTHOR != 0)
            .collect();
        authored.sort_by(|&a, &b| {
            let key = |at: u32| self.author_key(at as usize);
            key(a).cmp(key(b)).then(a.cmp(&b))
        });
        self.by_author = authored;
    }

    pub fn with_sweep(mut self, sweep: &Sweep) -> Index {
        if sweep.is_empty() {
            return self;
        }
        let apps = std::mem::take(&mut self.apps);
        let mut builder = Builder::default();
        let mut stats = Vec::with_capacity(self.len());
        for at in 0..self.len() {
            let mut entry = self.entry(at);
            let mut stat = Stat::default();
            if let Some(record) = sweep.records.get(&self.ids[at]) {
                if record.removed {
                    stat.flags = REMOVED;
                } else {
                    if !record.title.trim().is_empty() {
                        entry.title = record.title.clone();
                    }
                    if !record.tags.is_empty() {
                        entry.tags = record.tags.clone();
                    }
                    stat = Stat {
                        subscriptions: clamped(record.subscriptions),
                        created: clamped(record.created),
                        updated: clamped(record.updated),
                        flags: if record.banned { BANNED } else { 0 },
                    };
                }
            }
            builder.push(entry);
            stats.push(stat);
        }
        let mut merged = builder.finish().with_apps(apps);
        merged.stats = stats;
        merged
    }

    pub fn swept(&self) -> bool {
        !self.stats.is_empty()
    }

    pub(super) fn stat(&self, at: usize) -> Stat {
        self.stats.get(at).copied().unwrap_or_default()
    }

    pub(super) fn incompatible(&self, at: usize) -> bool {
        self.stat(at).flags & (REMOVED | BANNED) != 0
    }

    pub(super) fn tag_count(&self) -> usize {
        self.tag_names.len()
    }

    pub(super) fn tag_name(&self, tag: u32) -> &str {
        &self.tag_names[tag as usize]
    }

    pub(super) fn tags_of(&self, at: usize) -> &[u32] {
        self.tags.get(at)
    }

    pub(super) fn id_at(&self, at: usize) -> u64 {
        self.ids[at]
    }

    pub(super) fn title_key(&self, at: usize) -> &str {
        self.folded.get(at, TITLE)
    }

    pub(super) fn author_key(&self, at: usize) -> &str {
        self.folded.get(at, AUTHOR)
    }

    pub(super) fn package_key(&self, at: usize) -> &str {
        self.folded.get(at, PACKAGE)
    }

    pub(super) fn requires_of(&self, at: usize) -> &[u64] {
        self.requires.get(at)
    }

    pub(super) fn dlc_of(&self, at: usize) -> &[u64] {
        self.dlc.get(at)
    }

    pub(super) fn tags_called(&self, key: &str) -> Vec<u32> {
        let named = |(_, k): &(usize, &String)| k.as_str() == key;
        let found = self.tag_keys.iter().enumerate().filter(named);
        found.map(|(at, _)| at as u32).collect()
    }

    pub(super) fn has_any_tag(&self, at: usize, wanted: &[u32]) -> bool {
        self.tags.get(at).iter().any(|t| wanted.contains(t))
    }

    fn entry(&self, at: usize) -> Entry {
        let owned = |part| self.shown.get(at, part).to_string();
        let flag = |bit| self.has[at] & bit != 0;
        let listed = |ids: &[u64]| ids.iter().map(u64::to_string).collect();
        Entry {
            id: self.ids[at].to_string(),
            title: owned(TITLE),
            author: flag(HAS_AUTHOR).then(|| owned(AUTHOR)),
            package_id: flag(HAS_PACKAGE).then(|| owned(PACKAGE)),
            tags: (self.tags.get(at).iter())
                .map(|&t| self.tag_names[t as usize].clone())
                .collect(),
            requires: listed(self.requires.get(at)),
            dlc: listed(self.dlc.get(at)),
        }
    }

    /// The last entry of an id, as the index has always answered for a repeated one.
    fn position(&self, id: &str) -> Option<usize> {
        let id: u64 = id.parse().ok()?;
        let after = self.ids.partition_point(|&known| known <= id);
        after.checked_sub(1).filter(|&at| self.ids[at] == id)
    }

    pub fn with_apps(mut self, apps: BTreeMap<String, String>) -> Index {
        self.apps = apps;
        self
    }

    /// As Steam shows it: the database's `RimWorld - Biotech` is `Biotech`.
    pub fn app_name(&self, app_id: &str) -> Option<&str> {
        self.apps.get(app_id).map(|name| expansion_name(name))
    }

    pub fn apps(&self) -> Vec<(String, String)> {
        let mut apps: Vec<(String, String)> = self
            .apps
            .iter()
            .map(|(id, name)| (id.clone(), expansion_name(name).to_string()))
            .collect();
        apps.sort_by_key(|(_, name)| name.to_lowercase());
        apps
    }

    /// The game-version tags with at least `at_least` mods under them, oldest first. Steam lists
    /// the game's real versions (0.14 to 1.7); the rest are mistyped tags (see `REAL_VERSION`).
    pub fn version_tags(&self, at_least: usize) -> Vec<String> {
        let mut tags: Vec<String> = (self.version_counts.iter())
            .filter(|(_, n)| *n >= at_least)
            .map(|(t, _)| t.clone())
            .collect();
        tags.sort_by_key(|tag| versions::numbers(tag));
        tags
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Every Workshop id, ascending.
    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<Entry> {
        self.position(id).map(|at| self.entry(at))
    }

    /// Its Workshop requirements that the index knows, in the order of their ids.
    pub fn requires(&self, id: &str) -> Vec<String> {
        let known = |needed: &&u64| self.ids.binary_search(needed).is_ok();
        self.position(id).map_or_else(Vec::new, |at| {
            let wanted = self.requires.get(at).iter().filter(known);
            wanted.map(u64::to_string).collect()
        })
    }

    pub fn needed_by(&self, id: &str) -> Vec<String> {
        let Ok(id) = id.parse::<u64>() else {
            return Vec::new();
        };
        let from = self.needed_by.partition_point(|&(needed, _)| needed < id);
        let group = self.needed_by[from..]
            .iter()
            .take_while(|&&(needed, _)| needed == id)
            .map(|&(_, at)| at as usize)
            .collect();
        self.titled(group, None)
    }

    pub fn same_author(&self, id: &str) -> Vec<String> {
        let Some(at) = self.position(id) else {
            return Vec::new();
        };
        if self.has[at] & HAS_AUTHOR == 0 {
            return Vec::new();
        }
        let author = self.author_key(at);
        let key = |&&other: &&u32| self.author_key(other as usize);
        let from = self.by_author.partition_point(|other| key(&other) < author);
        let group = self.by_author[from..]
            .iter()
            .take_while(|other| key(other) == author)
            .map(|&other| other as usize)
            .collect();
        self.titled(group, Some(at))
    }

    fn titled(&self, mut found: Vec<usize>, except: Option<usize>) -> Vec<String> {
        found.retain(|&at| Some(at) != except);
        found.sort_by(|&a, &b| self.title_key(a).cmp(self.title_key(b)).then(a.cmp(&b)));
        found
            .into_iter()
            .map(|at| self.ids[at].to_string())
            .collect()
    }

    pub fn store(&self, to: &Path) -> Result<(), Error> {
        let entries = (0..self.len()).map(|at| {
            let e = self.entry(at);
            serde_json::json!([
                e.id,
                e.title,
                e.author,
                e.package_id,
                e.tags,
                e.requires,
                e.dlc
            ])
        });
        let apps = self.apps.iter().map(|app| serde_json::json!(app));
        crate::base::atomic_file::write_lines(to, entries.chain(apps))
    }

    /// Empty when the file is missing; a line that is not as written is skipped.
    pub fn read(path: &Path) -> Index {
        let Ok(file) = std::fs::File::open(path) else {
            return Index::default();
        };
        let mut builder = Builder::default();
        let mut apps = BTreeMap::new();
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            if let Ok((id, title, author, package_id, tags, requires, dlc)) =
                serde_json::from_str::<Line>(&line)
            {
                builder.push(Entry {
                    id,
                    title,
                    author,
                    package_id,
                    tags,
                    requires,
                    dlc,
                });
            } else if let Ok((id, name)) = serde_json::from_str::<(String, String)>(&line) {
                apps.insert(id, name);
            }
        }
        builder.finish().with_apps(apps)
    }
}

#[cfg(test)]
pub(super) fn entry(id: &str, title: &str, author: Option<&str>, tags: &[&str]) -> Entry {
    Entry {
        id: id.into(),
        title: title.into(),
        author: author.map(Into::into),
        package_id: None,
        tags: tags.iter().map(|t| t.to_string()).collect(),
        requires: Vec::new(),
        dlc: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stocked(tags: &[&str], count: usize) -> Vec<Entry> {
        (0..count)
            .map(|n| entry(&(n + 1).to_string(), "m", None, tags))
            .collect()
    }

    fn sample() -> Index {
        let mut b = entry("200", "Beta", Some("Ann"), &["Mod"]);
        b.requires = vec!["100".into(), "999".into()];
        let mut c = entry("300", "alpha two", Some("ann"), &[]);
        c.requires = vec!["100".into()];
        c.dlc = vec!["1826140".into()];
        Index::new(vec![c, b, entry("100", "Framework", None, &[])])
    }

    #[test]
    fn the_file_reads_back_as_stored_with_awkward_titles() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        assert!(Index::read(&path).is_empty());
        let mut odd = entry(
            "5000000",
            "Tab\there\nand \"quotes\" Ünï",
            Some("Zoë"),
            &["1.6"],
        );
        odd.package_id = Some("a.b".into());
        let index = Index::new(vec![odd.clone(), entry("1000000", "x", None, &[])]);
        index.store(&path).unwrap();
        std::fs::write(
            &path,
            format!("{}garbage\n", std::fs::read_to_string(&path).unwrap()),
        )
        .unwrap();
        let back = Index::read(&path);
        assert_eq!(back.len(), 2);
        assert_eq!(back.get("5000000"), Some(odd));
        assert_eq!(back.app_name("1826140"), None);
    }

    #[test]
    fn expansions_read_back_and_an_older_file_has_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        let apps = BTreeMap::from([("3022790".to_string(), "RimWorld - Odyssey".to_string())]);
        let index = Index::new(vec![entry("1", "x", None, &[])]).with_apps(apps);
        index.store(&path).unwrap();
        let back = Index::read(&path);
        assert_eq!((back.len(), back.app_name("3022790")), (1, Some("Odyssey")));
        std::fs::write(&path, "[\"1\",\"x\",null,null,[],[],[]]\n").unwrap();
        let old = Index::read(&path);
        assert_eq!((old.len(), old.app_name("3022790")), (1, None));
    }

    #[test]
    fn a_file_ending_in_app_lines_names_the_expansions_as_steam_does() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        std::fs::write(
            &path,
            "[\"5\",\"x\",null,null,[\"Mod\"],[],[\"1826140\"]]\n\
             [\"1149640\",\"RimWorld - Royalty\"]\n\
             [\"1826140\",\"RimWorld - Biotech\"]\n\
             [\"2380740\",\"Anomaly\"]\n",
        )
        .unwrap();
        let index = Index::read(&path).with_sweep(&Sweep::default());
        assert_eq!(index.len(), 1);
        let names: Vec<_> = index.apps().into_iter().map(|(_, name)| name).collect();
        assert_eq!(names, ["Anomaly", "Biotech", "Royalty"]);
        assert_eq!(index.app_name("1826140"), Some("Biotech"));
    }

    #[test]
    fn only_versions_with_a_hundred_mods_are_listed() {
        let mut all = stocked(&["1.6", "0.19"], 100);
        all.extend(stocked(&["0.8", "1.18"], 5));
        let index = Index::new(all);
        assert_eq!(index.version_tags(REAL_VERSION), ["0.19", "1.6"]);
        assert_eq!(index.version_tags(1).len(), 4);
    }

    #[test]
    fn requirements_keep_to_what_the_index_knows_and_needed_by_and_same_author_leave_the_mod_out() {
        let index = sample();
        assert_eq!(index.requires("200"), ["100"]);
        assert_eq!(index.get("200").unwrap().requires.len(), 2);
        assert!(index.requires("nope").is_empty());
        assert_eq!(index.needed_by("100"), ["300", "200"]);
        assert!(index.needed_by("300").is_empty());
        assert_eq!(index.same_author("200"), ["300"]);
        assert!(index.same_author("100").is_empty());
    }
}
