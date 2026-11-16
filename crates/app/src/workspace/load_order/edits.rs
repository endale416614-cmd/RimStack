use super::{LoadOrder, Side, rules};
use crate::App;
use rimstack_core::base::error::Error;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Category, Mod, Row, edge_of_run};
use rimstack_core::mods::rules::Rule;
use std::collections::HashMap;

impl LoadOrder {
    pub fn move_mod(&mut self, app: &App, side: Side, from: usize, to: usize) {
        let list = self.list(side);
        if from >= list.mods().len() || from == to {
            return;
        }
        self.end_preview();
        self.list_mut(side).move_mod(from, to);
        self.settle(app);
    }

    /// `None` for `to`: where its category and rules put it.
    pub fn take(&mut self, app: &App, from: Side, index: usize, to: Option<usize>) {
        self.end_preview();
        let Some(m) = self.list_mut(from).remove(index) else {
            return;
        };
        if m.installed.is_some() {
            self.put(app, from.other(), m, to);
        }
        self.settle(app);
    }

    fn put(&mut self, app: &App, into: Side, mut m: Mod, to: Option<usize>) {
        if into == Side::Inactive {
            m.category = m.filed;
            m.held_by.clear();
        }
        let at = self.list_mut(into).insert_in_category(m);
        match (into, to) {
            (_, Some(to)) => {
                let last = self.list(into).mods().len() - 1;
                self.list_mut(into).move_mod(at, to.min(last));
            }
            (Side::Active, None) => self.settle_one(app, at),
            (Side::Inactive, None) => {
                let order = self.inactive_by;
                self.inactive.arrange(order);
            }
        }
    }

    fn settle_one(&mut self, app: &App, at: usize) {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let found = rules::all_rules(app, &mut self.detected, library, self.active.mods());
        let to = rimstack_core::mods::ordering::place(&found, at, self.active.mods().len());
        self.active.move_mod(at, to);
    }

    /// `None` for `to`: each where its category and rules put it; else side by side from `to`.
    pub fn take_all(&mut self, app: &App, from: Side, ids: &[ModId], to: Option<usize>) {
        self.end_preview();
        let mut ids: Vec<ModId> = ids.to_vec();
        ids.sort_by_key(|&id| self.list(from).position_of(id));
        let into = from.other();
        self.batching = true;
        self.list_mut(from).hold();
        self.list_mut(into).hold();
        let mut placed = Vec::new();
        let mut put = 0;
        for id in ids {
            let Some(at) = self.list(from).position_of(id) else {
                continue;
            };
            let Some(m) = self.list_mut(from).remove(at) else {
                continue;
            };
            match (into, to) {
                (Side::Active, None) => {
                    self.active.insert_in_category(m);
                    placed.push(id);
                }
                _ => self.put(app, into, m, to.map(|to| to + put)),
            }
            put += 1;
        }
        self.settle_all(app, &placed);
        self.list_mut(from).let_go();
        self.list_mut(into).let_go();
        self.batching = false;
        self.settle(app);
    }

    /// Each of `placed`, in turn, where the rules put it: the rules are found once, for the
    /// whole list, which a mod at a time took 1 s for 1,862 mods (2026-11-13).
    fn settle_all(&mut self, app: &App, placed: &[ModId]) {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let mods = self.active.mods();
        let found = rules::all_rules(app, &mut self.detected, library, mods);
        let mut partners: HashMap<ModId, Vec<(ModId, bool, Rule)>> = HashMap::new();
        for rule in found {
            let (Some(first), Some(then)) = (mods[rule.first].installed, mods[rule.then].installed)
            else {
                continue;
            };
            partners.entry(first).or_default().push((then, true, rule));
            partners.entry(then).or_default().push((first, false, rule));
        }
        for &id in placed {
            let Some(at) = self.active.position_of(id) else {
                continue;
            };
            let rules: Vec<Rule> = partners
                .get(&id)
                .into_iter()
                .flatten()
                .filter_map(|(other, mine_first, rule)| {
                    let other = self.active.position_of(*other)?;
                    let (first, then) = match mine_first {
                        true => (at, other),
                        false => (other, at),
                    };
                    Some(Rule {
                        first,
                        then,
                        ..*rule
                    })
                })
                .collect();
            let to = rimstack_core::mods::ordering::place(&rules, at, self.active.mods().len());
            self.active.move_mod(at, to);
        }
    }

    /// Whether the order changed.
    pub fn move_mods(&mut self, app: &App, side: Side, ids: &[ModId], to: usize) -> bool {
        let list = self.list(side);
        let positions: Vec<usize> = ids.iter().filter_map(|&id| list.position_of(id)).collect();
        self.end_preview();
        let moved = self.list_mut(side).move_mods(&positions, to);
        if moved {
            self.settle(app);
        }
        moved
    }

    pub fn to_edge(&mut self, app: &App, ids: &[ModId], top: bool) {
        self.end_preview();
        let mut ids: Vec<ModId> = ids.to_vec();
        ids.sort_by_key(|&id| self.active.position_of(id));
        // The last one moved ends up outermost.
        if top {
            ids.reverse();
        }
        for id in ids {
            if let Some(at) = self.active.position_of(id) {
                let to = edge_of_run(self.active.mods(), at, top);
                self.active.move_mod(at, to);
            }
        }
        self.settle(app);
    }

    pub fn toggle_category(&mut self, app: &App, side: Side, category: Category) {
        self.list_mut(side).toggle(category);
        self.settle(app);
    }

    pub fn keep_folds_of(&mut self, earlier: &LoadOrder) {
        self.active.keep_folds_of(&earlier.active);
        self.inactive.keep_folds_of(&earlier.inactive);
    }

    fn sub_on(&self, side: Side, row: usize) -> Option<usize> {
        match self.list(side).rows().get(row)? {
            Row::Sub { first, .. } => Some(*first),
            _ => None,
        }
    }

    pub fn toggle_sub(&mut self, app: &App, side: Side, row: usize) {
        if let Some(first) = self.sub_on(side, row) {
            self.list_mut(side).toggle_sub(first);
            self.settle(app);
        }
    }

    pub fn group_on(&self, side: Side, row: usize) -> Option<(String, Vec<ModId>)> {
        let first = self.sub_on(side, row)?;
        let list = self.list(side);
        let tag = list.mods().get(first)?.sub.clone();
        let members = list.in_sub_of(first).into_iter();
        let ids = members
            .filter_map(|at| list.mods().get(at)?.installed)
            .collect();
        Some((tag, ids))
    }

    pub fn undo(&mut self, app: &App) -> bool {
        let stepped = app.session().undo().is_some();
        if stepped {
            self.sync(app);
        }
        stepped
    }

    pub fn redo(&mut self, app: &App) -> bool {
        let stepped = app.session().redo().is_some();
        if stepped {
            self.sync(app);
        }
        stepped
    }

    pub fn discard_changes(&mut self, app: &App) {
        let saved = app.session().saved().to_vec();
        app.session().record(saved);
        self.sync(app);
    }

    pub fn use_list(&mut self, app: &App, ids: Vec<String>) {
        app.session().record(ids);
        self.sync(app);
    }

    pub fn save(&mut self, app: &App) -> Result<(), Error> {
        let saving = self.to_save(app)?;
        app.write_list(&saving.file, &saving.written, &saving.expected)?;
        self.saved(app, saving.list);
        Ok(())
    }

    pub fn to_save(&mut self, app: &App) -> Result<Saving, Error> {
        if app.game_running() {
            return Err(Error::refused(rimstack_core::said!(
                "RimWorld is running. Close the game before saving the load order."
            )));
        }
        self.keep_sort(app);
        let file = app.paths().mods_config().ok_or_else(|| {
            Error::refused(rimstack_core::said!(
                "RimWorld's settings folder was not found."
            ))
        })?;
        let list = app.session().current().to_vec();
        // An id is empty for a folder that is not a readable mod.
        let written = list.iter().filter(|id| !id.is_empty()).cloned().collect();
        let expected = app.session().saved().to_vec();
        Ok(Saving {
            file,
            list,
            written,
            expected,
        })
    }

    /// The file's list becomes the one last read, so the next save is not refused.
    pub fn keep_file(&mut self, app: &App, take: bool) {
        let Some(on_disk) = app.mods_config_list() else {
            return;
        };
        app.session().mark_saved_as(on_disk.clone());
        self.follow_saved(app, &on_disk, app.list_written_at());
        if take {
            app.session().record(on_disk);
        }
        app.forget_list_on_disk();
        self.sync(app);
    }

    pub fn saved(&mut self, app: &App, list: Vec<String>) {
        self.follow_saved(app, &list, crate::workspace::now_secs());
        app.session().mark_saved_as(list);
        let mut mods = self.active.mods().to_vec();
        super::date(&mut mods, &self.activated);
        self.active.set_mods(mods);
        self.settle(app);
    }

    fn follow_saved(&mut self, app: &App, saved: &[String], at: u64) {
        let loaded = self.reading(app);
        if let Ok((library, _)) = &*loaded {
            self.activated = app.activated_since(library, saved, at);
        }
    }
}

pub struct Saving {
    pub file: std::path::PathBuf,
    pub list: Vec<String>,
    pub written: Vec<String>,
    pub expected: Vec<String>,
}
