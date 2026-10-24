use super::{id, ids, qobject};
use crate::workspace::load_order::Side;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::{say, say_n};

impl qobject::ModsData {
    fn refuse_list(self: Pin<&mut Self>) {
        let said = crate::words::said(&say!("That is not a list of mods."));
        self.told(said, QString::from("err"));
    }

    fn done_said(mut self: Pin<&mut Self>, said: Sentence) {
        if self.records {
            self.as_mut().done(crate::words::said(&said));
        }
    }

    pub(super) fn order_mut(self: Pin<&mut Self>) -> &mut crate::workspace::load_order::LoadOrder {
        &mut self.rust_mut().get_mut().order
    }

    pub(super) fn move_mod(mut self: Pin<&mut Self>, active: bool, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .move_mod(&app, Side::of(active), from, to);
        self.changed();
    }

    pub(super) fn take(mut self: Pin<&mut Self>, from_active: bool, index: i32, to: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let from = Side::of(from_active);
        let name = self
            .order
            .list(from)
            .mods()
            .get(index)
            .map(|m| m.name.clone());
        let Some(name) = name else {
            return;
        };
        let app = self.app();
        let to = usize::try_from(to).ok();
        self.as_mut().order_mut().take(&app, from, index, to);
        self.as_mut().changed();
        let said = match from {
            Side::Inactive => say!("Activated {}", name),
            Side::Active => say!("Deactivated {}", name),
        };
        self.done_said(said);
    }

    pub(super) fn take_all(mut self: Pin<&mut Self>, from_active: bool, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        if ids.is_empty() {
            return;
        }
        let from = Side::of(from_active);
        let app = self.app();
        self.as_mut().order_mut().take_all(&app, from, &ids);
        self.as_mut().changed();
        let said = match from {
            Side::Inactive => say_n!(ids.len(), "Activated {} mod", "Activated {} mods"),
            Side::Active => say_n!(ids.len(), "Deactivated {} mod", "Deactivated {} mods"),
        };
        self.done_said(said);
    }

    pub(super) fn to_edge(mut self: Pin<&mut Self>, given: &QVariant, top: bool) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        self.as_mut().order_mut().to_edge(&app, &ids, top);
        self.as_mut().changed();
        self.done_said(say_n!(ids.len(), "Moved {} mod", "Moved {} mods"));
    }

    pub(super) fn toggle_category(mut self: Pin<&mut Self>, active: bool, category: i32) {
        let found = usize::try_from(category).ok();
        let Some(&category) = found.and_then(|at| rimstack_core::mods::list::Category::ALL.get(at))
        else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .toggle_category(&app, Side::of(active), category);
        self.show();
    }

    pub(super) fn toggle_sub(mut self: Pin<&mut Self>, active: bool, row: i32) {
        let Ok(row) = usize::try_from(row) else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .toggle_sub(&app, Side::of(active), row);
        self.show();
    }

    pub(super) fn remove_sub(mut self: Pin<&mut Self>, active: bool, row: i32) {
        let group = usize::try_from(row)
            .ok()
            .and_then(|row| self.order.group_on(Side::of(active), row));
        let Some((tag, ids)) = group else {
            return;
        };
        let app = self.app();
        app.remove_tag(&ids, &tag);
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn show_by(mut self: Pin<&mut Self>, column: &QString, descending: bool) {
        self.as_mut()
            .order_mut()
            .show_by(&column.to_string(), descending);
        self.show();
    }

    pub(super) fn undo(mut self: Pin<&mut Self>) {
        let app = self.app();
        if self.as_mut().order_mut().undo(&app) {
            self.changed();
        }
    }

    pub(super) fn redo(mut self: Pin<&mut Self>) {
        let app = self.app();
        if self.as_mut().order_mut().redo(&app) {
            self.changed();
        }
    }

    pub(super) fn discard_changes(mut self: Pin<&mut Self>) {
        let app = self.app();
        self.as_mut().order_mut().discard_changes(&app);
        self.as_mut().changed();
        self.done_said(say!("Back to the saved list"));
    }

    pub(super) fn use_list(mut self: Pin<&mut Self>, given: &QVariant, quiet: bool) {
        let Some(ids) = crate::qml::values::from_qml::<Vec<String>>(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        let keep = self.records && !quiet;
        let before = keep
            .then(|| app.snapshot("@list", app.session().current()))
            .flatten();
        let count = ids.len();
        self.as_mut().order_mut().use_list(&app, ids);
        self.as_mut().changed();
        if keep {
            let taken = say_n!(
                count,
                "Another list taken: {} mod",
                "Another list taken: {} mods"
            );
            app.note_change(taken, before.as_deref());
            let said = say_n!(count, "Using a list of {} mod", "Using a list of {} mods");
            self.done_said(said);
        }
    }

    pub(super) fn keep_file(mut self: Pin<&mut Self>, take: bool) {
        let app = self.app();
        self.as_mut().order_mut().keep_file(&app, take);
        self.as_mut().set_save_problem(QString::default());
        self.as_mut().set_file_changed(false);
        self.as_mut().changed();
    }

    pub(super) fn save(mut self: Pin<&mut Self>) -> bool {
        if self.saving {
            return false;
        }
        if self.order.previewing() {
            self.as_mut().end_sort(true);
        }
        let app = self.app();
        let saving = match self.as_mut().order_mut().to_save(&app) {
            Ok(saving) => saving,
            Err(problem) => {
                self.as_mut()
                    .set_save_problem(crate::words::said_of(&problem));
                self.as_mut().saved(false);
                return false;
            }
        };
        let records = self.records;
        let changes = app.session().changes().said();
        let before = app.session().saved().to_vec();
        self.as_mut().set_saving(true);
        let worker = app.clone();
        crate::jobs::run(
            "save",
            self.qt_thread(),
            move || {
                let snapshot = records
                    .then(|| worker.snapshot("@saving", &before))
                    .flatten();
                let written = worker.write_list(&saving.file, &saving.written, &saving.expected);
                if written.is_ok() && records {
                    let said = say!("Saved the load order: {}", changes.inside());
                    worker.note_change(said, snapshot.as_deref());
                }
                (saving.list, written)
            },
            move |mut me, (list, written)| {
                if written.is_ok() {
                    me.as_mut().order_mut().saved(&app, list);
                }
                me.as_mut()
                    .set_save_problem(crate::words::why_not(&written));
                let outside = written.as_ref().err() == Some(&modsconfig::changed_outside());
                me.as_mut().set_file_changed(outside);
                me.as_mut().set_saving(false);
                me.as_mut().changed();
                me.saved(written.is_ok());
            },
        );
        true
    }

    pub(super) fn preview_sort(mut self: Pin<&mut Self>) {
        if !self.files_read {
            return;
        }
        let app = self.app();
        self.as_mut().order_mut().preview_sort(&app);
        self.show();
    }

    pub(super) fn end_sort(mut self: Pin<&mut Self>, keep: bool) {
        let app = self.app();
        if !keep {
            self.as_mut().order_mut().drop_sort(&app);
            return self.show();
        }
        if !self.order.previewing() {
            return;
        }
        let moves = self.order.sort_move_count();
        let before = self
            .records
            .then(|| app.snapshot("@sorting", app.session().current()))
            .flatten();
        self.as_mut().order_mut().keep_sort(&app);
        self.as_mut().changed();
        if self.records {
            let sorted = say_n!(moves, "Sorted: {} mod moved", "Sorted: {} mods moved");
            app.note_change(sorted.clone(), before.as_deref());
            self.done_said(sorted);
        }
    }

    pub(super) fn count_sort_moves(self: Pin<&mut Self>) {
        const QUIET: std::time::Duration = std::time::Duration::from_millis(400);
        let count = self.order.sort_count().filter(|_| self.files_read);
        let Some(count) = count else {
            self.counting.cancel();
            self.set_sort_pending(0);
            return;
        };
        let app = self.app();
        self.counting.run(
            "sort count",
            self.qt_thread(),
            move |turn| {
                std::thread::sleep(QUIET);
                turn.current().then(|| count.moves(&app))
            },
            |me, moves| {
                if let Some(moves) = moves {
                    me.set_sort_pending(i32::try_from(moves).unwrap_or(i32::MAX));
                }
            },
        );
    }

    pub(super) fn rescan(mut self: Pin<&mut Self>) {
        if self.rescanning || self.demo {
            return;
        }
        self.as_mut().set_rescanning(true);
        let app = self.app();
        crate::jobs::run("rescan", self.qt_thread(), move || app.rescan(), |_, ()| {});
    }

    pub(super) fn trust(mut self: Pin<&mut Self>, given: &QVariant, yes: bool) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.set_version_ok(&ids, yes);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn ignore_problem(
        mut self: Pin<&mut Self>,
        package_id: &QString,
        key: &QString,
        yes: bool,
    ) {
        let app = self.app();
        app.ignore_problem(&package_id.to_string(), &key.to_string(), yes);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn warn_again(mut self: Pin<&mut Self>, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.warn_again(&ids);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn keep_note(mut self: Pin<&mut Self>, mod_id: i32, tags: &QString, note: &QString) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        app.set_note(id, &tags.to_string(), &note.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn add_tag(mut self: Pin<&mut Self>, given: &QVariant, tag: &QString) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let (app, tag) = (self.app(), tag.to_string());
        app.add_tag(&ids, &tag);
        self.as_mut().order_mut().sync(&app);
        self.as_mut().show();
        let said = say_n!(ids.len(), "Tagged {} mod: {}", "Tagged {} mods: {}", tag);
        self.told(crate::words::said(&said), QString::from("ok"));
    }

    pub(super) fn group(mut self: Pin<&mut Self>, given: &QVariant, name: &QString) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.set_group(&ids, &name.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn file_under(mut self: Pin<&mut Self>, mod_id: i32, label: &QString) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        app.set_category(id, &label.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn add_rule(self: Pin<&mut Self>, first: i32, then: i32) {
        self.change_rule(first, then, true);
    }

    pub(super) fn remove_rule(self: Pin<&mut Self>, first: i32, then: i32) {
        self.change_rule(first, then, false);
    }

    fn change_rule(self: Pin<&mut Self>, first: i32, then: i32, add: bool) {
        if let (Some(first), Some(then)) = (id(first), id(then)) {
            self.app().change_rule(first, then, add);
            self.show();
        }
    }

    pub(super) fn win_over(mut self: Pin<&mut Self>, mod_id: i32, other: i32) {
        let (Some(id), Some(other)) = (id(mod_id), id(other)) else {
            return;
        };
        let app = self.app();
        app.change_rule(other, id, true);
        if let Some((from, to)) = self.order.list(Side::Active).below(id, other) {
            self.as_mut()
                .order_mut()
                .move_mod(&app, Side::Active, from, to);
        }
        self.changed();
    }

    pub(super) fn fix(
        mut self: Pin<&mut Self>,
        mod_id: i32,
        action: &QString,
        target: &QString,
    ) -> bool {
        if self.order.previewing() {
            return true;
        }
        let target = target.to_string();
        match action.to_string().as_str() {
            "activate" => {
                let found = self.package_mod(false, &QString::from(&target));
                if let Some(at) =
                    id(found).and_then(|i| self.order.list(Side::Inactive).position_of(i))
                {
                    self.take(false, at as i32, -1);
                }
                true
            }
            "deactivate" => {
                let at = target.parse::<i32>().unwrap_or(-1);
                self.take(true, at, -1);
                true
            }
            "trust" => {
                self.as_mut()
                    .trust(&crate::qml::values::to_qml(&[mod_id]), true);
                true
            }
            _ => false,
        }
    }

    pub(super) fn fix_all(mut self: Pin<&mut Self>) {
        if self.order.previewing() {
            return;
        }
        let needed = self.order.needed();
        let ids = self.needed_ids(&needed);
        let app = self.app();
        self.as_mut()
            .order_mut()
            .take_all(&app, Side::Inactive, &ids);
        self.as_mut().changed();
        self.done_said(say!("Activated the needed mods"));
    }

    pub(super) fn activate_fresh(mut self: Pin<&mut Self>) {
        let app = self.app();
        let fresh: Vec<ModId> = self.order.fresh(&app, Side::Inactive);
        if fresh.is_empty() || self.order.previewing() {
            return;
        }
        self.as_mut()
            .order_mut()
            .take_all(&app, Side::Inactive, &fresh);
        self.as_mut().changed();
        self.done_said(say_n!(
            fresh.len(),
            "Activated {} new mod",
            "Activated {} new mods"
        ));
    }

    pub(super) fn clear_active(mut self: Pin<&mut Self>) {
        let app = self.app();
        let ids = self.order.unofficial(&app);
        if ids.is_empty() || self.order.previewing() {
            return;
        }
        self.as_mut().order_mut().take_all(&app, Side::Active, &ids);
        self.as_mut().changed();
        self.done_said(say_n!(
            ids.len(),
            "Deactivated {} mod",
            "Deactivated {} mods"
        ));
    }

    pub(super) fn bin(mut self: Pin<&mut Self>, mod_id: i32) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let binned = self.app().bin_mod(id);
        self.as_mut()
            .set_upkeep_problem(crate::words::why_not(&binned));
        if binned.is_ok() {
            self.rescan();
        }
    }

    pub(super) fn bin_all(mut self: Pin<&mut Self>, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        let (binned, refused): (Vec<_>, Vec<_>) = ids
            .into_iter()
            .map(|id| app.bin_mod(id))
            .partition(Result::is_ok);
        let first = refused.into_iter().next().unwrap_or(Ok(()));
        self.as_mut()
            .set_upkeep_problem(crate::words::why_not(&first));
        if !binned.is_empty() {
            self.as_mut().rescan();
            self.done_said(say_n!(
                binned.len(),
                "Moved {} mod to the bin",
                "Moved {} mods to the bin"
            ));
        }
    }

    pub(super) fn copy_to_local(mut self: Pin<&mut Self>, mod_id: i32) {
        let Some(id) = id(mod_id) else {
            return;
        };
        self.as_mut().set_upkeep_problem(QString::default());
        let app = self.app();
        crate::jobs::run(
            "copy to the game folder",
            self.qt_thread(),
            move || app.copy_to_local(id).err(),
            |mut me, problem| match problem {
                Some(problem) => me.set_upkeep_problem(crate::words::said_of(&problem)),
                None => me.as_mut().rescan(),
            },
        );
    }
}
