//! The load order being edited and the session around it, in a model of their own: what the bars,
//! Downloads and the game watcher follow, so that laying out the Mods tab never touches them.

use crate::workspace::load_order::{LoadOrder, Overview, ProblemSaid, Side};
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};
use std::rc::Rc;

use super::Standing;
use crate::words::put;

pub struct ListState {
    pub(super) order: LoadOrder,
    /// `RowView::mod_id`s, the first chosen first.
    pub(super) selection: Vec<ModId>,
    pub(super) selected_active: bool,
    pub(super) standing: Standing,
    pub(super) overview: Option<Rc<Overview>>,
    pub(super) sort_pending: usize,
    pub(crate) rescanning: bool,
    pub(super) files_read: bool,
    pub(super) saving: bool,
    pub(super) demo: bool,
}

impl ListState {
    pub fn new(order: LoadOrder, demo: bool) -> Self {
        Self {
            order,
            selection: Vec::new(),
            selected_active: true,
            standing: Standing::default(),
            overview: None,
            sort_pending: 0,
            rescanning: false,
            files_read: false,
            saving: false,
            demo,
        }
    }

    pub fn session_standing(&self) -> &Standing {
        &self.standing
    }

    pub fn count(&self, side: Side) -> usize {
        self.order.list(side).counted()
    }

    pub fn matching(&self, side: Side) -> usize {
        self.order.list(side).matching()
    }

    pub fn selected_id(&self) -> Option<ModId> {
        self.selection.first().copied()
    }

    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    pub fn selected_on_active(&self) -> bool {
        self.selected_active
    }

    pub fn is_chosen(&self, side: Side, mod_id: Option<ModId>) -> bool {
        mod_id.is_some_and(|id| {
            self.selected_active == (side == Side::Active) && self.selection.contains(&id)
        })
    }

    pub fn previewing(&self) -> bool {
        self.order.previewing()
    }

    pub fn files_read(&self) -> bool {
        self.files_read
    }

    pub fn to_get(&self) -> Vec<ProblemSaid> {
        self.overview
            .as_ref()
            .map(|o| o.to_get.clone())
            .unwrap_or_default()
    }

    pub fn game_version(&self) -> String {
        self.overview
            .as_ref()
            .map(|o| o.game_version.clone())
            .unwrap_or_default()
    }

    pub fn overview(&self) -> Option<Rc<Overview>> {
        self.overview.clone()
    }

    pub fn active_installed(&self) -> Vec<ModId> {
        let mods = self.order.list(Side::Active).mods().iter();
        mods.filter_map(|m| m.installed).collect()
    }

    pub fn reading_files(&self) -> bool {
        !self.files_read && self.count(Side::Active) > 0 && !self.demo
    }

    pub fn read_as(&self) -> (usize, usize, String) {
        (
            self.count(Side::Active),
            self.count(Side::Inactive),
            put(&self.order.problem),
        )
    }

    pub fn problems(&self) -> (usize, usize) {
        match self.demo {
            true => (0, 0),
            false => self.order.errors_and_warnings(),
        }
    }

    /// What is under way on the load order: a change seen on disk waits for it.
    pub fn held(&self) -> bool {
        self.order.previewing() || self.saving || self.rescanning
    }

    pub fn in_the_way_said(&self, errors: bool, sort_pending: bool, dirty: bool) -> Vec<String> {
        let errors_count = self.order.errors_and_warnings().0;
        let mut said = Vec::new();
        if errors {
            said.push(put(&say_n!(
                errors_count,
                "{} mod has a problem that will cause errors in the game",
                "{} mods have problems that will cause errors in the game"
            )));
        }
        if sort_pending {
            said.push(put(&say_n!(
                self.sort_pending,
                "Sorting would move {} mod",
                "Sorting would move {} mods"
            )));
        }
        if dirty {
            said.push(put(&say!(
                "The list has unsaved changes; the game would start with the saved one"
            )));
        }
        said
    }

    pub fn standing(&self) -> (bool, bool, bool, Option<String>) {
        let unsaved = (self.standing.dirty && self.order.is_installed())
            .then(|| self.standing.unsaved.clone());
        (
            self.order.errors_and_warnings().0 > 0,
            self.sort_pending > 0,
            unsaved.is_some(),
            unsaved,
        )
    }

    pub fn work_said(&self) -> Vec<String> {
        let mut said = Vec::new();
        if self.rescanning && !self.demo {
            said.push(put(&say!("Reading the mod folders…")));
        }
        if self.reading_files() {
            said.push(put(&say!("Reading the active mods' files…")));
        }
        said
    }

    pub fn found(
        &self,
        text: &str,
        most: usize,
        cx: &gpui::App,
    ) -> Vec<crate::workspace::load_order::FoundMod> {
        self.order.found(&crate::ui::app(cx), text, most)
    }
}
