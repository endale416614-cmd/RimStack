use crate::App;
use rimstack_core::base::locks::{Held, Shared};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::settings::Switch;
use rimstack_core::kept::{activated, launches};
use rimstack_core::mods::describe::Trouble;
use rimstack_core::mods::library::{Library, ModId};
pub use rimstack_core::mods::list::Side;
use rimstack_core::mods::list::offers::{self, Counted, Offers};
use rimstack_core::mods::list::{Category, GroupedList, LeftOut, Mod, SortKey, flag};
use rimstack_core::mods::{demo, sort_reasons};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

mod details;
mod edits;
mod menu;
mod orders;
mod rows;
mod rules;
mod sorting;
mod summary;

pub use details::{Details, Files, Overview, ProblemSaid, Textures};
pub use menu::{Action, Ask, Clicked, Entry};
pub use orders::{OrderEntry, Orders};
pub use rows::{Level, Origin, RowView, sort_slides};
pub use sorting::{SortGroup, SortMove};
pub use summary::SelectionSummary;

pub struct Preview {
    before: Vec<Mod>,
    grouped_before: bool,
    /// 0 if it stays.
    moved_from: Vec<usize>,
    why: Vec<Option<sort_reasons::Why>>,
}

pub struct LoadOrder {
    active: GroupedList,
    inactive: GroupedList,
    troubles: Vec<Vec<Trouble>>,
    version_ignored: Vec<bool>,
    checked_order: Vec<Option<ModId>>,
    outdated: [Vec<Option<String>>; 2],
    checked_inactive: Vec<Option<ModId>>,
    twins: [Vec<bool>; 2],
    slowest: Option<u32>,
    preview: Option<Preview>,
    batching: bool,
    headings: bool,
    /// `None` by category.
    inactive_by: Option<(SortKey, bool)>,
    activated: activated::Since,
    /// The reading its `ModId`s come from: after a rescan, `App::library` is a newer one.
    /// `None` for a made-up list or one that could not be read.
    reading: Option<std::sync::Arc<crate::workspace::Loaded>>,
    detected: rules::Detected,
    pub status: [Sentence; 2],
    /// Empty for none.
    pub problem: Sentence,
    /// A sentence each; the first is the news.
    pub sort_note: Vec<Sentence>,
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Active => 0,
        Side::Inactive => 1,
    }
}

fn date(mods: &mut [Mod], since: &activated::Since) {
    for m in mods {
        m.activated = since
            .get(&rimstack_core::rimworld::id::fold(&m.id))
            .copied();
    }
}

fn arranged(app: &App, library: &Library) -> rimstack_core::mods::library::Arranged {
    let mut now = library.arrange(app.session().current());
    let measured = app.startup();
    for m in now.active.iter_mut() {
        let took = measured.of(&rimstack_core::rimworld::id::split_steam(&m.id).0);
        m.load_micros = took.map(|ms| (ms * 1000.0) as u32);
    }
    let notes = app.notes().reading();
    let steam_record = app.steam_record();
    let (times, sizes) = (&steam_record.times, &steam_record.sizes);
    for m in now.active.iter_mut().chain(now.inactive.iter_mut()) {
        let installed = m.installed.and_then(|i| library.get(i));
        if let Some(i) = installed.filter(|i| i.source == Source::Workshop) {
            m.updated = times.get(&i.folder).copied();
            m.size = sizes.get(&i.folder).copied();
        }
        let key = installed.and_then(|i| i.key());
        if let Some(note) = key.and_then(|k| notes.get(&k)) {
            m.tags = note.tags.clone();
            m.sub = note.group().to_string();
            if let Some(chosen) = Category::from_kept(&note.category) {
                m.category = chosen;
                m.filed = chosen;
                m.chosen = true;
            }
        }
    }
    now
}

impl LoadOrder {
    fn empty() -> Self {
        Self {
            active: GroupedList::new(Vec::new()),
            inactive: GroupedList::new(Vec::new()),
            troubles: Vec::new(),
            version_ignored: Vec::new(),
            checked_order: Vec::new(),
            outdated: [Vec::new(), Vec::new()],
            checked_inactive: Vec::new(),
            twins: [Vec::new(), Vec::new()],
            slowest: None,
            preview: None,
            batching: false,
            headings: true,
            inactive_by: None,
            activated: activated::Since::new(),
            reading: None,
            detected: rules::Detected::default(),
            status: [Sentence::default(), Sentence::default()],
            problem: Sentence::default(),
            sort_note: Vec::new(),
        }
    }

    pub fn demo(active: usize, inactive: usize) -> Self {
        let mut order = Self::empty();
        order.active = GroupedList::new(demo::generate(active));
        order.inactive = GroupedList::new(demo::generate(inactive));
        order.twins = [
            rimstack_core::mods::list::twins(order.active.mods()),
            rimstack_core::mods::list::twins(order.inactive.mods()),
        ];
        order
    }

    pub fn read(app: &App, headings: bool, inactive_by: Option<(SortKey, bool)>) -> Self {
        let loaded = app.library();
        let (library, took) = match &*loaded {
            Ok(read) => read,
            Err(problem) => {
                let mut order = Self::empty();
                order.problem = problem.sentence().clone();
                return order;
            }
        };
        app.seen();
        let mut order = Self::empty();
        order.reading = Some(loaded.clone());
        order.headings = headings;
        order.inactive_by = inactive_by;
        let saved = app.session().saved().to_vec();
        order.activated = app.activated_since(library, &saved, app.list_written_at());
        let mut now = arranged(app, library);
        date(&mut now.active, &order.activated);
        let (mods, _) = rules::sorted_by_category(app, &mut order.detected, library, now.active);
        order.active = GroupedList::flat(mods);
        order.active.set_grouped(headings);
        order.inactive = GroupedList::new(now.inactive);
        order.inactive.arrange(inactive_by);
        order.follow_left_out(app);
        order.status = [
            match library.missing {
                0 => say!("{} active", library.active.len()),
                n => say!("{} active, {} not installed", library.active.len(), n),
            },
            say!(
                "{} inactive, read in {} ms",
                library.inactive.len(),
                took.as_millis()
            ),
        ];
        order.settle(app);
        order
    }

    pub fn is_installed(&self) -> bool {
        self.reading.is_some()
    }

    /// The order's own reading; a made-up list has none and is given the current one, or none
    /// while the mods are first read: the window never waits on that.
    fn reading(&self, app: &App) -> std::sync::Arc<crate::workspace::Loaded> {
        self.reading
            .clone()
            .or_else(|| app.library_if_read())
            .unwrap_or_else(|| std::sync::Arc::new(Ok(Default::default())))
    }

    pub fn list(&self, side: Side) -> &GroupedList {
        match side {
            Side::Active => &self.active,
            Side::Inactive => &self.inactive,
        }
    }

    fn list_mut(&mut self, side: Side) -> &mut GroupedList {
        match side {
            Side::Active => &mut self.active,
            Side::Inactive => &mut self.inactive,
        }
    }

    pub fn troubles(&self) -> &[Vec<Trouble>] {
        &self.troubles
    }

    pub fn needed(&self) -> Vec<String> {
        rimstack_core::mods::describe::tally(&self.troubles).needed
    }

    pub fn previewing(&self) -> bool {
        self.preview.is_some()
    }

    pub fn headings(&self) -> bool {
        self.headings
    }

    pub fn inactive_by(&self) -> Option<(SortKey, bool)> {
        self.inactive_by
    }

    pub fn order_of(&self, side: Side) -> Option<(SortKey, bool)> {
        match side {
            Side::Active => self.active.shown_by(),
            Side::Inactive => self.inactive_by,
        }
    }

    pub fn errors_and_warnings(&self) -> (usize, usize) {
        let with = |error: bool| {
            let has = |own: &&Vec<Trouble>| own.iter().any(|t| t.error == error);
            self.troubles.iter().filter(has).count()
        };
        (with(true), with(false))
    }

    /// `None` when what was read still holds.
    pub fn files_to_read(&self, app: &App) -> Option<Vec<ModId>> {
        if !self.is_installed() {
            return None;
        }
        let mut members: Vec<ModId> = self.checked_order.iter().flatten().copied().collect();
        members.sort_unstable();
        members.dedup();
        let known = app.conflicts().reading();
        let read = known.as_ref().is_some_and(|c| c.members == members);
        (!read).then_some(members)
    }

    pub fn settle(&mut self, app: &App) {
        self.twins = [
            rimstack_core::mods::list::twins(self.active.mods()),
            rimstack_core::mods::list::twins(self.inactive.mods()),
        ];
        self.slowest = self
            .active
            .mods()
            .iter()
            .filter_map(|m| m.load_micros)
            .max();
        if !self.is_installed() {
            return;
        }
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let ids = |list: &GroupedList| -> Vec<Option<ModId>> {
            list.mods().iter().map(|m| m.installed).collect()
        };
        let (active, inactive) = (ids(&self.active), ids(&self.inactive));
        let changed = [
            active != self.checked_order,
            inactive != self.checked_inactive,
        ];
        if changed.contains(&true) {
            let trust = rules::Trust::read(app, library);
            if changed[0] {
                let mods = self.active.mods();
                (self.troubles, self.version_ignored) = rules::troubles(app, library, mods, &trust);
                self.outdated[0] = rules::outdated(library, mods, &trust);
                self.checked_order = active;
            }
            if changed[1] {
                self.outdated[1] = rules::outdated(library, self.inactive.mods(), &trust);
                self.checked_inactive = inactive;
            }
        }
        if self.preview.is_none() && !self.batching {
            let ids = self.active.mods().iter().map(|m| m.id.clone()).collect();
            app.session().record(ids);
        }
        for side in [Side::Active, Side::Inactive] {
            let flags = self.flags(app, library, side);
            self.list_mut(side).set_flags(flags);
        }
        self.share_facts(app);
    }

    fn share_facts(&mut self, app: &App) {
        let facts = app.mod_facts(&self.reading(app));
        self.active.set_facts(facts.clone());
        self.inactive.set_facts(facts);
    }

    pub fn facts_read(&mut self, app: &App) {
        self.share_facts(app);
    }

    pub fn needs_facts(&self) -> bool {
        self.active.needs_facts() || self.inactive.needs_facts()
    }

    pub fn filter_summary(&self) -> Sentence {
        let (active, inactive) = (self.active.matching(), self.inactive.matching());
        match self.active.unread() + self.inactive.unread() {
            0 => say!("{} active, {} inactive", active, inactive),
            n => say_n!(
                n,
                "{} mod not read yet: {} active, {} inactive",
                "{} mods not read yet: {} active, {} inactive",
                active,
                inactive
            ),
        }
    }

    pub fn filter_offers(&self, typed: &str, text: &str, counted: &Counted) -> Offers {
        let mods = self.active.mods().iter().chain(self.inactive.mods());
        offers::offers(typed, text, counted, mods)
    }

    pub fn recheck(&mut self, app: &App) {
        self.checked_order = Vec::new();
        self.checked_inactive = Vec::new();
        self.settle(app);
    }

    pub fn sync(&mut self, app: &App) {
        if !self.is_installed() {
            return;
        }
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        self.preview = None;
        self.sort_note.clear();
        let mut now = arranged(app, library);
        date(&mut now.active, &self.activated);
        let (mods, _) = rules::sorted_by_category(app, &mut self.detected, library, now.active);
        self.active.set_grouped(self.headings);
        self.active.set_mods(mods);
        self.inactive.set_mods(now.inactive);
        self.inactive.arrange(self.inactive_by);
        self.reading = Some(loaded.clone());
        self.settle(app);
    }

    pub fn refile(&mut self, app: &App) {
        let loaded = self.reading(app);
        let (Ok((library, _)), None) = (&*loaded, &self.preview) else {
            return;
        };
        if !self.is_installed() {
            return;
        }
        let mods = self.active.mods().to_vec();
        let (mods, _) = rules::sorted_by_category(app, &mut self.detected, library, mods);
        self.active.set_mods(mods);
        self.settle(app);
    }

    pub fn set_headings(&mut self, app: &App, headings: bool) {
        self.headings = headings;
        if self.active.is_grouped() != headings {
            self.active.set_grouped(headings);
            self.settle(app);
        }
    }

    pub fn set_inactive_by(&mut self, app: &App, order: Option<(SortKey, bool)>) {
        self.inactive_by = order;
        self.inactive.arrange(order);
        self.settle(app);
    }

    /// The settings the lists and Sort read: a change to any other draws them as they were.
    pub fn settings_read(app: &App) -> Vec<String> {
        let settings = app.settings();
        let switches = [
            Switch::DimLeftOut,
            Switch::CommunityRules,
            Switch::FileHints,
            Switch::VersionWarnings,
        ];
        let mut read: Vec<String> = switches
            .into_iter()
            .map(|switch| settings.switch(switch).to_string())
            .collect();
        for folder in ["game", "config", "workshop"] {
            read.push(settings.get(folder).unwrap_or_default().to_string());
        }
        read
    }

    pub fn follow_left_out(&mut self, app: &App) {
        let left_out = match app.settings().switch(Switch::DimLeftOut) {
            true => LeftOut::Dimmed,
            false => LeftOut::Hidden,
        };
        self.active.set_left_out(left_out);
        self.inactive.set_left_out(left_out);
    }

    pub fn set_filter(&mut self, text: &str) {
        self.active.set_filter_on(Side::Active, text);
        self.inactive.set_filter_on(Side::Inactive, text);
    }

    /// `None` shows the load order itself.
    pub fn set_active_by(&mut self, order: Option<(SortKey, bool)>) {
        self.active.show_by(order);
    }

    fn flags(&self, app: &App, library: &Library, side: Side) -> Vec<u16> {
        let in_conflict = self.in_conflict(app);
        let behind = app.behind().reading();
        let mut copies: HashMap<String, usize> = HashMap::new();
        for key in library.installed.iter().filter_map(|m| m.key()) {
            *copies.entry(key).or_default() += 1;
        }
        let week_ago = super::now_secs().saturating_sub(7 * 24 * 3600);
        let newest_save = app.newest_save();
        let in_save = newest_save
            .as_ref()
            .as_ref()
            .filter(|_| side == Side::Active);
        let steam_record = app.steam_record();
        let found = app.find_mods();
        let seen = app.seen().reading();
        let session = app.session();
        let changes = session.changes();
        let changed: HashSet<&str> = match side {
            Side::Active => changes
                .moved
                .iter()
                .chain(&changes.activated)
                .copied()
                .collect(),
            Side::Inactive => HashSet::new(),
        };
        let troubles: &[Vec<Trouble>] = match side {
            Side::Active => &self.troubles,
            Side::Inactive => &[],
        };
        let mods = self.list(side).mods().iter().enumerate();
        mods.map(|(at, m)| {
            let own = troubles.get(at).map(Vec::as_slice).unwrap_or_default();
            let installed = m.installed.and_then(|i| library.get(i));
            let folder = installed.map(|i| &i.folder);
            let key = rimstack_core::rimworld::id::split_steam(&m.id).0;
            let marks = [
                (own.iter().any(|t| t.error), flag::ERROR),
                (own.iter().any(|t| !t.error), flag::WARNING),
                (
                    m.installed.is_some_and(|i| in_conflict.contains(&i)),
                    flag::CONFLICT,
                ),
                (
                    folder.is_some_and(|f| behind.as_ref().is_some_and(|b| b.contains(f))),
                    flag::UPDATED,
                ),
                (
                    in_save.is_some_and(|ids| !ids.contains(&key)),
                    flag::NOT_IN_SAVE,
                ),
                (
                    folder.is_some_and(|f| {
                        steam_record.times.get(f).is_some_and(|at| *at >= week_ago)
                    }),
                    flag::RECENT,
                ),
                (copies.get(&key).is_some_and(|n| *n > 1), flag::DUPLICATE),
                (
                    installed.is_some_and(|i| seen.is_new(&i.path.to_string_lossy())),
                    flag::NEW,
                ),
                (changed.contains(m.id.as_str()), flag::CHANGED),
                (
                    m.installed.is_some_and(|i| found.contains(&i)),
                    flag::IN_SEARCH,
                ),
            ];
            marks
                .iter()
                .filter(|(on, _)| *on)
                .fold(0, |flags, (_, mark)| flags | mark)
        })
        .collect()
    }

    pub fn in_conflict(&self, app: &App) -> HashSet<ModId> {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return HashSet::new();
        };
        (0..known.members.len())
            .filter(|&at| !known.overlaps.with(at).is_empty())
            .map(|at| known.members[at])
            .collect()
    }

    pub fn fresh(&self, app: &App, side: Side) -> Vec<ModId> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        if !self.list(side).mods().iter().any(|m| m.installed.is_some()) {
            return Vec::new();
        }
        let seen = app.seen().reading();
        let mods = self.list(side).mods().iter();
        mods.filter_map(|m| m.installed)
            .filter(|&i| {
                library
                    .get(i)
                    .is_some_and(|m| seen.is_new(&m.path.to_string_lossy()))
            })
            .collect()
    }
}

impl App {
    /// Since when each mod of `saved` has been in the saved list; one that joined since it was
    /// last asked is dated `at`. A list the game emptied to its own mods dates nothing.
    pub fn activated_since(
        &self,
        library: &Library,
        saved: &[String],
        at: u64,
    ) -> activated::Since {
        static KEEPING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _held = KEEPING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let file = self.activated_file();
        let saved: Vec<String> = saved.iter().filter(|id| !id.is_empty()).cloned().collect();
        let launched = launches::read(&self.launches_file());
        let emptied = launched
            .last()
            .is_some_and(|last| super::game::emptied_by_game(library, &saved, last));
        let kept = self
            .activated_kept
            .held()
            .clone()
            .or_else(|| activated::read(&file));
        if emptied {
            return kept.unwrap_or_default();
        }
        let (mut since, mut changed) = kept.map_or_else(
            || (activated::first(&saved, &launched, at), true),
            |kept| (kept, false),
        );
        changed |= activated::follow(&mut since, &saved, at);
        if changed {
            *self.activated_kept.held() = Some(since.clone());
            self.activated_writes
                .ask("activated.json written", since.clone(), move |since| {
                    if let Err(problem) = activated::write(&file, &since) {
                        rimstack_core::problem!("when mods were activated was not kept: {problem}");
                    }
                });
        }
        since
    }

    /// When ModsConfig.xml was last written; now without one.
    pub fn list_written_at(&self) -> u64 {
        let file = self.paths().mods_config();
        match file.map_or(0, |f| rimstack_core::base::watch::changed_at(&f)) {
            0 => super::now_secs(),
            millis => millis / 1000,
        }
    }

    /// Off the window's thread.
    pub fn read_active_files(&self, members: Vec<ModId>) {
        use rayon::prelude::*;
        use rimstack_core::kept::mod_facts::ModFacts;
        use rimstack_core::mods::conflicts;
        let generation = self.conflicts_generation();
        let run = self.begin_facts_run();
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let active: HashSet<String> = members
            .iter()
            .filter_map(|&i| library.get(i)?.key())
            .collect();
        let jobs: Vec<super::mod_facts::Job> = members
            .iter()
            .map(|&i| {
                let root = library.get(i).map(|m| m.path.clone()).unwrap_or_default();
                let loaded = rimstack_core::rimworld::loadfolders::folders(
                    &root,
                    &library.game_version,
                    &|m| active.contains(m),
                );
                super::mod_facts::Job {
                    root,
                    loaded,
                    whole: true,
                }
            })
            .collect();
        let mut record = ModFacts::read(&self.facts_file(), &library.game_version);
        let (facts, read) = super::mod_facts::gather(&mut record, &jobs, &library.game_assemblies);
        self.count_mods_read(read);
        let provided: Vec<Vec<String>> = jobs
            .par_iter()
            .map(|job| conflicts::provided(&job.loaded))
            .collect();
        let scanned = members
            .iter()
            .copied()
            .zip(facts.iter().map(|f| f.scanned.clone()));
        let found = super::Conflicts {
            overlaps: conflicts::overlaps(&provided),
            scanned: scanned.collect(),
            members: members.clone(),
        };
        if !self.keep_facts(run, &loaded, members.iter().copied().zip(facts)) {
            return;
        }
        self.keep_files_read(generation, found);
        let installed: HashSet<String> = library
            .ids()
            .map(|(_, m)| m.path.to_string_lossy().into_owned())
            .collect();
        if read > 0 {
            self.keep_record(run, &mut record, &installed);
        }
        self.read_inactive_facts(run, &loaded, &mut record, &members);
    }

    /// Dropped when `textures_changed` ran since `generation`.
    fn keep_files_read(&self, generation: u64, found: super::Conflicts) {
        let mut slot = self.conflicts().writing();
        if self.conflicts_generation() != generation {
            return;
        }
        *slot = Some(found);
        drop(slot);
        self.publish(crate::changes::Changes::FILES_READ);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMod {
    pub name: String,
    pub mod_id: ModId,
    pub active: bool,
}

impl LoadOrder {
    /// A mod's package id as a mod lists it among others, lower case; `None` for one not installed.
    pub fn package_of(&self, app: &App, id: ModId) -> Option<String> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return None;
        };
        library.get(id)?.key()
    }

    /// The mod of `side` with this package id, as `package_of` gives it.
    pub fn with_package(&self, app: &App, side: Side, key: &str) -> Option<ModId> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return None;
        };
        let has = |id: &ModId| {
            library
                .get(*id)
                .and_then(|m| m.key())
                .is_some_and(|k| k == key)
        };
        self.list(side)
            .mods()
            .iter()
            .filter_map(|m| m.installed)
            .find(has)
    }

    pub fn found(&self, app: &App, text: &str, most: usize) -> Vec<FoundMod> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let active = self.list(Side::Active);
        library
            .find(text, most)
            .into_iter()
            .filter_map(|id| {
                Some(FoundMod {
                    name: library.get(id)?.display_name().to_string(),
                    mod_id: id,
                    active: active.position_of(id).is_some(),
                })
            })
            .collect()
    }

    pub fn same_mods_in(&self, newer: &LoadOrder) -> HashMap<ModId, ModId> {
        let (Some(old), Some(new)) = (self.held(), newer.held()) else {
            return HashMap::new();
        };
        let by_folder: HashMap<&std::path::Path, ModId> =
            new.ids().map(|(id, m)| (m.path.as_path(), id)).collect();
        let found = |(id, m): (ModId, &rimstack_core::mods::library::Installed)| {
            Some((id, *by_folder.get(m.path.as_path())?))
        };
        old.ids().filter_map(found).collect()
    }

    fn held(&self) -> Option<&Library> {
        self.reading.as_deref()?.as_ref().ok().map(|(l, _)| l)
    }

    /// `selected` of the earlier reading, in this one: what is gone is left out.
    pub fn carried(&self, same: &HashMap<ModId, ModId>, selected: &[ModId]) -> Carried {
        let kept: Vec<ModId> = selected
            .iter()
            .filter_map(|id| same.get(id).copied())
            .collect();
        let active = kept
            .first()
            .is_some_and(|&first| self.active.position_of(first).is_some());
        Carried::of(&kept, active)
    }

    pub fn inverted(&self, side: Side, selected_on: Side, selected: &[ModId]) -> Carried {
        let selected = if selected_on == side { selected } else { &[] };
        Carried::of(
            &self.list(side).shown_except(selected),
            side == Side::Active,
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct Carried {
    pub ids: Vec<ModId>,
    pub active: bool,
}

impl Carried {
    fn of(ids: &[ModId], active: bool) -> Carried {
        Carried {
            ids: ids.to_vec(),
            active: active && !ids.is_empty(),
        }
    }
}

/// What a drag of one mod takes along: the whole selection when it is part of it.
#[derive(Debug, Serialize, PartialEq)]
pub struct Holding {
    pub ids: Vec<ModId>,
    pub name: Sentence,
}

impl Holding {
    pub fn of(id: Option<ModId>, name: &str, selected: &[ModId]) -> Holding {
        match id {
            Some(id) if selected.len() > 1 && selected.contains(&id) => Holding {
                ids: selected.to_vec(),
                name: say_n!(selected.len(), "{} mod", "{} mods"),
            },
            _ => Holding {
                ids: id.into_iter().collect(),
                name: Sentence::Plain(name.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests;
