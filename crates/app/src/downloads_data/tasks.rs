//! One at a time, in the order asked: two would write the same SteamCMD folder.

use super::{Work, qobject};
use crate::changes::Changes;
use crate::workspace::fetch::{Done, SteamAsk};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::say;

impl qobject::DownloadsData {
    fn task(
        mut self: Pin<&mut Self>,
        doing: Sentence,
        work: impl FnOnce() -> Done + Send + 'static,
    ) {
        if self.demo {
            return;
        }
        let doing = crate::words::put(&doing);
        let work: Work = Box::new(work);
        match self.task_running {
            true => self
                .as_mut()
                .rust_mut()
                .get_mut()
                .waiting
                .push_back((doing, work)),
            false => self.start(&doing, work),
        }
    }

    fn start(mut self: Pin<&mut Self>, doing: &str, work: Work) {
        self.as_mut().set_task_running(true);
        self.as_mut().set_task_note(QString::from(doing));
        crate::jobs::run("download task", self.qt_thread(), work, |mut me, done| {
            me.as_mut().finish(done);
            if let Some((doing, work)) = me.as_mut().rust_mut().get_mut().waiting.pop_front() {
                me.start(&doing, work);
            }
        });
    }

    fn finish(mut self: Pin<&mut Self>, done: Done) {
        let app = self.app();
        if done.wrote {
            app.publish(Changes::MOD_FILES);
        }
        let failed = done.note.is_err();
        let said = match done.note {
            Ok(did) => {
                let said = crate::words::put(&did);
                app.note(did, String::new());
                said
            }
            Err(problem) => crate::words::put(problem.sentence()),
        };
        self.as_mut().set_steamcmd(app.steamcmd().is_some());
        self.as_mut().set_updating(-1);
        self.as_mut().set_task_running(false);
        self.as_mut().set_task_note(QString::from(&said));
        self.ended(QString::from(&said), failed);
    }

    fn ask_steam(self: Pin<&mut Self>, what: SteamAsk, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        let app = self.app();
        self.task(say!("Asking Steam…"), move || app.ask_steam(what, &ids));
    }

    pub(super) fn update_in_steam(mut self: Pin<&mut Self>) {
        let ids: Vec<String> = self
            .checked
            .items
            .iter()
            .map(|row| row.id.clone())
            .collect();
        self.as_mut()
            .rust_mut()
            .get_mut()
            .asked_of_steam
            .extend(ids.iter().cloned());
        let checked = std::mem::take(&mut self.as_mut().rust_mut().get_mut().checked);
        self.as_mut().show_updates(checked);
        self.ask_steam(SteamAsk::Download, ids);
    }

    pub(super) fn subscribe_wanted(self: Pin<&mut Self>) {
        let wanted = self
            .looked_up
            .as_ref()
            .map_or(Vec::new(), |q| q.wanted.clone());
        self.ask_steam(SteamAsk::Subscribe, wanted);
    }

    pub(super) fn subscribe(self: Pin<&mut Self>, workshop_id: &QString) {
        self.ask_steam(SteamAsk::Subscribe, vec![workshop_id.to_string()]);
    }

    pub(super) fn unsubscribe(self: Pin<&mut Self>, mod_id: i32) {
        let workshop = ModId::from_qml(mod_id).and_then(|id| self.app().workshop_id(id));
        self.ask_steam(SteamAsk::Unsubscribe, workshop.into_iter().collect());
    }

    pub(super) fn unsubscribe_all(self: Pin<&mut Self>, mod_ids: &QVariant) {
        let app = self.app();
        let workshop = crate::qml::values::mod_ids(mod_ids)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| app.workshop_id(id))
            .collect();
        self.ask_steam(SteamAsk::Unsubscribe, workshop);
    }

    pub(super) fn download_wanted(self: Pin<&mut Self>) {
        let wanted = self
            .looked_up
            .as_ref()
            .map_or(Vec::new(), |q| q.wanted.clone());
        if wanted.is_empty() {
            return;
        }
        let app = self.app();
        let doing = say!("Downloading {} with SteamCMD…", wanted.len());
        self.task(doing, move || app.download_with_steamcmd(&wanted));
    }

    pub(super) fn update_from_git(mut self: Pin<&mut Self>, mod_id: i32) {
        let Some(id) = ModId::from_qml(mod_id) else {
            return;
        };
        self.as_mut().set_updating(mod_id);
        let app = self.app();
        self.as_mut().task(say!("Updating from Git…"), move || {
            app.update_checkout(id)
        });
    }

    pub(super) fn clone_repository(self: Pin<&mut Self>, at: i32) {
        let link = usize::try_from(at).ok().and_then(|at| {
            let queue = self.looked_up.as_ref().ok()?;
            Some(queue.repositories.get(at)?.link.clone())
        });
        let Some(link) = link else {
            return;
        };
        let app = self.app();
        self.task(say!("Cloning {}…", link), move || {
            app.clone_repository(&link)
        });
    }

    pub(super) fn install_steamcmd(self: Pin<&mut Self>) {
        let app = self.app();
        self.task(say!("Installing SteamCMD…"), move || {
            app.install_steamcmd()
        });
    }

    pub(super) fn redownload(self: Pin<&mut Self>, mod_id: i32) {
        let Some(id) = ModId::from_qml(mod_id) else {
            return;
        };
        let app = self.app();
        self.task(say!("Downloading again with SteamCMD…"), move || {
            app.redownload(id)
        });
    }
}
