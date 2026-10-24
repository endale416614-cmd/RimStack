//! The window never hands back a list or a path; mods are named by id.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(QVariant, updates)]
        #[qproperty(bool, updates_busy, cxx_name = "updatesBusy")]
        #[qproperty(i32, behind_active, cxx_name = "behindActive")]
        /// `{ items, repositories, note, wanted }` or `{ problem }`.
        #[qproperty(QVariant, queue)]
        #[qproperty(bool, queue_busy, cxx_name = "queueBusy")]
        #[qproperty(QVariant, since)]
        /// `""`, `"active"` or `"inactive"`.
        #[qproperty(QString, since_only, cxx_name = "sinceOnly")]
        /// `[{ time, text }]`, by Workshop id.
        #[qproperty(QVariant, notes)]
        #[qproperty(bool, notes_busy, cxx_name = "notesBusy")]
        /// `{ done, asked, resume, refused }`.
        #[qproperty(QVariant, notes_state, cxx_name = "notesState")]
        #[qproperty(QVariant, replacements)]
        #[qproperty(bool, steamcmd)]
        #[qproperty(QString, task_note, cxx_name = "taskNote")]
        #[qproperty(bool, task_running, cxx_name = "taskRunning")]
        /// The checkout being brought up to its remote, by mod id; -1 for none.
        #[qproperty(i32, updating)]
        type DownloadsData = super::DownloadsDataRust;
    }

    impl cxx_qt::Threading for DownloadsData {}

    extern "RustQt" {
        #[qsignal]
        fn ended(self: Pin<&mut DownloadsData>, note: QString, failed: bool);

        #[qinvokable]
        #[cxx_name = "checkUpdates"]
        fn check_updates(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        #[cxx_name = "lookUp"]
        fn look_up(self: Pin<&mut DownloadsData>, pasted: &QString);

        /// Steam must be running.
        #[qinvokable]
        #[cxx_name = "updateInSteam"]
        fn update_in_steam(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        #[cxx_name = "subscribeWanted"]
        fn subscribe_wanted(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        #[cxx_name = "downloadWanted"]
        fn download_wanted(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        fn unsubscribe(self: Pin<&mut DownloadsData>, mod_id: i32);
        /// One request to Steam for all of them; mods not from the Workshop are left out.
        #[qinvokable]
        #[cxx_name = "unsubscribeAll"]
        fn unsubscribe_all(self: Pin<&mut DownloadsData>, mod_ids: &QVariant);
        /// Steam must be running.
        #[qinvokable]
        fn subscribe(self: Pin<&mut DownloadsData>, workshop_id: &QString);
        #[qinvokable]
        #[cxx_name = "updateFromGit"]
        fn update_from_git(self: Pin<&mut DownloadsData>, mod_id: i32);
        #[qinvokable]
        #[cxx_name = "cloneRepository"]
        fn clone_repository(self: Pin<&mut DownloadsData>, at: i32);
        #[qinvokable]
        #[cxx_name = "installSteamcmd"]
        fn install_steamcmd(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        fn redownload(self: Pin<&mut DownloadsData>, mod_id: i32);

        #[qinvokable]
        #[cxx_name = "fetchSinceNotes"]
        fn fetch_since_notes(self: Pin<&mut DownloadsData>);
        #[qinvokable]
        #[cxx_name = "fetchNotes"]
        fn fetch_notes(self: Pin<&mut DownloadsData>, mod_id: i32);
        #[qinvokable]
        #[cxx_name = "commitsOf"]
        fn commits_of(self: &DownloadsData, mod_id: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "ignoreReplacement"]
        fn ignore_replacement(self: &DownloadsData, mod_id: i32);
        #[qinvokable]
        #[cxx_name = "suggestReplacementsAgain"]
        fn suggest_replacements_again(self: &DownloadsData);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_downloads_data() -> UniquePtr<DownloadsData>;
    }
}

use crate::App;
use crate::changes::Changes;
use crate::qml::values::to_qml;
use crate::reports::Answer;
use crate::workspace::changed::{ChangeNotes, Only, SinceLaunch, since_shown};
use crate::workspace::downloads::{Queue, Updates, updates_shown};
use crate::workspace::fetch::Done;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use rimstack_core::base::error::Error;
use rimstack_core::mods::library::ModId;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

mod notes;
mod tasks;

pub struct DownloadsDataRust {
    updates: QVariant,
    updates_busy: bool,
    behind_active: i32,
    queue: QVariant,
    queue_busy: bool,
    since: QVariant,
    since_only: QString,
    notes: QVariant,
    notes_busy: bool,
    notes_state: QVariant,
    replacements: QVariant,
    steamcmd: bool,
    task_note: QString,
    task_running: bool,
    updating: i32,
    app: crate::hub::Given,
    demo: bool,
    checked: Updates,
    looked_up: Result<Queue, Error>,
    changed: SinceLaunch,
    fetched: ChangeNotes,
    asked_of_steam: HashSet<String>,
    waiting: VecDeque<(String, Work)>,
    since_reading: crate::jobs::Lane,
}

type Work = Box<dyn FnOnce() -> Done + Send>;

impl Default for DownloadsDataRust {
    fn default() -> Self {
        Self {
            updates: QVariant::default(),
            updates_busy: false,
            behind_active: 0,
            queue: QVariant::default(),
            queue_busy: false,
            since: QVariant::default(),
            since_only: QString::default(),
            notes: QVariant::default(),
            notes_busy: false,
            notes_state: QVariant::default(),
            replacements: QVariant::default(),
            steamcmd: false,
            task_note: QString::default(),
            task_running: false,
            updating: -1,
            app: crate::hub::Given::default(),
            demo: false,
            checked: Updates::default(),
            looked_up: Ok(Queue::default()),
            changed: SinceLaunch::default(),
            fetched: ChangeNotes::new(),
            asked_of_steam: HashSet::new(),
            waiting: VecDeque::new(),
            since_reading: crate::jobs::Lane::default(),
        }
    }
}

impl qobject::DownloadsData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>, demo: bool) {
        let me = self.as_mut().rust_mut().get_mut();
        me.app = crate::hub::Given::of(app.clone());
        me.demo = demo;
        if demo {
            return;
        }
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        self.as_mut()
            .on_since_only_changed(|me| me.show_since())
            .release();
        self.heard(Changes::LIBRARY | Changes::SETTINGS);
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        let app = self.app();
        if changes.contains(Changes::LIBRARY | Changes::SETTINGS | Changes::COMMUNITY) {
            self.as_mut().set_replacements(to_qml(&app.replaceable()));
            self.as_mut().set_steamcmd(app.steamcmd().is_some());
        }
        if changes.contains(Changes::LIBRARY | Changes::UPDATES) {
            self.as_mut().restore_updates();
        }
        if changes.contains(Changes::LIBRARY | Changes::UPDATES | Changes::LAUNCHES | Changes::LOGS)
        {
            self.read_since();
        }
    }

    fn restore_updates(self: Pin<&mut Self>) {
        let app = self.app();
        crate::jobs::run(
            "updates restore",
            self.qt_thread(),
            move || app.kept_updates(),
            |mut me, kept| {
                let Some(mut kept) = kept.filter(|_| !me.updates_busy) else {
                    return;
                };
                if kept.checked == me.checked.checked {
                    kept.problem = me.checked.problem.clone();
                }
                me.as_mut().show_updates(kept);
            },
        );
    }

    fn show_updates(mut self: Pin<&mut Self>, updates: Updates) {
        let me = self.as_mut().rust_mut().get_mut();
        let behind: HashSet<&String> = updates.items.iter().map(|row| &row.id).collect();
        me.asked_of_steam.retain(|id| behind.contains(id));
        me.checked = updates;
        let shown = updates_shown(&self.checked, &self.asked_of_steam);
        let (report, active) = (to_qml(&shown), shown.active.len());
        self.as_mut().set_updates(report);
        self.set_behind_active(i32::try_from(active).unwrap_or(i32::MAX));
    }

    fn check_updates(mut self: Pin<&mut Self>) {
        if self.updates_busy || self.demo {
            return;
        }
        self.as_mut().set_updates_busy(true);
        let app = self.app();
        crate::jobs::run(
            "updates check",
            self.qt_thread(),
            move || {
                let checked = app.check_updates();
                app.publish(Changes::UPDATES);
                checked
            },
            |mut me, checked| {
                me.as_mut().set_updates_busy(false);
                match checked {
                    Ok(updates) => me.show_updates(updates),
                    Err(problem) => me.show_updates(Updates {
                        problem: Some(problem.sentence().clone()),
                        ..Updates::default()
                    }),
                }
            },
        );
    }

    fn look_up(mut self: Pin<&mut Self>, pasted: &QString) {
        if self.queue_busy || self.demo {
            return;
        }
        self.as_mut().set_queue_busy(true);
        let (app, pasted) = (self.app(), pasted.to_string());
        crate::jobs::run(
            "look up",
            self.qt_thread(),
            move || app.look_up(&pasted),
            |mut me, looked_up| {
                let answer = match &looked_up {
                    Ok(queue) => to_qml(queue),
                    Err(problem) => to_qml(&Answer::<()>::from(Err(problem.clone()))),
                };
                me.as_mut().rust_mut().get_mut().looked_up = looked_up;
                me.as_mut().set_queue(answer);
                me.set_queue_busy(false);
            },
        );
    }

    fn read_since(self: Pin<&mut Self>) {
        let app = self.app();
        self.since_reading.run(
            "changed since launch",
            self.qt_thread(),
            move |_| app.since_launch(),
            |mut me, changed| {
                me.as_mut().rust_mut().get_mut().changed = changed;
                me.show_since();
            },
        );
    }

    fn show_since(mut self: Pin<&mut Self>) {
        let only = Only::named(&self.since_only.to_string());
        let shown = to_qml(&since_shown(&self.changed, only, &self.fetched));
        self.as_mut().set_since(shown);
    }

    fn ignore_replacement(&self, mod_id: i32) {
        if let Some(id) = ModId::from_qml(mod_id)
            && let Err(problem) = self.app().ignore_replacement(Some(id))
        {
            rimstack_core::problem!("{problem}");
        }
    }

    fn suggest_replacements_again(&self) {
        if let Err(problem) = self.app().ignore_replacement(None) {
            rimstack_core::problem!("{problem}");
        }
    }
}
