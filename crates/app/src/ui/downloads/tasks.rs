//! What Downloads asks of Steam, SteamCMD and Git: one at a time, in the order asked, since two
//! would write the same SteamCMD folder. And the change notes, read from the Workshop.

use super::{Downloads, Waiting};
use crate::ui::prompt::{self, Answer, Line, Question};
use crate::words::put;
use crate::workspace::changed::{MOST_ASKED, MOST_NOTES, NotesState, notes_asked};
use crate::workspace::fetch::{Done, SteamAsk};
use crate::workspace::now_secs;
use gpui::{Context, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::sources::changes::{self, Fetching};
use rimstack_core::{say, say_n};

impl Downloads {
    pub(super) fn task(
        &mut self,
        doing: Sentence,
        updating: Option<ModId>,
        work: impl FnOnce() -> Done + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            return;
        }
        let waits = Waiting {
            doing: put(&doing),
            updating,
            work: Box::new(work),
        };
        match self.tasks.running {
            true => self.tasks.waiting.push_back(waits),
            false => self.start(waits, cx),
        }
    }

    fn start(&mut self, waits: Waiting, cx: &mut Context<Self>) {
        self.tasks.running = true;
        self.tasks.note = waits.doing;
        self.tasks.updating = waits.updating;
        crate::jobs::run("download task", cx, waits.work, |downloads, done, cx| {
            downloads.finish(done, cx);
            if let Some(next) = downloads.tasks.waiting.pop_front() {
                downloads.start(next, cx);
            }
        });
        cx.notify();
    }

    fn finish(&mut self, done: Done, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        if done.wrote {
            app.rescan_soon();
        }
        self.tasks.note = match done.note {
            Ok(did) => {
                let said = put(&did);
                app.note(did, String::new());
                said
            }
            Err(problem) => put(problem.sentence()),
        };
        self.steamcmd = app.steamcmd().is_some();
        self.tasks.updating = None;
        if self.tasks.cloning.take().is_some()
            && let Ok(queue) = &mut self.looked_up
        {
            rimstack_core::sources::downloads::recheck(&mut queue.repositories);
        }
        self.tasks.running = false;
        cx.notify();
    }

    fn ask_steam(&mut self, what: SteamAsk, ids: Vec<String>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let app = crate::ui::app(cx);
        self.task(
            say!("Asking Steam…"),
            None,
            move || app.ask_steam(what, &ids),
            cx,
        );
    }

    pub fn update_in_steam(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.updates.by_steam().map(|row| row.id.clone()).collect();
        self.asked_of_steam.extend(ids.iter().cloned());
        self.ask_steam(SteamAsk::Download, ids, cx);
    }

    pub(super) fn apply(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .updates
            .by_steam()
            .map(|row| row.id.clone())
            .filter(|id| !self.asked_of_steam.contains(id))
            .collect();
        let copies: Vec<String> = match self.tasks.running {
            true => Vec::new(),
            false => self
                .updates
                .copies_by_itself()
                .map(|row| row.id.clone())
                .collect(),
        };
        let checkouts: Vec<ModId> = self
            .updates
            .checkouts
            .iter()
            .filter(|c| c.due() && !c.workshop)
            .map(|c| c.mod_id)
            .collect();
        if ids.is_empty() && checkouts.is_empty() && copies.is_empty() {
            return;
        }
        let app = crate::ui::app(cx);
        let any_checkout = !checkouts.is_empty() || !copies.is_empty();
        crate::jobs::run(
            "updates applied",
            cx,
            move || {
                let running = any_checkout && app.game_running();
                let asked = !ids.is_empty();
                let took = asked && app.ask_steam(SteamAsk::Download, &ids).note.is_ok();
                (running, asked, took, ids)
            },
            move |downloads, (running, asked, took, ids), cx| {
                downloads.steam_waits = asked && !took;
                if took {
                    downloads.asked_of_steam.extend(ids);
                }
                if !running {
                    for id in checkouts {
                        downloads.update_from_git(id, cx);
                    }
                    downloads.update_copies(copies, cx);
                }
                cx.notify();
            },
        );
    }

    pub(super) fn answer(
        &self,
        run: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Answer {
        let me = cx.entity().downgrade();
        Box::new(move |_, cx| {
            let _ = me.update(cx, run);
        })
    }

    pub fn ask_install_steamcmd(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = self.report.steamcmd_folder.clone();
        let lines = vec![
            Line::Text(put(&say!(
                "Valve's own downloader is fetched from Valve into"
            ))),
            Line::Path(folder),
            Line::Text(put(&say!(
                "It runs with a home folder of its own there, apart from Steam's settings."
            ))),
        ];
        let title = put(&say!("Install SteamCMD"));
        let run = self.answer(Self::install_steamcmd, cx);
        prompt::ask(
            Question {
                title,
                lines,
                label: put(&say!("Install")),
                danger: false,
                run,
            },
            window,
            cx,
        );
    }

    pub fn ask_download(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.list.count;
        let folder = self.report.local_mods.clone().unwrap_or_default();
        let lines = vec![
            Line::Text(put(&say!(
                "Each is downloaded and copied into the game's own Mods folder:"
            ))),
            Line::Path(folder),
            Line::Text(put(&say!("Steam does not update mods installed this way."))),
        ];
        let title = put(&say_n!(
            count,
            "Download {} mod with SteamCMD",
            "Download {} mods with SteamCMD"
        ));
        let run = self.answer(Self::download_wanted, cx);
        prompt::ask(
            Question {
                title,
                lines,
                label: put(&say!("Download")),
                danger: false,
                run,
            },
            window,
            cx,
        );
    }

    pub fn ask_clone(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repository) = self
            .looked_up
            .as_ref()
            .ok()
            .and_then(|queue| queue.repositories.get(at))
            .cloned()
        else {
            return;
        };
        let lines = vec![
            Line::Text(put(&say!("git clone, from"))),
            Line::Path(repository.link),
            Line::Text(put(&say!("Into"))),
            Line::Path(repository.into.unwrap_or_default()),
        ];
        let title = put(&say!("Clone {}", repository.name));
        let run = self.answer(move |d, cx| d.clone_repository(at, cx), cx);
        prompt::ask(
            Question {
                title,
                lines,
                label: put(&say!("Clone")),
                danger: false,
                run,
            },
            window,
            cx,
        );
    }

    pub fn ask_unsubscribe(
        &mut self,
        mod_id: ModId,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = crate::ui::app(cx).mod_path(mod_id).unwrap_or_default();
        let lines = vec![
            Line::Text(put(&say!(
                "Steam must be running. It unsubscribes and then deletes the mod's folder:"
            ))),
            Line::Path(path),
        ];
        let title = put(&say!("Unsubscribe from {}", name));
        let run = self.answer(move |d, cx| d.unsubscribe(mod_id, cx), cx);
        prompt::ask(
            Question {
                title,
                lines,
                label: put(&say!("Unsubscribe")),
                danger: true,
                run,
            },
            window,
            cx,
        );
    }

    pub fn subscribe_wanted(&mut self, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        let count = app.listed_ids().len();
        if count > 0 {
            self.task(
                say!("Subscribing to {} in Steam…", count),
                None,
                move || app.subscribe_listed(),
                cx,
            );
        }
    }

    pub fn download_wanted(&mut self, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        let count = app.listed_ids().len();
        if count > 0 {
            self.task(
                say!("Downloading {} with SteamCMD…", count),
                None,
                move || app.download_listed(),
                cx,
            );
        }
    }

    pub fn unsubscribe(&mut self, mod_id: ModId, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.task(
            say!("Asking Steam…"),
            None,
            move || app.unsubscribe(mod_id),
            cx,
        );
    }

    pub fn subscribe(&mut self, workshop_id: String, cx: &mut Context<Self>) {
        self.ask_steam(SteamAsk::Subscribe, vec![workshop_id], cx);
    }

    pub fn redownload(&mut self, id: ModId, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.task(
            say!("Downloading again with SteamCMD…"),
            None,
            move || app.redownload(id),
            cx,
        );
    }

    pub fn update_from_git(&mut self, id: ModId, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.task(
            say!("Updating from Git…"),
            Some(id),
            move || app.update_checkout(id),
            cx,
        );
    }

    pub fn clone_repository(&mut self, at: usize, cx: &mut Context<Self>) {
        let link = self
            .looked_up
            .as_ref()
            .ok()
            .and_then(|queue| queue.repositories.get(at))
            .map(|r| r.link.clone());
        let Some(link) = link else {
            return;
        };
        self.tasks.cloning = Some(at);
        let app = crate::ui::app(cx);
        self.task(
            say!("Cloning {}…", link.clone()),
            None,
            move || app.clone_repository(&link),
            cx,
        );
    }

    pub fn install_steamcmd(&mut self, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.task(
            say!("Installing SteamCMD…"),
            None,
            move || app.install_steamcmd(),
            cx,
        );
    }

    pub fn fetch_since_notes(&mut self, cx: &mut Context<Self>) {
        let asked = notes_asked(&self.changed);
        self.fetch_notes(asked, cx);
    }

    pub fn fetch_notes_of(
        &mut self,
        id: rimstack_core::mods::library::ModId,
        cx: &mut Context<Self>,
    ) {
        if let Some(workshop) = super::super::app(cx).workshop_id(id) {
            self.fetch_notes(vec![(workshop, 0)], cx);
        }
    }

    fn fetch_notes(&mut self, asked: Vec<(String, u64)>, cx: &mut Context<Self>) {
        if self.notes_busy || self.demo || asked.is_empty() {
            return;
        }
        let several = asked.len() > 1;
        let asked: Vec<(String, u64)> = asked
            .into_iter()
            .filter(|(id, _)| !(several && self.fetched.contains_key(id)))
            .take(MOST_ASKED)
            .collect();
        self.notes_busy = true;
        let (tell, mut told) = futures::channel::mpsc::unbounded();
        let count = asked.len();
        crate::jobs::spawn("change notes", move || {
            let mut said = |fetching| {
                let _ = tell.unbounded_send(Some(fetching));
            };
            changes::fetch_notes(
                &asked,
                MOST_NOTES,
                &changes::fetch_steam_notes,
                &std::thread::sleep,
                &now_secs,
                &mut said,
            );
            let _ = tell.unbounded_send(None);
        });
        cx.spawn(async move |downloads, cx| {
            use futures::StreamExt;
            while let Some(heard) = told.next().await {
                let going = downloads.update(cx, |downloads, cx| {
                    let state = |done, resume, refused| NotesState {
                        done,
                        asked: count,
                        resume,
                        refused,
                    };
                    match heard {
                        Some(Fetching::Under { done }) => {
                            downloads.notes_state = Some(state(done, 0, false))
                        }
                        Some(Fetching::Waiting { done, resume }) => {
                            downloads.notes_state = Some(state(done, resume, false))
                        }
                        Some(Fetching::GaveUp { done }) => {
                            downloads.notes_state = Some(state(done, 0, true));
                            crate::ui::notices::say(
                                put(&say_n!(
                                    count,
                                    "Steam's site keeps refusing: asked about {} mod, got the change notes of {}",
                                    "Steam's site keeps refusing: asked about {} mods, got the change notes of {}",
                                    done
                                )),
                                crate::ui::notices::Tone::Err,
                                cx,
                            );
                        }
                        Some(Fetching::Fetched { id, notes }) => {
                            std::rc::Rc::make_mut(&mut downloads.fetched).insert(id, notes);
                        }
                        None => downloads.notes_busy = false,
                    }
                    cx.notify();
                });
                if going.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }
}
