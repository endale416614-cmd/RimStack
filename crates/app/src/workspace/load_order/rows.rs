//! The view reads these as published, never the state behind them.

use super::{LoadOrder, Side, side_index};
use crate::App;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe::{Fix, Trouble};
use rimstack_core::mods::library::{Installed, Library, ModId};
use rimstack_core::mods::list::{Category, Mod, Row};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::say;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
    Header(Category, usize),
    Sub(Category, String, usize),
    /// By its folder, the same over a rescan, which gives every mod a new `ModId`.
    Installed(std::path::PathBuf, usize),
    /// By its package id.
    Missing(String, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowView {
    pub key: RowKey,
    pub is_header: bool,
    pub is_sub: bool,
    pub again: bool,
    pub name: Sentence,
    pub author: String,
    pub version: String,
    /// 0 for a heading.
    pub number: i32,
    pub count: i32,
    pub collapsed: bool,
    pub category: i32,
    /// -1 for a heading.
    pub mod_index: i32,
    /// -1 for a heading or a mod that is not installed.
    pub mod_id: i32,
    /// Where Sort moves the mod from, numbered from 1; 0 when it stays.
    pub moved_from: i32,
    /// 0 nothing, 1 a warning, 2 an error.
    pub trouble: i32,
    pub trouble_text: Sentence,
    pub tags: String,
    pub local: bool,
    pub picture: String,
    pub load_time: String,
    /// Against the slowest mod, from 0 to 1.
    pub load_part: f64,
    pub version_ignored: bool,
    pub outdated: bool,
    /// Empty when not outdated.
    pub made_for: String,
    pub fresh: bool,
    /// 0 Workshop, 1 local, 2 a Git checkout, 3 the game's own.
    pub source: i32,
    pub why_moved: Sentence,
    pub group_size: i32,
    pub twin: bool,
}

impl RowView {
    fn heading(key: RowKey, name: &str, category: Category) -> Self {
        Self {
            key,
            is_header: true,
            is_sub: false,
            again: false,
            name: Sentence::Plain(name.to_string()),
            author: String::new(),
            version: String::new(),
            number: 0,
            count: 0,
            collapsed: false,
            category: category as i32,
            mod_index: -1,
            mod_id: -1,
            moved_from: 0,
            trouble: 0,
            trouble_text: Sentence::default(),
            tags: String::new(),
            local: false,
            picture: String::new(),
            load_time: String::new(),
            load_part: 0.0,
            version_ignored: false,
            outdated: false,
            made_for: String::new(),
            fresh: false,
            source: 0,
            why_moved: Sentence::default(),
            group_size: 0,
            twin: false,
        }
    }
}

fn key_of(installed: Option<&Installed>, m: &Mod, nth: &mut impl FnMut(RowKey) -> usize) -> RowKey {
    match installed {
        Some(i) => RowKey::Installed(i.path.clone(), nth(RowKey::Installed(i.path.clone(), 0))),
        None => RowKey::Missing(m.id.clone(), nth(RowKey::Missing(m.id.clone(), 0))),
    }
}

fn source_of(installed: Option<&Installed>) -> (i32, String) {
    let source = installed.map_or(0, |i| match i.source {
        Source::Official => 3,
        Source::Local if i.git => 2,
        Source::Local => 1,
        Source::Workshop => 0,
    });
    let picture = installed.and_then(|i| i.preview.as_ref());
    let picture = picture.map_or(String::new(), |p| p.to_string_lossy().into_owned());
    (source, picture)
}

impl LoadOrder {
    pub fn rows(&self, app: &App, side: Side) -> Vec<RowView> {
        let loaded = self.reading(app);
        let library = loaded.as_ref().as_ref().ok().map(|(l, _)| l);
        let fresh: std::collections::HashSet<ModId> = self.fresh(app, side).into_iter().collect();
        let list = self.list(side);
        let mods = list.mods();
        let mut seen: HashMap<RowKey, usize> = HashMap::new();
        let mut nth = |key: RowKey| -> usize {
            let count = seen.entry(key).or_default();
            *count += 1;
            *count - 1
        };
        let mut categories_shown: Vec<Category> = Vec::new();
        list.rows()
            .iter()
            .map(|row| match *row {
                Row::Header {
                    category,
                    count,
                    collapsed,
                } => {
                    let key = RowKey::Header(category, nth(RowKey::Header(category, 0)));
                    let mut view = RowView::heading(key, category.label(), category);
                    view.again = categories_shown.contains(&category);
                    categories_shown.push(category);
                    view.count = count as i32;
                    view.collapsed = collapsed;
                    view
                }
                Row::Sub {
                    first,
                    count,
                    collapsed,
                } => {
                    let of = &mods[first];
                    let same = |m: &Mod| m.category == of.category && m.sub == of.sub;
                    let key = RowKey::Sub(
                        of.category,
                        of.sub.clone(),
                        nth(RowKey::Sub(of.category, of.sub.clone(), 0)),
                    );
                    let mut view = RowView::heading(key, &of.sub, of.category);
                    view.is_sub = true;
                    view.again = mods[..first].iter().any(same);
                    view.count = count as i32;
                    view.collapsed = collapsed;
                    view.group_size = list.in_sub_of(first).len() as i32;
                    view
                }
                Row::Mod { index } => {
                    self.mod_row(side, index, &mods[index], library, &fresh, &mut nth)
                }
            })
            .collect()
    }

    fn mod_row(
        &self,
        side: Side,
        index: usize,
        m: &Mod,
        library: Option<&Library>,
        fresh: &std::collections::HashSet<ModId>,
        nth: &mut impl FnMut(RowKey) -> usize,
    ) -> RowView {
        let own: &[Trouble] = match side {
            Side::Active => self
                .troubles
                .get(index)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            Side::Inactive => &[],
        };
        let installed = library.zip(m.installed).and_then(|(l, i)| l.get(i));
        let (source, picture) = source_of(installed);
        let took = m.load_micros.filter(|micros| *micros >= 1000);
        let load_time = took
            .map(|micros| rimstack_core::rimworld::startup::seconds(f64::from(micros) / 1000.0));
        let load_part = match (m.load_micros, self.slowest) {
            (Some(own), Some(most)) if most > 0 && side == Side::Active => {
                f64::from(own) / f64::from(most)
            }
            _ => 0.0,
        };
        // A game version is shown as `made_for`.
        let shown = || own.iter().filter(|t| t.fix != Fix::Trust);
        let first = shown().find(|t| t.error).or(shown().next());
        let made_for = self.outdated[side_index(side)]
            .get(index)
            .and_then(Option::as_ref);
        let preview = side == Side::Active;
        RowView {
            key: key_of(installed, m, nth),
            is_header: false,
            is_sub: false,
            again: false,
            name: match m.installed {
                Some(_) => Sentence::Plain(m.name.clone()),
                None => say!("{} (not installed)", m.name),
            },
            author: m.author.clone(),
            version: m.version.clone(),
            number: index as i32 + 1,
            count: 0,
            collapsed: false,
            category: m.category as i32,
            mod_index: index as i32,
            mod_id: m.installed.map_or(-1, ModId::to_qml),
            moved_from: if preview {
                self.moved_from(index) as i32
            } else {
                0
            },
            trouble: shown()
                .map(|t| if t.error { 2 } else { 1 })
                .max()
                .unwrap_or(0),
            trouble_text: first.map_or_else(Sentence::default, |t| t.short.clone()),
            tags: m.tags.join(", "),
            local: m.local,
            picture,
            load_time: load_time.unwrap_or_default(),
            load_part,
            version_ignored: preview && self.version_ignored.get(index).copied().unwrap_or(false),
            outdated: made_for.is_some(),
            made_for: made_for.cloned().unwrap_or_default(),
            fresh: m.installed.is_some_and(|i| fresh.contains(&i)),
            source,
            why_moved: if preview {
                self.why_moved(index).cloned().unwrap_or_default()
            } else {
                Sentence::default()
            },
            group_size: 0,
            twin: self.twins[side_index(side)]
                .get(index)
                .copied()
                .unwrap_or(false),
        }
    }
}
