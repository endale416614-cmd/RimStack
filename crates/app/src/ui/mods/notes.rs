use super::Mods;
use crate::ui::menu::{self, Entry};
use crate::words::put;
use crate::workspace::load_order::{LoadOrder, Side};
use gpui::{Context, Pixels, Point, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::say;
use std::time::Duration;

const PAUSE: Duration = Duration::from_millis(600);
const SHOWN: Duration = Duration::from_millis(1800);

/// The other mod of a rule of your own being chosen: the chosen one loads after it, or before.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Picking {
    After,
    Before,
}

#[derive(Default)]
pub struct Drafts {
    pause: crate::jobs::Lane,
    fade: crate::jobs::Lane,
    note: Option<(ModId, String, String)>,
    group: Option<(ModId, String)>,
}

impl Mods {
    pub(super) fn start_picking(&mut self, picking: Option<Picking>, cx: &mut Context<Self>) {
        self.chosen_reads.picking = picking;
        self.sync_lists(cx);
        self.sync_details(cx);
        cx.notify();
    }

    pub(super) fn pick(&mut self, other: ModId, cx: &mut Context<Self>) {
        let Some(chosen) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        match self.chosen_reads.picking.take() {
            Some(Picking::After) => app.change_rule(other, chosen, true),
            Some(Picking::Before) => app.change_rule(chosen, other, true),
            None => return,
        }
        self.show(cx);
    }

    pub(super) fn remove_rule(&mut self, first: ModId, then: ModId, cx: &mut Context<Self>) {
        super::super::app(cx).change_rule(first, then, false);
        self.show(cx);
    }

    pub(super) fn win_over(&mut self, other: ModId, cx: &mut Context<Self>) {
        let Some(id) = self.list.read(cx).selected_id() else {
            return;
        };
        let app = super::super::app(cx);
        app.change_rule(other, id, true);
        self.edit_list(cx, |list, _| {
            if let Some((from, to)) = list.order.list(Side::Active).below(id, other) {
                list.order.move_mod(&app, Side::Active, from, to);
            }
        });
        self.changed(cx);
    }

    pub(super) fn compare_with(&mut self, other: ModId, cx: &mut Context<Self>) {
        let list = self.list.read(cx);
        let Some(id) = list.selected_id() else {
            return;
        };
        if let Some((earlier, later)) = list.order.list(Side::Active).in_load_order(id, other) {
            self.open_compare(earlier, later, cx);
        }
    }

    pub(super) fn warn_again(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        app.warn_again(&[id]);
        self.edit_list(cx, |list, _| list.order.recheck(&app));
        self.show(cx);
    }

    pub(super) fn draft_note(&mut self, tags: String, note: String, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_id(cx) {
            self.chosen_reads.drafts.note = Some((id, tags, note));
            self.wait_for_drafts(cx);
        }
    }

    pub(super) fn draft_group(&mut self, name: String, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_id(cx) {
            self.chosen_reads.drafts.group = Some((id, name));
            self.wait_for_drafts(cx);
        }
    }

    fn wait_for_drafts(&mut self, cx: &mut Context<Self>) {
        self.chosen_reads
            .drafts
            .pause
            .after(PAUSE, cx, |mods, cx| mods.keep_drafts(cx));
    }

    pub(super) fn keep_drafts(&mut self, cx: &mut Context<Self>) {
        self.chosen_reads.drafts.pause.cancel();
        let (note, group) = (
            self.chosen_reads.drafts.note.take(),
            self.chosen_reads.drafts.group.take(),
        );
        let app = super::super::app(cx);
        let mut changed = false;
        if let Some((id, tags, note)) = note {
            changed |= app.set_note(id, &tags, &note);
        }
        if let Some((id, name)) = group {
            changed |= app.set_group(&[id], &name);
        }
        if !changed {
            return;
        }
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.chosen_reads.notes_saved = true;
        self.show(cx);
        self.chosen_reads.drafts.fade.after(SHOWN, cx, |mods, cx| {
            mods.chosen_reads.notes_saved = false;
            mods.sync_details(cx);
        });
    }

    pub(super) fn category_choices(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let chosen = self
            .details
            .as_ref()
            .map(|d| d.category_chosen.clone())
            .unwrap_or_default();
        let labels = self
            .list
            .read(cx)
            .overview
            .as_ref()
            .map(|o| o.categories.clone())
            .unwrap_or_default();
        let me = cx.entity().downgrade();
        let file = |label: &'static str| {
            let me = me.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = me.update(cx, |mods, cx| mods.file_under(id, label, cx));
            }
        };
        let mut entries: Vec<Entry> = vec![
            Entry::item(put(&say!("Automatic")), file(""))
                .icon(if chosen.is_empty() { "check" } else { "" })
                .into(),
            Entry::Line,
        ];
        for label in labels {
            let item = Entry::item(put(&Sentence::said(label, Vec::new())), file(label));
            entries.push(
                if chosen == label {
                    item.icon("check")
                } else {
                    item
                }
                .into(),
            );
        }
        menu::open_at(at, entries, window, cx);
    }

    fn file_under(&mut self, id: ModId, label: &str, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        app.set_category(id, label);
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.show(cx);
    }

    pub(super) fn read_chosen(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.details.as_ref().and(self.selected_id(cx)) else {
            self.chosen_reads.read.cancel();
            self.chosen_reads.files = None;
            return;
        };
        self.read_commits(id, cx);
        if self
            .chosen_reads
            .files
            .as_ref()
            .is_some_and(|(of, _)| *of == id)
        {
            return;
        }
        let app = super::super::app(cx);
        self.chosen_reads.read.run(
            "files of a mod",
            cx,
            move |_| {
                let textures = LoadOrder::textures(&app, id);
                let files = LoadOrder::files(&app, id);
                let hidden = app.hidden_in(id);
                (textures, files, hidden)
            },
            move |mods, read, cx| {
                mods.chosen_reads.files = Some((id, std::rc::Rc::new(read)));
                mods.sync_details(cx);
            },
        );
    }

    fn read_commits(&mut self, id: ModId, cx: &mut Context<Self>) {
        let git = self.details.as_ref().is_some_and(|d| !d.workshop && d.git);
        if !git
            || self
                .chosen_reads
                .commits
                .as_ref()
                .is_some_and(|(of, _)| *of == id)
        {
            return;
        }
        let app = super::super::app(cx);
        self.chosen_reads.commits_read.run(
            "commits of a mod",
            cx,
            move |_| (app.commits_of(id), app.repository_of(id)),
            move |mods, (commits, page), cx| {
                mods.chosen_reads.commits = Some((id, std::rc::Rc::new(commits)));
                mods.chosen_reads.repository = Some((id, page));
                mods.describe(cx);
            },
        );
    }

    pub(super) fn unhide_chosen(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        self.status.unhide_problem = crate::words::why_not(&app.unhide_mod(id));
        self.chosen_reads.files = None;
        self.read_chosen(cx);
        self.sync_details(cx);
    }
}
