use super::Mods;
use crate::workspace::load_order::Side;
use gpui::Context;
use rimstack_core::mods::library::ModId;

impl Mods {
    pub fn show_mod(&mut self, mod_id: ModId, cx: &mut Context<Self>) {
        for side in [Side::Active, Side::Inactive] {
            let at = self
                .shown(side)
                .rows
                .iter()
                .position(|row| row.mod_id == Some(mod_id) && !row.is_header);
            if let Some(at) = at {
                if side == Side::Inactive && self.view.inactive_hidden {
                    self.set_hidden(false, cx);
                }
                self.choose(side, mod_id, cx);
                self.list(side).update(cx, |list, cx| list.reveal(at, cx));
                return;
            }
        }
    }

    pub fn start_bench(&mut self, cx: &mut Context<Self>) {
        self.active_list.update(cx, |list, cx| list.start_bench(cx));
    }

    pub fn trust(
        &mut self,
        ids: Vec<rimstack_core::mods::library::ModId>,
        yes: bool,
        cx: &mut Context<Self>,
    ) {
        let app = crate::ui::app(cx);
        app.set_version_ok(&ids, yes);
        self.edit_list(cx, |list, _| list.order.recheck(&app));
        self.show(cx);
    }

    /// The load order becomes `ids`, unsaved; the history keeps the list it was.
    pub fn use_list(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        let before = app.snapshot("@list", app.session().current());
        let count = ids.len();
        self.edit_list(cx, |list, _| list.order.use_list(&app, ids));
        self.changed(cx);
        if !self.demo {
            use rimstack_core::say_n;
            let taken = say_n!(
                count,
                "Another list taken: {} mod",
                "Another list taken: {} mods"
            );
            app.note_change(taken, before.as_deref());
            let said = say_n!(count, "Using a list of {} mod", "Using a list of {} mods");
            self.done_said(said, cx);
        }
    }

    pub fn set_hunting(&mut self, hunting: bool, cx: &mut Context<Self>) {
        self.hunting = hunting;
        self.sync_details(cx);
    }

    /// The load order becomes `ids`, unsaved, nothing kept nor said: a round of the search.
    pub fn use_list_quietly(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.edit_list(cx, |list, _| list.order.use_list(&app, ids));
        self.changed(cx);
    }

    pub fn inactive_hidden(&self) -> bool {
        self.view.inactive_hidden
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        let hidden = !self.view.inactive_hidden;
        self.set_hidden(hidden, cx);
    }

    pub fn inactive_by_category(&self) -> bool {
        self.view.inactive_order.is_empty()
    }

    pub fn toggle_inactive_order(&mut self, cx: &mut Context<Self>) {
        let then = if self.inactive_by_category() {
            "name"
        } else {
            ""
        };
        self.order_inactive(then, cx);
    }

    /// `act` after the next draw, and after each one until it answers that it was done.
    pub fn when_drawn(&mut self, act: super::AfterDrawn, cx: &mut Context<Self>) {
        self.when_drawn.push(act);
        cx.notify();
    }

    pub(super) fn run_when_drawn(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.when_drawn.is_empty() {
            return;
        }
        let waiting = std::mem::take(&mut self.when_drawn);
        cx.defer_in(window, move |mods, window, cx| {
            for mut act in waiting {
                if !act(mods, window, cx) {
                    mods.when_drawn.push(act);
                }
            }
            if !mods.when_drawn.is_empty() {
                cx.notify();
            }
        });
    }
}
