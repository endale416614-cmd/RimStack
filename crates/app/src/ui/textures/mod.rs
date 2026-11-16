mod files;
mod row;
mod screen;
mod spare;

use super::rows::Scrolling;
use super::theme::Theme;
use crate::changes::Changes;
use crate::words::put;
use crate::workspace::textures::{Refused, SpareFound};
use files::{FileList, Group, Item, Note, Which};
use gpui::{Context, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::textures::Row;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const READS: Changes = Changes::LIBRARY
    .or(Changes::TEXTURES)
    .or(Changes::LOAD_ORDER)
    .or(Changes::LANGUAGE);

pub struct Textures {
    unseen: crate::ui::section::Unseen,
    demo: bool,
    rows: Rc<Vec<Row>>,
    /// Up to 1.6, where a DDS loads faster; `None` until the mods are counted.
    dds_faster: Option<bool>,
    busy: bool,
    /// Counted again once the count under way ends.
    stale: bool,
    progress: String,
    found: Option<(Rc<Vec<Refused>>, FileList)>,
    spare: Option<(Rc<SpareFound>, FileList)>,
    auditing: bool,
    active_only: bool,
    selected: Option<ModId>,
    confirm_remove: bool,
    pub(crate) stop: Arc<AtomicBool>,
    scroll: Scrolling,
    focus: gpui::FocusHandle,
}

impl Textures {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let scroll: Scrolling = Scrolling::default();
        scroll
            .borrow_mut()
            .set_reduce_motion(theme.appearance.reduce_motion);
        let mut textures = Self {
            unseen: Default::default(),
            demo,
            rows: Rc::default(),
            dds_faster: None,
            busy: false,
            stale: true,
            progress: String::new(),
            found: None,
            spare: None,
            auditing: false,
            active_only: true,
            selected: None,
            confirm_remove: false,
            stop: Arc::default(),
            scroll,
            focus: cx.focus_handle(),
        };
        if !demo {
            textures.hear(cx);
            textures.count(cx);
        }
        textures
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(changes) = self.unseen.pass(changes) else {
            return;
        };
        if changes.contains(Changes::LIBRARY | Changes::TEXTURES | Changes::LOAD_ORDER) {
            self.stale = true;
            self.count(cx);
        } else if changes.contains(Changes::LANGUAGE) {
            cx.notify();
        }
    }

    /// Every mod's textures counted, off the window's thread; one count at a time.
    fn count(&mut self, cx: &mut Context<Self>) {
        if !self.stale || self.busy || self.demo {
            return;
        }
        self.stale = false;
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "textures count",
            cx,
            move || app.texture_rows(),
            |textures, counted, cx| {
                textures.rows = Rc::new(counted.rows);
                textures.dds_faster = Some(counted.converts);
                textures.busy = false;
                textures.count(cx);
                cx.notify();
            },
        );
        cx.notify();
    }

    fn work(&mut self, ids: Vec<ModId>, remove: bool, cx: &mut Context<Self>) {
        if self.busy || ids.is_empty() || self.demo {
            return;
        }
        self.busy = true;
        self.stop.store(false, Ordering::Relaxed);
        let (app, stop) = (super::app(cx), self.stop.clone());
        let (tell, mut told) =
            futures::channel::mpsc::unbounded::<Result<Sentence, (Vec<Row>, Sentence)>>();
        crate::jobs::spawn("textures convert", move || {
            let progress = |text: Sentence| {
                let _ = tell.unbounded_send(Ok(text));
            };
            let note = app.convert_textures(&ids, remove, &stop, &progress);
            let _ = tell.unbounded_send(Err((app.texture_rows().rows, note)));
        });
        cx.spawn(async move |textures, cx| {
            use futures::StreamExt;
            while let Some(heard) = told.next().await {
                let going = textures.update(cx, |textures, cx| {
                    match heard {
                        Ok(progress) => textures.progress = put(&progress),
                        Err((rows, note)) => {
                            textures.rows = Rc::new(rows);
                            textures.stale = false;
                            textures.progress = put(&note);
                            textures.busy = false;
                        }
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

    pub(crate) fn task_said(&self) -> Option<String> {
        use rimstack_core::say;
        self.busy.then(|| match self.progress.is_empty() {
            true => put(&say!("Counting textures…")),
            false => self.progress.clone(),
        })
    }

    fn skip(&mut self, id: ModId, skip: bool, cx: &mut Context<Self>) {
        if let Err(problem) = super::app(cx).skip_textures(id, skip) {
            rimstack_core::problem!("{problem}");
        }
    }

    fn audit(&mut self, cx: &mut Context<Self>) {
        if self.auditing || self.demo {
            return;
        }
        self.auditing = true;
        let app = super::app(cx);
        crate::jobs::run(
            "dds audit",
            cx,
            move || {
                let found = app.refused_dds();
                let groups = refused_by_mod(&found);
                (found, groups)
            },
            |textures, (found, groups), cx| {
                textures.found = Some((Rc::new(found), FileList::new(groups)));
                textures.auditing = false;
                cx.notify();
            },
        );
        cx.notify();
    }

    fn set_refused_aside(&mut self, cx: &mut Context<Self>) {
        let Some((found, _)) = self.found.take() else {
            return;
        };
        let failed = super::app(cx).set_dds_aside(&found);
        if failed > 0 {
            rimstack_core::problem!("{failed} DDS files could not be set aside");
        }
        self.audit(cx);
    }

    pub(crate) fn find_spare(&mut self, cx: &mut Context<Self>) {
        use rimstack_core::say;
        if self.busy || self.demo {
            return;
        }
        self.busy = true;
        self.stop.store(false, Ordering::Relaxed);
        self.progress = put(&say!("Comparing each DDS with its PNG…"));
        let (app, stop) = (super::app(cx), self.stop.clone());
        crate::jobs::run(
            "spare dds",
            cx,
            move || {
                let found = app.spare_dds(&stop);
                let groups = spare_by_mod(&found);
                (found, groups)
            },
            |textures, (found, groups), cx| {
                textures.spare = Some((Rc::new(found), FileList::new(groups)));
                textures.busy = false;
                textures.progress.clear();
                cx.notify();
            },
        );
        cx.notify();
    }

    fn delete_spare(&mut self, cx: &mut Context<Self>) {
        let Some((found, _)) = self.spare.take() else {
            return;
        };
        let found = Rc::unwrap_or_clone(found);
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "spare dds delete",
            cx,
            move || app.delete_spare_dds(&found),
            |textures, said, cx| {
                textures.busy = false;
                textures.progress = put(&said);
                textures.count(cx);
                cx.notify();
            },
        );
        cx.notify();
    }

    fn ask_delete_spare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use super::prompt::{self, Line, Question};
        use rimstack_core::{say, say_n};
        let Some(found) = self.spare.as_ref().map(|(f, _)| f.clone()) else {
            return;
        };
        let Some(first) = found.first_file() else {
            return;
        };
        let me = cx.entity().downgrade();
        let question = Question {
            title: put(&say_n!(
                found.files(),
                "Delete {} DDS file",
                "Delete {} DDS files"
            )),
            lines: vec![
                Line::Text(put(&say!(
                    "Each shows the same picture as the PNG beside it, which stays, and the game makes its own copy from the PNG. Steam brings a mod's files back when it updates or checks the mod. The first of them:"
                ))),
                Line::Path(first.to_string_lossy().into_owned()),
            ],
            label: put(&say!("Delete them")),
            danger: true,
            run: Box::new(move |_, cx| {
                let _ = me.update(cx, |textures, cx| textures.delete_spare(cx));
            }),
        };
        prompt::ask(question, window, cx);
    }

    fn ask_refused_aside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use super::prompt::{self, Line, Question};
        use rimstack_core::say;
        let Some(found) = self
            .found
            .as_ref()
            .map(|(f, _)| f.clone())
            .filter(|f| !f.is_empty())
        else {
            return;
        };
        let me = cx.entity().downgrade();
        let question = Question {
            title: put(&say!("Set {} DDS files aside", found.len())),
            lines: vec![
                Line::Text(put(&say!(
                    "Each is renamed to .dds.rimstack-bad inside its own mod's folder, so the game loads the mod's own texture instead. The first of them:"
                ))),
                Line::Path(found[0].file.clone()),
            ],
            label: put(&say!("Set aside")),
            danger: false,
            run: Box::new(move |_, cx| {
                let _ = me.update(cx, |textures, cx| textures.set_refused_aside(cx));
            }),
        };
        prompt::ask(question, window, cx);
    }
}

impl Textures {
    fn converts(&self) -> bool {
        self.dds_faster == Some(true)
    }

    fn refuses(&self) -> bool {
        self.dds_faster == Some(false)
    }

    pub(super) fn toggle_files(&mut self, which: Which, group: usize, cx: &mut Context<Self>) {
        let list = match which {
            Which::Refused => self.found.as_mut().map(|(_, l)| l),
            Which::Spare => self.spare.as_mut().map(|(_, l)| l),
        };
        if let Some(list) = list {
            list.toggle(group);
            cx.notify();
        }
    }
}

fn refused_by_mod(found: &[Refused]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut sorted: Vec<&Refused> = found.iter().collect();
    sorted.sort_by(|a, b| a.of_mod.cmp(&b.of_mod));
    for refused in sorted {
        let item = Item {
            shown: refused.name.clone(),
            note: Note::Why(refused.why.clone()),
        };
        match groups.last_mut() {
            Some(group) if group.name == refused.of_mod => group.items.push(item),
            _ => groups.push(Group {
                name: refused.of_mod.clone(),
                bytes: None,
                items: vec![item],
            }),
        }
    }
    groups
}

fn spare_by_mod(found: &SpareFound) -> Vec<Group> {
    found
        .mods
        .iter()
        .map(|m| Group {
            name: m.name.clone(),
            bytes: Some(m.bytes),
            items: m
                .files
                .iter()
                .map(|f| Item {
                    shown: f.shown.clone(),
                    note: Note::Bytes(f.spare.bytes),
                })
                .collect(),
        })
        .collect()
}

/// `megabytes(n)`: KB under a tenth of a megabyte, one decimal under ten, whole above.
fn megabytes(n: f64) -> String {
    use rimstack_core::say;
    if n <= 0.0 {
        return String::new();
    }
    if n < 0.1 {
        return put(&say!("{} KB", ((n * 1024.0).round() as usize).max(1)));
    }
    match n < 10.0 {
        true => put(&say!("{} MB", format!("{n:.1}"))),
        false => put(&say!("{} MB", crate::words::count(n.round() as usize))),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

impl crate::ui::section::Shown for Textures {
    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if let Some(missed) = self.unseen.shown(shown) {
            self.heard(missed, cx);
        }
    }
}
