use crate::mods::facts::Facts;
use crate::mods::library::ModId;
use filter::Filter;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Earlier variants load earlier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Loaders,
    Game,
    Libraries,
    Content,
    Patches,
    Textures,
    Interface,
    Tweaks,
    Last,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Loaders,
        Category::Game,
        Category::Libraries,
        Category::Content,
        Category::Patches,
        Category::Textures,
        Category::Interface,
        Category::Tweaks,
        Category::Last,
    ];

    pub fn from_label(label: &str) -> Option<Self> {
        // Notes written before the tweaks joined still say "Performance".
        let label = if label == "Performance" {
            Category::Tweaks.label()
        } else {
            label
        };
        Self::ALL.into_iter().find(|c| c.label() == label)
    }

    /// Kept in the notes by key: never reworded, as its label may be.
    pub fn key(self) -> &'static str {
        match self {
            Category::Loaders => "loaders",
            Category::Game => "game",
            Category::Libraries => "libraries",
            Category::Content => "content",
            Category::Patches => "patches",
            Category::Textures => "textures",
            Category::Interface => "interface",
            Category::Tweaks => "tweaks",
            Category::Last => "last",
        }
    }

    /// Notes kept before there were keys hold a label instead.
    pub fn from_kept(kept: &str) -> Option<Self> {
        let by_key = Self::ALL.into_iter().find(|c| c.key() == kept);
        by_key.or_else(|| Self::from_label(kept))
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Loaders => "Loaders",
            Category::Game => "Game and DLC",
            Category::Libraries => "Libraries",
            Category::Content => "Content",
            Category::Patches => "Patches",
            Category::Textures => "Textures",
            Category::Interface => "Interface",
            Category::Tweaks => "Performance & tweaks",
            Category::Last => "Loads last",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mod {
    /// Empty for demo data and for a folder that cannot be read as a mod.
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    /// The latest game version its About.xml lists; empty when it lists none.
    pub latest_game: String,
    pub category: Category,
    /// `None` for a mod that is not installed and for demo data.
    pub installed: Option<ModId>,
    pub tags: Vec<String>,
    pub local: bool,
    pub workshop: bool,
    pub git: bool,
    /// Left out of the list unless the filter asks for it with `is:leftover`.
    pub leftover: bool,
    pub code: bool,
    /// Lower case `type.method`.
    pub patched: std::sync::Arc<[String]>,
    /// Its first tag; empty without tags.
    pub sub: String,
    /// Microseconds it took at the game's last start, where measured.
    pub load_micros: Option<u32>,
    /// Seconds since 1970 when Steam last updated it; Workshop mods only.
    pub updated: Option<u64>,
    /// Bytes, as Steam records them; Workshop mods only.
    pub size: Option<u64>,
    /// When its folder was made: for a Workshop mod, its first download.
    pub downloaded: Option<u64>,
    /// Since when it has been in the saved load order.
    pub activated: Option<u64>,
    /// Its own category, chosen or worked out; `category` is where it sits, which a rule can
    /// make an earlier one. A sort starts from this.
    pub filed: Category,
    /// `filed` was chosen by hand.
    pub chosen: bool,
    /// The mod that has to load after it and keeps it in an earlier category than `filed`.
    pub held_by: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Group,
    Name,
    Author,
    Version,
    PackageId,
    Updated,
    Downloaded,
    Activated,
    Size,
    Load,
}

impl SortKey {
    pub const ALL: [SortKey; 10] = [
        SortKey::Group,
        SortKey::Name,
        SortKey::Author,
        SortKey::Version,
        SortKey::PackageId,
        SortKey::Updated,
        SortKey::Downloaded,
        SortKey::Activated,
        SortKey::Size,
        SortKey::Load,
    ];

    /// Kept in the settings and handed to the window: never reworded.
    pub fn key(self) -> &'static str {
        match self {
            SortKey::Group => "group",
            SortKey::Name => "name",
            SortKey::Author => "author",
            SortKey::Version => "version",
            SortKey::PackageId => "package",
            SortKey::Updated => "updated",
            SortKey::Downloaded => "downloaded",
            SortKey::Activated => "activated",
            SortKey::Size => "size",
            SortKey::Load => "load",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    /// Shown one way only: asked again, it gives back the list's own order.
    pub fn turns(self) -> bool {
        self != SortKey::Group
    }

    pub fn starts_descending(self) -> bool {
        matches!(
            self,
            SortKey::Updated | SortKey::Downloaded | SortKey::Activated | SortKey::Size
        )
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortValue {
    Text(String),
    Number(u64),
}

type SortedBy = (Option<SortValue>, String);

fn sorted_by(m: &Mod, key: SortKey) -> SortedBy {
    let tie = match key {
        SortKey::Name => String::new(),
        _ => m.name.to_lowercase(),
    };
    (sort_value(m, key), tie)
}

fn sort_value(m: &Mod, key: SortKey) -> Option<SortValue> {
    let text = |t: &str| (!t.is_empty()).then(|| SortValue::Text(t.to_lowercase()));
    match key {
        SortKey::Group => text(&m.sub),
        SortKey::Name => text(&m.name),
        SortKey::Author => text(&m.author),
        SortKey::Version => text(&m.version),
        SortKey::PackageId => text(&m.id),
        SortKey::Updated => m.updated.map(SortValue::Number),
        SortKey::Downloaded => m.downloaded.map(SortValue::Number),
        SortKey::Activated => m.activated.map(SortValue::Number),
        SortKey::Size => m.size.map(SortValue::Number),
        SortKey::Load => m.load_micros.map(|micros| SortValue::Number(micros.into())),
    }
}

/// A mod without the value comes last either way; mods with the same value go by name.
fn sorted<T>(items: Vec<T>, by: impl Fn(&T) -> SortedBy, descending: bool) -> Vec<T> {
    let mut keyed: Vec<(SortedBy, T)> = items.into_iter().map(|item| (by(&item), item)).collect();
    keyed.sort_by(|((a, a_name), _), ((b, b_name), _)| {
        let by_value = match descending {
            true => b.cmp(a),
            false => a.cmp(b),
        };
        let missing = a.is_none().cmp(&b.is_none());
        missing.then(by_value).then_with(|| a_name.cmp(b_name))
    });
    keyed.into_iter().map(|(_, item)| item).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Header {
        category: Category,
        count: usize,
        collapsed: bool,
    },
    Sub {
        first: usize,
        count: usize,
        collapsed: bool,
    },
    Mod {
        index: usize,
    },
}

pub fn runs(mods: &[Mod]) -> usize {
    let changes = mods.windows(2).filter(|w| w[0].category != w[1].category);
    changes.count() + usize::from(!mods.is_empty())
}

pub struct GroupedList {
    mods: Vec<Mod>,
    /// Lower-cased "name author" per mod; filtering allocates nothing.
    haystacks: Vec<String>,
    collapsed: HashSet<Category>,
    collapsed_subs: HashSet<(Category, String)>,
    filter: Filter,
    /// Which list this is, as the filter last said; `None` lets `is:active` and `is:inactive` pass.
    side: Option<Side>,
    left_out: LeftOut,
    unmatched: bool,
    /// Empty until `set_flags`, and again after the order changes.
    flags: Vec<u16>,
    grouped: bool,
    shown_by: Option<(SortKey, bool)>,
    rows: Vec<Row>,
    facts: Arc<HashMap<ModId, Arc<Facts>>>,
    /// While held, the rows are made again only once, when let go.
    held: bool,
}

fn haystack(m: &Mod) -> String {
    format!("{} {}", m.name, m.author).to_lowercase()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeftOut {
    Dimmed,
    #[default]
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Active,
    Inactive,
}

impl Side {
    pub fn of(active: bool) -> Self {
        if active { Side::Active } else { Side::Inactive }
    }

    pub fn other(self) -> Self {
        match self {
            Side::Active => Side::Inactive,
            Side::Inactive => Side::Active,
        }
    }
}

pub mod flag {
    pub const ERROR: u16 = 1;
    pub const WARNING: u16 = 2;
    pub const CONFLICT: u16 = 4;
    pub const UPDATED: u16 = 8;
    pub const NOT_IN_SAVE: u16 = 16;
    pub const RECENT: u16 = 32;
    pub const DUPLICATE: u16 = 64;
    pub const NEW: u16 = 128;
    pub const CHANGED: u16 = 256;
    pub const IN_SEARCH: u16 = 512;
}

impl GroupedList {
    /// `mods` must already be in load order.
    pub fn new(mods: Vec<Mod>) -> Self {
        let haystacks = mods.iter().map(haystack).collect();
        let mut list = Self {
            mods,
            haystacks,
            collapsed: HashSet::new(),
            collapsed_subs: HashSet::new(),
            filter: Filter::default(),
            side: None,
            left_out: LeftOut::default(),
            unmatched: false,
            flags: Vec::new(),
            grouped: true,
            shown_by: None,
            rows: Vec::new(),
            facts: Arc::default(),
            held: false,
        };
        list.rebuild();
        list
    }

    /// Many edits in a row: the rows stay as they were until `let_go`.
    pub fn hold(&mut self) {
        self.held = true;
    }

    pub fn let_go(&mut self) {
        self.held = false;
        self.rebuild();
    }

    pub fn flat(mods: Vec<Mod>) -> Self {
        let mut list = Self::new(mods);
        list.grouped = false;
        list.rebuild();
        list
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn mods(&self) -> &[Mod] {
        &self.mods
    }

    /// Changing the order forgets it.
    pub fn set_flags(&mut self, flags: Vec<u16>) {
        self.flags = flags;
        if self.filter.reads_flags() {
            self.rebuild();
        }
    }

    pub fn flagged(&self, mark: u16) -> usize {
        self.flags.iter().filter(|flags| *flags & mark != 0).count()
    }

    /// `None` puts it in category order, under headings.
    pub fn arrange(&mut self, order: Option<(SortKey, bool)>) {
        let mut mods = std::mem::take(&mut self.mods);
        match order {
            Some((key, descending)) => mods = sorted(mods, |m| sorted_by(m, key), descending),
            None => mods.sort_by_cached_key(|m| (m.category, m.name.to_lowercase())),
        }
        self.grouped = order.is_none();
        self.set_mods(mods);
    }

    pub fn set_tags(&mut self, installed: ModId, tags: &[String]) -> bool {
        let Some(m) = self
            .mods
            .iter_mut()
            .find(|m| m.installed == Some(installed))
        else {
            return false;
        };
        m.tags = tags.to_vec();
        m.sub = tags.first().cloned().unwrap_or_default();
        self.rebuild();
        true
    }

    pub fn toggle_sub(&mut self, first: usize) {
        let Some(m) = self.mods.get(first).filter(|m| !m.sub.is_empty()) else {
            return;
        };
        let key = (m.category, m.sub.clone());
        if !self.collapsed_subs.remove(&key) {
            self.collapsed_subs.insert(key);
        }
        self.rebuild();
    }

    pub fn in_sub_of(&self, first: usize) -> Vec<usize> {
        let Some(of) = self.mods.get(first).filter(|m| !m.sub.is_empty()) else {
            return Vec::new();
        };
        let same = |m: &Mod| m.category == of.category && m.sub == of.sub;
        let each = self.mods.iter().enumerate();
        each.filter(|(_, m)| same(m)).map(|(at, _)| at).collect()
    }

    pub fn subs(&self, category: Category) -> Vec<String> {
        let in_it = self.mods.iter().filter(|m| m.category == category);
        let names: std::collections::BTreeSet<&str> = in_it.map(|m| m.sub.as_str()).collect();
        names
            .into_iter()
            .filter(|n| !n.is_empty())
            .map(String::from)
            .collect()
    }

    /// Positions in `Row::Mod` stay load-order positions.
    pub fn show_by(&mut self, order: Option<(SortKey, bool)>) {
        self.shown_by = order;
        self.rebuild();
    }

    pub fn shown_by(&self) -> Option<(SortKey, bool)> {
        self.shown_by
    }

    pub fn is_grouped(&self) -> bool {
        self.grouped
    }

    pub fn set_grouped(&mut self, grouped: bool) {
        self.grouped = grouped;
        self.rebuild();
    }

    pub fn set_mods(&mut self, mods: Vec<Mod>) {
        self.haystacks = mods.iter().map(haystack).collect();
        self.mods = mods;
        self.flags.clear();
        self.rebuild();
    }

    pub fn set_facts(&mut self, facts: Arc<HashMap<ModId, Arc<Facts>>>) {
        self.facts = facts;
        if self.filter.needs_facts() {
            self.rebuild();
        }
    }

    pub fn needs_facts(&self) -> bool {
        self.filter.needs_facts() && !self.filter.shuts(self.side)
    }

    pub fn unread(&self) -> usize {
        if !self.filter.needs_facts() || self.filter.shuts(self.side) {
            return 0;
        }
        let installed = self.mods.iter().filter_map(|m| m.installed);
        installed.filter(|id| !self.facts.contains_key(id)).count()
    }

    pub fn matching(&self) -> usize {
        if self.unmatched {
            return 0;
        }
        (0..self.mods.len()).filter(|&i| self.matches(i)).count()
    }

    pub fn left_out(&self) -> Option<LeftOut> {
        self.unmatched.then_some(self.left_out)
    }

    pub fn set_left_out(&mut self, left_out: LeftOut) {
        if self.left_out != left_out {
            self.left_out = left_out;
            self.rebuild();
        }
    }

    pub fn set_filter(&mut self, text: &str) {
        self.filter = Filter::parse(text);
        self.side = None;
        self.rebuild();
    }

    pub fn set_filter_on(&mut self, side: Side, text: &str) {
        self.filter = Filter::parse(text);
        self.side = Some(side);
        self.rebuild();
    }

    pub fn toggle(&mut self, category: Category) {
        if !self.collapsed.remove(&category) {
            self.collapsed.insert(category);
        }
        self.rebuild();
    }

    pub fn keep_folds_of(&mut self, earlier: &GroupedList) {
        self.collapsed.clone_from(&earlier.collapsed);
        self.collapsed_subs.clone_from(&earlier.collapsed_subs);
        self.rebuild();
    }

    pub fn move_mod(&mut self, from: usize, to: usize) {
        if from >= self.mods.len() || to >= self.mods.len() || from == to {
            return;
        }
        let m = self.mods.remove(from);
        let h = self.haystacks.remove(from);
        self.mods.insert(to, m);
        self.haystacks.insert(to, h);
        self.flags.clear();
        self.rebuild();
    }

    pub fn insert_in_category(&mut self, m: Mod) -> usize {
        let at = if self.grouped {
            self.mods
                .iter()
                .rposition(|x| x.category <= m.category)
                .map_or(0, |p| p + 1)
        } else {
            self.mods.len()
        };
        self.haystacks.insert(at, haystack(&m));
        self.mods.insert(at, m);
        self.flags.clear();
        self.rebuild();
        at
    }

    pub fn remove(&mut self, index: usize) -> Option<Mod> {
        if index >= self.mods.len() {
            return None;
        }
        self.haystacks.remove(index);
        let m = self.mods.remove(index);
        self.flags.clear();
        self.rebuild();
        Some(m)
    }

    fn listed(&self, index: usize) -> bool {
        !self.mods[index].leftover || self.filter.leftover
    }

    pub fn counted(&self) -> usize {
        self.mods.iter().filter(|m| !m.leftover).count()
    }

    fn matches(&self, index: usize) -> bool {
        if !self.listed(index) {
            return false;
        }
        let flags = self.flags.get(index).copied().unwrap_or(0);
        let facts = self.mods[index]
            .installed
            .and_then(|id| self.facts.get(&id));
        let facts = facts.map(|facts| &**facts);
        if self.filter.shuts(self.side) {
            return false;
        }
        self.filter.is_empty()
            || self
                .filter
                .admits(&self.mods[index], &self.haystacks[index], flags, facts)
    }

    fn shows(&self, index: usize) -> bool {
        match self.unmatched {
            true => self.left_out == LeftOut::Dimmed && self.listed(index),
            false => self.matches(index),
        }
    }

    fn rebuild(&mut self) {
        if self.held {
            return;
        }
        self.unmatched = !self.filter.is_empty() && !(0..self.mods.len()).any(|i| self.matches(i));
        if let Some((SortKey::Group, _)) = self.shown_by {
            self.rows = self.gathered();
            return;
        }
        if let Some((key, descending)) = self.shown_by {
            let shown: Vec<usize> = (0..self.mods.len()).filter(|&i| self.shows(i)).collect();
            let shown = sorted(shown, |&i| sorted_by(&self.mods[i], key), descending);
            self.rows = shown.into_iter().map(|index| Row::Mod { index }).collect();
            return;
        }
        if !self.grouped {
            self.rows = (0..self.mods.len())
                .filter(|&i| self.shows(i))
                .map(|index| Row::Mod { index })
                .collect();
            return;
        }
        let mut rows = Vec::with_capacity(self.mods.len() + Category::ALL.len());
        let mut start = 0;
        while start < self.mods.len() {
            let category = self.mods[start].category;
            let mut end = start;
            while end < self.mods.len() && self.mods[end].category == category {
                end += 1;
            }
            let visible: Vec<usize> = (start..end).filter(|&i| self.shows(i)).collect();
            self.push_category(&mut rows, category, &visible);
            start = end;
        }
        self.rows = rows;
    }

    /// Each category once and each of its tags once, wherever their mods sit: the untagged first,
    /// then the tags in the order their first mods load, each tag's mods in load order.
    fn gathered(&self) -> Vec<Row> {
        let mut shown: Vec<usize> = (0..self.mods.len()).filter(|&i| self.shows(i)).collect();
        let mut first_at: HashMap<(Category, &str), usize> = HashMap::new();
        for &i in &shown {
            let m = &self.mods[i];
            first_at.entry((m.category, m.sub.as_str())).or_insert(i);
        }
        shown.sort_by_key(|&i| {
            let m = &self.mods[i];
            let tag_at = first_at[&(m.category, m.sub.as_str())];
            (m.category, !m.sub.is_empty(), tag_at)
        });
        let mut rows = Vec::with_capacity(shown.len() + Category::ALL.len());
        for run in shown.chunk_by(|&a, &b| self.mods[a].category == self.mods[b].category) {
            self.push_category(&mut rows, self.mods[run[0]].category, run);
        }
        rows
    }

    fn push_category(&self, rows: &mut Vec<Row>, category: Category, visible: &[usize]) {
        if visible.is_empty() {
            return;
        }
        let collapsed = self.collapsed.contains(&category);
        rows.push(Row::Header {
            category,
            count: visible.len(),
            collapsed,
        });
        if !collapsed {
            self.push_with_subs(rows, visible);
        }
    }

    fn push_with_subs(&self, rows: &mut Vec<Row>, visible: &[usize]) {
        let mut at = 0;
        while at < visible.len() {
            let sub = &self.mods[visible[at]].sub;
            let same = visible[at..]
                .iter()
                .take_while(|&&i| self.mods[i].sub == *sub);
            let count = same.count();
            let key = (self.mods[visible[at]].category, sub.clone());
            let collapsed = !sub.is_empty() && self.collapsed_subs.contains(&key);
            if !sub.is_empty() {
                rows.push(Row::Sub {
                    first: visible[at],
                    count,
                    collapsed,
                });
            }
            if !collapsed {
                let stretch = &visible[at..at + count];
                rows.extend(stretch.iter().map(|&index| Row::Mod { index }));
            }
            at += count;
        }
    }
}

pub fn split_groups<'a>(mods: impl IntoIterator<Item = &'a Mod>) -> Vec<String> {
    let mut closed: HashSet<(Category, &str)> = HashSet::new();
    let mut split: Vec<String> = Vec::new();
    let mut last: Option<(Category, &str)> = None;
    for m in mods {
        let now = (!m.sub.is_empty()).then_some((m.category, m.sub.as_str()));
        if now == last {
            continue;
        }
        if let Some(left) = last {
            closed.insert(left);
        }
        if let Some(entered) = now
            && closed.contains(&entered)
            && !split.iter().any(|name| name == entered.1)
        {
            split.push(entered.1.to_string());
        }
        last = now;
    }
    split
}

pub fn twins(mods: &[Mod]) -> Vec<bool> {
    let mut named: std::collections::HashMap<String, usize> = Default::default();
    for m in mods {
        *named.entry(m.name.to_ascii_lowercase()).or_default() += 1;
    }
    let shared = |m: &Mod| named[&m.name.to_ascii_lowercase()] > 1;
    mods.iter().map(shared).collect()
}

pub fn edge_of_run(mods: &[Mod], at: usize, top: bool) -> usize {
    let same = |other: &Mod| other.category == mods[at].category;
    match top {
        true => at - mods[..at].iter().rev().take_while(|m| same(m)).count(),
        false => at + mods[at + 1..].iter().take_while(|m| same(m)).count(),
    }
}

mod filter;
mod moves;
mod navigate;
pub mod offers;
#[cfg(test)]
mod tests;
