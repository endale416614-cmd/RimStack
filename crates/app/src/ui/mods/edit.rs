use super::Mods;
use crate::changes::Changes;
use crate::ui::notices;
use crate::words::put;
use crate::workspace::load_order::Side;
use gpui::{Context, KeyDownEvent, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::{say, say_n};

pub type AfterSave = Box<dyn FnOnce(&mut Mods, &mut Context<Mods>)>;

impl Mods {
    pub(super) fn changed(&mut self, cx: &mut Context<Self>) {
        self.follow_selection(cx);
        self.show(cx);
        if !self.demo {
            super::super::app(cx).publish(Changes::LOAD_ORDER);
        }
    }

    pub(super) fn done_said(&self, said: Sentence, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let me = cx.entity().downgrade();
        notices::offer(
            put(&said),
            put(&say!("Undo")),
            move |_, cx| {
                let _ = me.update(cx, |mods, cx| mods.undo(cx));
            },
            cx,
        );
    }

    fn ids(&self, cx: &gpui::App) -> Vec<ModId> {
        self.list.read(cx).selection.clone()
    }

    pub(super) fn move_mod(&mut self, side: Side, from: usize, to: usize, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        let moved = self
            .list
            .read(cx)
            .order
            .list(side)
            .mods()
            .get(from)
            .and_then(|m| m.installed);
        self.edit_list(cx, |list, _| list.order.move_mod(&app, side, from, to));
        self.changed(cx);
        if let Some(id) = moved {
            let slides = [(id, from as i32 - to as i32)].into();
            self.list(side)
                .update(cx, |list, cx| list.slide(slides, cx));
        }
    }

    pub(super) fn take(
        &mut self,
        from: Side,
        index: usize,
        to: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self
            .list
            .read(cx)
            .order
            .list(from)
            .mods()
            .get(index)
            .map(|m| m.name.clone())
        else {
            return;
        };
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.take(&app, from, index, to));
        self.changed(cx);
        let said = match from {
            Side::Inactive => say!("Activated {}", name),
            Side::Active => say!("Deactivated {}", name),
        };
        self.done_said(said, cx);
    }

    fn take_ids(&mut self, from: Side, ids: &[ModId], to: Option<usize>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.take_all(&app, from, ids, to));
        self.changed(cx);
        let said = match from {
            Side::Inactive => say_n!(ids.len(), "Activated {} mod", "Activated {} mods"),
            Side::Active => say_n!(ids.len(), "Deactivated {} mod", "Deactivated {} mods"),
        };
        self.done_said(said, cx);
    }

    pub fn move_selected(&mut self, cx: &mut Context<Self>) {
        let (from, ids) = (Side::of(self.list.read(cx).selected_active), self.ids(cx));
        self.take_ids(from, &ids, None, cx);
        self.select(None, true, cx);
    }

    pub(super) fn activate_selected(&mut self, side: Side, cx: &mut Context<Self>) {
        let Some(first) = self.selected_id(cx) else {
            return;
        };
        let state = self.list.read(cx);
        let list = state.order.list(side);
        let (Some(position), false) = (list.position_of(first), state.order.previewing()) else {
            return;
        };
        let next = list.beside_mod(first);
        self.take(side, position, None, cx);
        if let Some(next) = next {
            self.select(Some(next), side == Side::Active, cx);
        }
    }

    pub(super) fn toggle_mod(&mut self, side: Side, mod_id: ModId, cx: &mut Context<Self>) {
        let state = self.list.read(cx);
        let position = state.order.list(side).position_of(mod_id);
        if let (Some(position), false) = (position, state.order.previewing()) {
            self.take(side, position, None, cx);
        }
    }

    /// A drop: `held` (more than one) move together, one alone moves from `index`; `to` is the
    /// place dropped on, the list's length for its end.
    pub(super) fn drop_on(
        &mut self,
        (from, index, held): (Side, usize, Vec<ModId>),
        (into, to): (Side, usize),
        cx: &mut Context<Self>,
    ) {
        match (held.len() > 1, from == into) {
            (true, true) => {
                let app = super::super::app(cx);
                if self.edit_list(cx, |list, _| list.order.move_mods(&app, from, &held, to)) {
                    self.changed(cx);
                    self.done_said(say_n!(held.len(), "Moved {} mod", "Moved {} mods"), cx);
                }
            }
            (true, false) => self.take_ids(from, &held, Some(to), cx),
            (false, true) => {
                let last = self.list.read(cx).count(from).saturating_sub(1);
                let place = to.min(last);
                if index != place {
                    self.move_mod(from, index, place, cx);
                }
            }
            (false, false) => self.take(from, index, Some(to), cx),
        }
    }

    fn nudge(&mut self, side: Side, by: isize, cx: &mut Context<Self>) {
        let Some(first) = self.selected_id(cx) else {
            return;
        };
        let sorted_by_column = side == Side::Active && !self.orders.active_key.is_empty();
        if self.list.read(cx).order.previewing() || sorted_by_column {
            return;
        }
        if let Some((from, to)) = self.list.read(cx).order.list(side).nudged(first, by) {
            self.move_mod(side, from, to, cx);
            self.reveal_selected(side, false, cx);
        }
    }

    pub fn to_edge(&mut self, top: bool, cx: &mut Context<Self>) {
        let ids = self.ids(cx);
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.to_edge(&app, &ids, top));
        self.changed(cx);
        self.done_said(say_n!(ids.len(), "Moved {} mod", "Moved {} mods"), cx);
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        if !self.list.read(cx).previewing() && self.edit_list(cx, |list, _| list.order.undo(&app)) {
            self.changed(cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        if !self.list.read(cx).previewing() && self.edit_list(cx, |list, _| list.order.redo(&app)) {
            self.changed(cx);
        }
    }

    pub(super) fn discard_changes(&mut self, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.discard_changes(&app));
        self.changed(cx);
        self.done_said(say!("Back to the saved list"), cx);
    }

    /// The list on disk taken in place of this one, or this one kept to overwrite it.
    pub fn keep_file(&mut self, take: bool, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.keep_file(&app, take));
        self.status.save_problem.clear();
        self.status.file_changed = false;
        self.changed(cx);
    }

    /// Writes the load order to ModsConfig.xml off the window's thread; `then` runs once written.
    pub fn save(&mut self, then: Option<AfterSave>, cx: &mut Context<Self>) {
        if self.list.read(cx).saving || self.demo {
            return;
        }
        if self.list.read(cx).previewing() {
            self.end_sort(true, cx);
        }
        let app = super::super::app(cx);
        let saving = match self.list.update(cx, |list, _| list.order.to_save(&app)) {
            Ok(saving) => saving,
            Err(problem) => {
                self.status.save_problem = crate::words::said_of(&problem);
                cx.notify();
                return;
            }
        };
        let changes = app.session().changes().said();
        let before = app.session().saved().to_vec();
        self.edit_list(cx, |list, _| list.saving = true);
        let worker = app.clone();
        crate::jobs::run(
            "save",
            cx,
            move || {
                let snapshot = worker.snapshot("@saving", &before);
                let written = worker.write_list(&saving.file, &saving.written, &saving.expected);
                if written.is_ok() {
                    let said = say!("Saved the load order: {}", changes.inside());
                    worker.note_change(said, snapshot.as_deref());
                }
                (saving.list, written)
            },
            move |mods, (list, written), cx| {
                mods.edit_list(cx, |state, _| {
                    state.saving = false;
                    if written.is_ok() {
                        state.order.saved(&app, list);
                    }
                });
                mods.status.save_problem = crate::words::why_not(&written);
                mods.status.file_changed =
                    written.as_ref().err() == Some(&modsconfig::changed_outside());
                mods.changed(cx);
                if let (Ok(()), Some(then)) = (written, then) {
                    then(mods, cx);
                }
            },
        );
        cx.notify();
    }

    pub fn preview_sort(&mut self, cx: &mut Context<Self>) {
        let state = self.list.read(cx);
        if state.previewing() || !state.files_read || self.demo {
            return;
        }
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.preview_sort(&app));
        self.status.closed_note.clear();
        self.show(cx);
        let slides = crate::workspace::load_order::sort_slides(&self.active.rows);
        self.list(Side::Active)
            .update(cx, |list, cx| list.slide(slides, cx));
        let order = &self.list.read(cx).order;
        if let Some(position) = order.first_moved()
            && let Some(row) = order.list(Side::Active).row_of(position)
        {
            self.list(Side::Active)
                .update(cx, |list, cx| list.reveal(row, cx));
        }
    }

    pub(super) fn end_sort(&mut self, keep: bool, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        if !keep {
            self.edit_list(cx, |list, _| list.order.drop_sort(&app));
            return self.show(cx);
        }
        let Some(sorted) = self.edit_list(cx, |list, _| list.order.keep_sort_noted(&app)) else {
            return;
        };
        self.changed(cx);
        self.done_said(sorted, cx);
    }
}

impl Mods {
    pub fn select(&mut self, mod_id: Option<ModId>, in_active: bool, cx: &mut Context<Self>) {
        self.keep_selection(mod_id.into_iter().collect(), in_active, cx);
    }

    pub(super) fn select_also(&mut self, mod_id: ModId, in_active: bool, cx: &mut Context<Self>) {
        let first = self
            .selected_id(cx)
            .filter(|_| self.list.read(cx).selected_active == in_active);
        let Some(first) = first else {
            return self.select(Some(mod_id), in_active, cx);
        };
        if mod_id == first {
            return;
        }
        let mut ids = self.ids(cx);
        match ids.iter().position(|held| *held == mod_id) {
            Some(at) => drop(ids.remove(at)),
            None => ids.push(mod_id),
        }
        self.keep_selection(ids, in_active, cx);
    }

    pub(super) fn select_through(&mut self, to: ModId, in_active: bool, cx: &mut Context<Self>) {
        let first = self
            .selected_id(cx)
            .filter(|_| self.list.read(cx).selected_active == in_active);
        let Some(first) = first else {
            return self.select(Some(to), in_active, cx);
        };
        let list = self.list.read(cx).order.list(Side::of(in_active));
        let Some(others) = list.others_through(first, to) else {
            return;
        };
        let ids = std::iter::once(first).chain(others).collect();
        self.keep_selection(ids, in_active, cx);
    }

    pub(super) fn invert_selection(&mut self, in_active: bool, cx: &mut Context<Self>) {
        let state = self.list.read(cx);
        let (side, on) = (Side::of(in_active), Side::of(state.selected_active));
        let inverted = state.order.inverted(side, on, &state.selection);
        self.keep_selection(inverted.ids, in_active, cx);
    }

    pub(super) fn select_all(&mut self, side: Side, cx: &mut Context<Self>) {
        let ids = self
            .shown(side)
            .rows
            .iter()
            .filter(|r| !r.is_header)
            .filter_map(|r| r.mod_id)
            .collect();
        self.keep_selection(ids, side == Side::Active, cx);
    }

    /// The first chosen first, then the others by id, as `Selection::of` kept them.
    pub(super) fn keep_selection(
        &mut self,
        ids: Vec<ModId>,
        in_active: bool,
        cx: &mut Context<Self>,
    ) {
        let mut ids = ids.into_iter();
        let first = ids.next();
        let mut others: Vec<ModId> = ids.filter(|id| Some(*id) != first).collect();
        others.sort();
        others.dedup();
        self.edit_list(cx, |list, _| {
            list.selection = first.into_iter().chain(others).collect();
            list.selected_active = in_active;
        });
        self.describe(cx);
        self.sync_lists(cx);
        cx.notify();
    }

    fn follow_selection(&mut self, cx: &mut Context<Self>) {
        let state = self.list.read(cx);
        let Some(first) = state.selected_id() else {
            return;
        };
        let on = |side| state.order.list(side).position_of(first).is_some();
        let active = match (on(Side::Active), on(Side::Inactive)) {
            (true, _) => true,
            (_, true) => false,
            _ => return,
        };
        self.edit_list(cx, |list, _| list.selected_active = active);
    }

    pub(super) fn reveal_selected(&mut self, side: Side, centre: bool, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let rows = self.shown(side).rows.clone();
        if let Some(at) = rows
            .iter()
            .position(|r| r.mod_id == Some(id) && !r.is_header)
        {
            self.list(side).update(cx, |list, cx| match centre {
                true => list.reveal(at, cx),
                false => list.contain(at, cx),
            });
        }
    }

    /// The arrows step through the list and Alt moves the chosen mod; Enter moves it across;
    /// Ctrl + A chooses every mod and Ctrl + I the others.
    pub(super) fn list_key(
        &mut self,
        side: Side,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = &event.keystroke;
        let bare = !keys.modifiers.modified();
        let (alt, command) = (keys.modifiers.alt, keys.modifiers.secondary(cx));
        let by = match keys.key.as_str() {
            "up" => -1,
            "down" => 1,
            _ => 0,
        };
        match keys.key.as_str() {
            "up" | "down" if alt => self.nudge(side, by, cx),
            "up" | "down" => {
                let current = self
                    .selected_id(cx)
                    .filter(|_| self.list.read(cx).selected_active == (side == Side::Active));
                let row = self.list.read(cx).order.list(side).step_from(current, by);
                let id = row
                    .and_then(|row| self.shown(side).rows.get(row))
                    .and_then(|r| r.mod_id);
                if let Some(id) = id {
                    self.select(Some(id), side == Side::Active, cx);
                    self.reveal_selected(side, false, cx);
                }
            }
            "enter" => self.activate_selected(side, cx),
            "a" if command => self.select_all(side, cx),
            "i" if command => self.invert_selection(side == Side::Active, cx),
            "t" | "n" if bare && self.details.is_some() => {
                let tags = keys.key == "t";
                self.details_view
                    .update(cx, |view, cx| view.edit_notes(tags, window, cx));
            }
            "escape" if self.chosen_reads.picking.is_some() => self.start_picking(None, cx),
            "escape" if !self.list.read(cx).selection.is_empty() => self.select(None, true, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Mods {
    /// Which of the mods with a problem the arrows last chose, from 1, while it stays chosen.
    pub(super) fn problem_place(&self, cx: &gpui::App) -> Option<usize> {
        use rimstack_core::mods::list::Row;
        let state = self.list.read(cx);
        let list = state.order.list(Side::Active);
        let Row::Mod { index } = *list.rows().get(self.problem_nav.row?)? else {
            return None;
        };
        let id = list.mods().get(index)?.installed?;
        if state.selected_id() != Some(id) {
            return None;
        }
        let marked: Vec<bool> = state
            .order
            .troubles()
            .iter()
            .map(|t| !t.is_empty())
            .collect();
        Some(rimstack_core::mods::checks::place_among(&marked, index).0).filter(|place| *place > 0)
    }

    pub(super) fn show_problem(&mut self, forward: bool, cx: &mut Context<Self>) {
        use rimstack_core::mods::list::Row;
        let state = self.list.read(cx);
        let list = state.order.list(Side::Active);
        let rows = list.rows();
        let troubles = state.order.troubles();
        let troubled = |index: usize| troubles.get(index).is_some_and(|t| !t.is_empty());
        let count = rows.len() as isize;
        if count == 0 {
            return;
        }
        let step = if forward { 1 } else { -1 };
        let from = match self.problem_nav.row {
            Some(row) => row as isize,
            None if forward => -1,
            None => 0,
        };
        let found = (1..=count)
            .map(|n| (from + step * n).rem_euclid(count) as usize)
            .find(|&row| matches!(rows[row], Row::Mod { index } if troubled(index)));
        self.problem_nav.row = found;
        let Some(row) = found else {
            return;
        };
        if let Some(id) = self.active.rows.get(row).map(|r| r.mod_id) {
            self.select(id, true, cx);
            self.list(Side::Active)
                .update(cx, |list, cx| list.contain(row, cx));
        }
    }

    /// No warning about the game version while the game stays on `version`; "" brings them back.
    pub(super) fn set_version_aside(&mut self, version: String, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        let before = app.settings_report().version_aside_for.unwrap_or_default();
        if let Err(problem) = app.keep_setting("versionAsideFor", &version) {
            notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
            return;
        }
        if !version.is_empty() {
            let me = cx.entity().downgrade();
            notices::offer(
                put(&say!(
                    "Game version warnings are ignored for RimWorld {}",
                    version
                )),
                put(&say!("Undo")),
                move |_, cx| {
                    let before = before.clone();
                    let _ = me.update(cx, |mods, cx| mods.set_version_aside(before, cx));
                },
                cx,
            );
        }
    }
}

pub fn acting(
    mods: gpui::WeakEntity<Mods>,
    said: &crate::workspace::load_order::ProblemSaid,
) -> crate::ui::problem::Acting {
    let said = said.clone();
    std::rc::Rc::new(move |act, _, cx| {
        let _ = mods.update(cx, |mods, cx| mods.act_on(act, &said, cx));
    })
}

/// As `acting`, from another section: asked of the Mods tab through the window's router.
pub fn acting_from_afar(
    said: &crate::workspace::load_order::ProblemSaid,
) -> crate::ui::problem::Acting {
    let said = Box::new(said.clone());
    std::rc::Rc::new(move |act, _, cx| {
        crate::ui::navigate::go(crate::ui::navigate::Go::Problem(act, said.clone()), cx)
    })
}

impl Mods {
    pub(crate) fn act_on(
        &mut self,
        act: crate::ui::problem::Act,
        said: &crate::workspace::load_order::ProblemSaid,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::problem::Act;
        let fix = &said.fix;
        match act {
            Act::Fix => {
                self.apply_fix(said.mod_id, fix.action, &fix.target, fix.page.is_some(), cx)
            }
            Act::Ignore => self.ignore_problem(said.package_id.clone(), said.ignore.clone(), cx),
            Act::Show => {
                crate::ui::navigate::go(
                    crate::ui::navigate::Go::Section(crate::ui::section::Section::Mods),
                    cx,
                );
                if let Some(id) = said.mod_id {
                    self.show_mod(id, cx)
                }
            }
        }
    }
}

impl Mods {
    fn ignore_problem(&mut self, package_id: String, key: String, cx: &mut Context<Self>) {
        self.set_ignored(&package_id, &key, true, cx);
        let me = cx.entity().downgrade();
        notices::offer(
            put(&say!("The problem is ignored")),
            put(&say!("Undo")),
            move |_, cx| {
                let _ = me.update(cx, |mods, cx| {
                    mods.set_ignored(&package_id, &key, false, cx)
                });
            },
            cx,
        );
    }

    fn set_ignored(&mut self, package_id: &str, key: &str, yes: bool, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        app.ignore_problem(package_id, key, yes);
        self.edit_list(cx, |list, _| list.order.recheck(&app));
        self.show(cx);
    }

    pub(super) fn fix_all(&mut self, cx: &mut Context<Self>) {
        if self.list.read(cx).previewing() {
            return;
        }
        let app = super::super::app(cx);
        let needed = self.list.read(cx).order.needed();
        let ids = {
            let loaded = app.library();
            let Ok((library, _)) = &*loaded else {
                return;
            };
            let key_of = |id: ModId| library.get(id)?.key();
            self.list
                .read(cx)
                .order
                .list(Side::Inactive)
                .with_keys(&needed, &key_of)
        };
        self.edit_list(cx, |list, _| {
            list.order.take_all(&app, Side::Inactive, &ids, None)
        });
        self.changed(cx);
        self.done_said(say!("Activated the needed mods"), cx);
    }

    pub fn chosen_details(&self) -> Option<std::rc::Rc<crate::workspace::load_order::Details>> {
        self.details.clone()
    }

    pub fn conflicts(&self) -> usize {
        self.reads.conflicts
    }

    pub fn toggle_selected(&mut self, cx: &mut Context<Self>) {
        let state = self.list.read(cx);
        if let Some(id) = state.selected_id() {
            self.toggle_mod(Side::of(state.selected_active), id, cx);
        }
    }

    pub fn next_problem(&mut self, cx: &mut Context<Self>) {
        self.show_problem(true, cx);
    }

    pub fn open_problems(&mut self, cx: &mut Context<Self>) {
        self.problem_nav.open = true;
        self.sync_details(cx);
    }

    pub(super) fn close_problems(&mut self, cx: &mut Context<Self>) {
        self.problem_nav.open = false;
        self.sync_details(cx);
    }
}
