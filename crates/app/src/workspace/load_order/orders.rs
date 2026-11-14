use super::{LoadOrder, Side};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::columns;
use rimstack_core::mods::list::{Mod, SortKey};
use rimstack_core::rimworld::startup;
use rimstack_core::say;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Orders {
    pub active: Vec<OrderEntry>,
    pub active_by: BTreeMap<&'static str, OrderEntry>,
    /// "" for the load order itself.
    pub active_key: &'static str,
    pub inactive: Vec<OrderEntry>,
    pub inactive_by: BTreeMap<&'static str, OrderEntry>,
    pub inactive_key: &'static str,
    pub inactive_said: Sentence,
    /// The widest text of each list's name, author, version and load columns, a first author
    /// only.
    pub widest: BTreeMap<&'static str, String>,
    pub inactive_widest: BTreeMap<&'static str, String>,
}

impl Orders {
    pub fn by(&self, side: Side) -> &BTreeMap<&'static str, OrderEntry> {
        match side {
            Side::Active => &self.active_by,
            Side::Inactive => &self.inactive_by,
        }
    }

    pub fn key(&self, side: Side) -> &'static str {
        match side {
            Side::Active => self.active_key,
            Side::Inactive => self.inactive_key,
        }
    }

    pub fn widest(&self, side: Side) -> &BTreeMap<&'static str, String> {
        match side {
            Side::Active => &self.widest,
            Side::Inactive => &self.inactive_widest,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OrderEntry {
    /// A `SortKey::key`; "" for the list's own order.
    pub key: &'static str,
    /// With its way when it is the order shown.
    pub label: Sentence,
    /// "check" on the order shown, "" on the others.
    pub icon: &'static str,
    /// Shown by a key, not the list's own order.
    pub on: bool,
    /// The order once this one is picked, as `columns::kept` writes it.
    pub then: String,
}

fn label(key: Option<SortKey>, side: Side) -> Sentence {
    match key {
        None if side == Side::Active => say!("Load order"),
        None => say!("Category"),
        Some(SortKey::Group) => say!("Group"),
        Some(SortKey::Name) => say!("Name"),
        Some(SortKey::Author) => say!("Author"),
        Some(SortKey::Version) => say!("Version"),
        Some(SortKey::PackageId) => say!("Package id"),
        Some(SortKey::Updated) => say!("Updated"),
        Some(SortKey::Downloaded) => say!("Downloaded"),
        Some(SortKey::Activated) => say!("Activated"),
        Some(SortKey::Size) => say!("Size"),
        Some(SortKey::Load) => say!("Load"),
    }
}

fn said(order: Option<(SortKey, bool)>, side: Side) -> Sentence {
    let named = label(order.map(|(key, _)| key), side);
    let Some((_, descending)) = order.filter(|(key, _)| key.turns()) else {
        return named;
    };
    let way = if descending { "▾" } else { "▴" };
    Sentence::joined(" ", vec![named, Sentence::Plain(way.to_string())])
}

fn entries(now: Option<(SortKey, bool)>, side: Side, keys: &[SortKey]) -> Vec<OrderEntry> {
    let each = std::iter::once(None).chain(keys.iter().copied().map(Some));
    each.map(|key| {
        let on = now.map(|(by, _)| by) == key;
        OrderEntry {
            key: key.map_or("", SortKey::key),
            label: if on {
                said(now, side)
            } else {
                label(key, side)
            },
            icon: if on { "check" } else { "" },
            on: on && key.is_some(),
            then: columns::kept(columns::pressed(now, key)),
        }
    })
    .collect()
}

impl LoadOrder {
    pub fn orders(&self) -> Orders {
        let timed = self.slowest.is_some();
        let keys = SortKey::ALL.into_iter();
        let active_keys: Vec<SortKey> = keys
            .clone()
            .filter(|k| *k != SortKey::Load || timed)
            .collect();
        let inactive_keys: Vec<SortKey> = keys
            .filter(|k| !matches!(k, SortKey::Group | SortKey::Activated | SortKey::Load))
            .collect();
        let active = entries(self.order_of(Side::Active), Side::Active, &active_keys);
        let inactive = entries(
            self.order_of(Side::Inactive),
            Side::Inactive,
            &inactive_keys,
        );
        let by = |entries: &[OrderEntry]| entries.iter().map(|e| (e.key, e.clone())).collect();
        let key = |side| self.order_of(side).map_or("", |(key, _)| key.key());
        Orders {
            active_by: by(&active),
            active_key: key(Side::Active),
            active,
            inactive_by: by(&inactive),
            inactive_key: key(Side::Inactive),
            inactive,
            inactive_said: said(self.inactive_by, Side::Inactive),
            widest: self.widest(Side::Active),
            inactive_widest: self.widest(Side::Inactive),
        }
    }

    fn widest(&self, side: Side) -> BTreeMap<&'static str, String> {
        let mods = self.list(side).mods();
        let widest = |texts: Vec<String>| {
            let most = texts.into_iter().max_by_key(|t| t.chars().count());
            most.unwrap_or_default()
        };
        let first_author = |m: &Mod| m.author.split(", ").next().unwrap_or_default().to_string();
        let seconds = |micros: u32| startup::seconds(f64::from(micros) / 1000.0);
        BTreeMap::from([
            (
                "name",
                widest(mods.iter().map(|m| m.name.clone()).collect()),
            ),
            ("author", widest(mods.iter().map(first_author).collect())),
            (
                "version",
                widest(mods.iter().map(|m| m.version.clone()).collect()),
            ),
            (
                "load",
                widest(
                    mods.iter()
                        .filter_map(|m| m.load_micros)
                        .map(seconds)
                        .collect(),
                ),
            ),
        ])
    }
}
