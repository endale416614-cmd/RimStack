mod line;
mod parts;
mod parts_more;
mod screen;

use super::page::Paging;
use super::text_field::{Edited, Entered, TextField};
use super::theme::Theme;
use crate::changes::Changes;
use crate::jobs::Lane;
use crate::reports::{Database, FolderVerdict, KeyShown, ReleaseSaid, SettingsReport};
use crate::words::put;
use crate::workspace::KeyState;
use gpui::{AppContext, Context, Entity, SharedString, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::settings::UPDATE_EVERY;
use rimstack_core::say;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const FOLDER_PAUSE: Duration = Duration::from_millis(400);
const SETTING_PAUSE: Duration = Duration::from_millis(600);

const FOLDERS: [&str; 3] = ["game", "config", "workshop"];

struct Fields {
    find: Entity<TextField>,
    folders: [Entity<TextField>; 3],
    launch: Entity<TextField>,
    key: Entity<TextField>,
    fonts: Entity<TextField>,
}

#[derive(Default)]
struct Said {
    language: String,
    own_theme: String,
    mod_settings: String,
    rimsort: String,
}

const READS: Changes = Changes::SETTINGS
    .or(Changes::FOLDERS)
    .or(Changes::LIBRARY)
    .or(Changes::LOGS)
    .or(Changes::COMMUNITY)
    .or(Changes::LANGUAGE)
    .or(Changes::THEME)
    .or(Changes::KEY);

pub struct Settings {
    unseen: crate::ui::section::Unseen,
    demo: bool,
    report: Rc<SettingsReport>,
    /// The folder boxes and the launch arguments take the report's text once, when it comes.
    filled: bool,
    databases: Rc<Vec<Database>>,
    languages: Rc<Vec<String>>,
    folder_checks: HashMap<&'static str, FolderVerdict>,
    key: Option<KeyState>,
    key_checking: bool,
    key_failure: Option<Sentence>,
    community_busy: bool,
    community_current: bool,
    community_problem: Option<Sentence>,
    release: Option<ReleaseSaid>,
    release_busy: bool,
    problem: Option<Sentence>,
    part: usize,
    finding: String,
    fields: Fields,
    said: Said,
    fonts_open: bool,
    fonts: Rc<Vec<SharedString>>,
    typing: HashMap<&'static str, Lane>,
    reading: Lane,
    unread: Changes,
    paging: Paging,
    font_scroll: super::rows::Scrolling,
    prefs: rimstack_core::kept::prefs::Prefs,
    part_keys: gpui::FocusHandle,
}

impl Settings {
    pub fn new(demo: bool, part: usize, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let field = |cx: &mut Context<Self>| cx.new(|cx| TextField::new(theme.body(), cx));
        let fields = Fields {
            find: field(cx),
            folders: [field(cx), field(cx), field(cx)],
            launch: field(cx),
            key: field(cx),
            fonts: field(cx),
        };
        fields.key.update(cx, |field, _| field.set_masked(true));
        cx.subscribe(&fields.find, |settings, field, _: &Edited, cx| {
            settings.finding = field.read(cx).text().to_string();
            cx.notify();
        })
        .detach();
        for (at, folder) in fields.folders.iter().enumerate() {
            cx.subscribe(folder, move |settings, field, _: &Edited, cx| {
                let typed = field.read(cx).text().to_string();
                settings.check_folder(FOLDERS[at], typed, cx);
            })
            .detach();
        }
        cx.subscribe(&fields.launch, |settings, field, _: &Edited, cx| {
            let typed = field.read(cx).text().to_string();
            settings.keep_later("launchArguments", typed, cx);
        })
        .detach();
        cx.subscribe(&fields.key, |settings, _, _: &Entered, cx| {
            settings.check_key(cx)
        })
        .detach();
        cx.observe(&fields.fonts, |_, _, cx| cx.notify()).detach();
        let reduce = theme.appearance.reduce_motion;
        let mut settings = Self {
            unseen: Default::default(),
            demo,
            report: Rc::default(),
            filled: false,
            databases: Rc::default(),
            languages: Rc::default(),
            folder_checks: HashMap::new(),
            key: None,
            key_checking: false,
            key_failure: None,
            community_busy: false,
            community_current: false,
            community_problem: None,
            release: None,
            release_busy: false,
            problem: None,
            part,
            finding: String::new(),
            fields,
            said: Said::default(),
            fonts_open: false,
            fonts: Rc::default(),
            typing: HashMap::new(),
            reading: Lane::default(),
            unread: Changes::NONE,
            paging: Paging::new(reduce),
            font_scroll: super::rows::Scrolling::default(),
            prefs: super::app(cx).prefs(),
            part_keys: cx.focus_handle().tab_stop(true),
        };
        if !demo {
            settings.hear(cx);
        }
        settings.heard(
            Changes::SETTINGS | Changes::LANGUAGE | Changes::COMMUNITY | Changes::KEY,
            cx,
        );
        settings
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, Self::heard);
    }

    fn heard(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(changes) = self.unseen.pass(changes) else {
            return;
        };
        let app = super::app(cx);
        // A read asked for while another runs takes its place, so it reads what both asked.
        self.unread |= changes;
        let changes = self.unread;
        let report = changes
            .contains(Changes::SETTINGS | Changes::FOLDERS | Changes::LIBRARY | Changes::LOGS);
        let databases =
            changes.contains(Changes::SETTINGS | Changes::COMMUNITY | Changes::LANGUAGE);
        let languages = changes.contains(Changes::LANGUAGE);
        let read = move || {
            (
                report.then(|| app.settings_report()),
                databases.then(|| app.databases()),
                languages.then(|| app.languages()),
            )
        };
        let show = |settings: &mut Self,
                    read: (
            Option<SettingsReport>,
            Option<Vec<Database>>,
            Option<Vec<String>>,
        ),
                    cx: &mut Context<Self>| {
            settings.unread = Changes::NONE;
            let (report, databases, languages) = read;
            if let Some(report) = report {
                settings.report = Rc::new(report);
                settings.fill(cx);
            }
            if let Some(databases) = databases {
                settings.databases = Rc::new(databases);
            }
            if let Some(languages) = languages {
                settings.languages = Rc::new(languages);
            }
            cx.notify();
        };
        match report || databases || languages {
            false => self.unread = Changes::NONE,
            true => self.reading.run("settings read", cx, move |_| read(), show),
        }
        if changes.contains(Changes::THEME) {
            self.prefs = super::app(cx).prefs();
        }
        if changes.contains(Changes::KEY) && !self.demo {
            self.read_key(cx);
        }
    }

    /// The boxes take what is kept once the report first comes; after that, what is typed stays.
    fn fill(&mut self, cx: &mut Context<Self>) {
        if self.filled {
            return;
        }
        self.filled = true;
        let report = self.report.clone();
        let kept = [&report.set_game, &report.set_config, &report.set_workshop];
        for (field, kept) in self.fields.folders.iter().zip(kept) {
            field.update(cx, |field, cx| {
                field.set_text(kept.as_deref().unwrap_or_default(), cx)
            });
        }
        let launch = report.launch_arguments.clone().unwrap_or_default();
        self.fields
            .launch
            .update(cx, |field, cx| field.set_text(&launch, cx));
    }

    fn read_key(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "steam key state",
            cx,
            move || app.key_state(),
            |settings, state, cx| {
                settings.key = Some(state);
                cx.notify();
            },
        );
    }

    fn key_shown(&self) -> Option<KeyShown> {
        let state = self.key.clone()?;
        Some(KeyShown::of(
            state,
            self.key_checking,
            self.key_failure.clone(),
        ))
    }

    pub fn show_part(&mut self, part: usize, cx: &mut Context<Self>) {
        self.part = part;
        cx.notify();
    }

    pub(super) fn pick_part(&mut self, part: usize, cx: &mut Context<Self>) {
        if part >= self.parts().len() {
            return;
        }
        self.fields
            .find
            .update(cx, |field, cx| field.set_text("", cx));
        self.finding.clear();
        self.part = part;
        self.paging.scroll.borrow_mut().set_top(gpui::px(0.));
        cx.notify();
    }

    pub fn community_busy(&self) -> bool {
        self.community_busy
    }

    pub fn keep(&mut self, name: &str, value: &str, cx: &mut Context<Self>) {
        self.problem = super::app(cx)
            .keep_setting(name, value)
            .err()
            .map(|e| e.sentence().clone());
        cx.notify();
    }

    fn keep_update_every(&mut self, at: usize, cx: &mut Context<Self>) {
        if let Some(minutes) = UPDATE_EVERY.get(at) {
            self.keep("updateEvery", minutes, cx);
        }
    }

    fn lane(&mut self, name: &'static str) -> &mut Lane {
        self.typing.entry(name).or_default()
    }

    fn check_folder(&mut self, name: &'static str, typed: String, cx: &mut Context<Self>) {
        let typed = typed.trim().to_string();
        if super::app(cx).settings().get(name).unwrap_or_default() == typed {
            self.lane(name).cancel();
            self.folder_checks.remove(name);
            return cx.notify();
        }
        let judged = typed.clone();
        self.lane(name).run_after(
            FOLDER_PAUSE,
            "folder check",
            cx,
            move |_| crate::workspace::judge_folder(name, &judged),
            move |settings, verdict, cx| {
                let app = super::app(cx);
                if app.settings().get(name).unwrap_or_default() == typed {
                    settings.folder_checks.remove(name);
                } else {
                    if verdict.ok {
                        settings.keep(name, &typed, cx);
                    }
                    settings.folder_checks.insert(name, verdict);
                }
                cx.notify();
            },
        );
    }

    fn keep_later(&mut self, name: &'static str, typed: String, cx: &mut Context<Self>) {
        let typed = typed.trim().to_string();
        if super::app(cx).settings().get(name).unwrap_or_default() == typed {
            self.lane(name).cancel();
            return;
        }
        self.lane(name)
            .after(SETTING_PAUSE, cx, move |settings, cx| {
                let same = super::app(cx).settings().get(name).unwrap_or_default() == typed;
                if !same {
                    settings.keep(name, &typed, cx);
                }
            });
    }

    fn check_key(&mut self, cx: &mut Context<Self>) {
        let typed = self.fields.key.read(cx).text().to_string();
        if self.key_checking || typed.is_empty() {
            return;
        }
        self.fields
            .key
            .update(cx, |field, cx| field.set_text("", cx));
        self.key_checking = true;
        self.key_failure = None;
        let app = super::app(cx);
        crate::jobs::run(
            "steam key check",
            cx,
            move || app.check_and_keep(&typed).err(),
            |settings, failure, cx| {
                settings.key_checking = false;
                settings.key_failure = failure;
                settings.read_key(cx);
            },
        );
        cx.notify();
    }

    fn forget_key(&mut self, cx: &mut Context<Self>) {
        if self.key_checking {
            return;
        }
        self.key_checking = true;
        self.key_failure = None;
        let app = super::app(cx);
        crate::jobs::run(
            "steam key forget",
            cx,
            move || app.forget_key().err(),
            |settings, failure, cx| {
                settings.key_checking = false;
                settings.key_failure = failure;
                settings.read_key(cx);
            },
        );
        cx.notify();
    }

    fn download_community(&mut self, cx: &mut Context<Self>) {
        if self.community_busy || self.demo {
            return;
        }
        self.community_busy = true;
        self.community_problem = None;
        let app = super::app(cx);
        crate::jobs::run(
            "community download",
            cx,
            move || app.download_databases(),
            |settings, done, cx| {
                settings.community_current = done.is_ok();
                settings.community_problem = done.err().map(|e| e.sentence().clone());
                settings.community_busy = false;
                cx.notify();
            },
        );
        cx.notify();
    }

    fn check_release(&mut self, cx: &mut Context<Self>) {
        if self.release_busy || self.demo {
            return;
        }
        self.release_busy = true;
        let nightly = super::app(cx).settings().nightly_updates();
        crate::jobs::run(
            "release check",
            cx,
            move || crate::workspace::newest_release(nightly),
            |settings, found, cx| {
                settings.release = Some(found);
                settings.release_busy = false;
                cx.notify();
            },
        );
        cx.notify();
    }

    fn use_language(&mut self, name: &str, cx: &mut Context<Self>) {
        self.keep("language", name, cx);
        let app = super::app(cx);
        crate::words::show_in(crate::words::Words::of(&app.speak()));
        if !self.demo {
            app.rescan_soon();
        }
        cx.refresh_windows();
    }

    fn use_font(&mut self, family: &str, cx: &mut Context<Self>) {
        self.keep("font", family, cx);
        self.fonts_open = false;
        super::use_font(family, cx);
    }

    fn open_file(
        &self,
        file: Result<std::path::PathBuf, rimstack_core::base::error::Error>,
        cx: &mut Context<Self>,
    ) -> String {
        match file {
            Ok(file) => {
                cx.open_with_system(&file);
                String::new()
            }
            Err(problem) => put(problem.sentence()),
        }
    }

    fn start_language(&mut self, cx: &mut Context<Self>) {
        let name = self.report.language_in_use.clone();
        let made = super::app(cx).start_language(&name);
        self.said.language = self.open_file(made, cx);
        cx.notify();
    }

    fn start_own_theme(&mut self, cx: &mut Context<Self>) {
        let appearance = cx.global::<Theme>().appearance.clone();
        let colours: serde_json::Map<String, serde_json::Value> = super::theme::COLOUR_NAMES
            .iter()
            .filter_map(|name| Some((name.to_string(), appearance.colour_text(name)?.into())))
            .collect();
        let made = super::app(cx).start_own_theme(appearance.theme.name(), &colours.into());
        self.said.own_theme = self.open_file(made, cx);
        cx.notify();
    }

    fn reset_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let resets: Vec<_> = self
            .lines(window, cx)
            .into_iter()
            .filter_map(|row| match row {
                line::Row::Line(line) if line.changed && self.shows(&line) => line.reset,
                _ => None,
            })
            .collect();
        for reset in resets {
            reset(self, window, cx);
        }
    }

    fn shows(&self, line: &line::Line) -> bool {
        let finding = self.finding.trim();
        if finding.is_empty() {
            return line.part == self.part;
        }
        let words = format!("{} {}", line.title, line.tip).to_lowercase();
        finding
            .to_lowercase()
            .split(' ')
            .filter(|w| !w.is_empty())
            .all(|word| words.contains(word))
    }

    fn say_hints(&mut self, cx: &mut Context<Self>) {
        super::keep_appearance("hintsSeen", "false", cx);
        super::navigate::go(super::navigate::Go::StartHints, cx);
        super::notices::say(
            put(&say!("The hints show on the Mods screen")),
            super::notices::Tone::Plain,
            cx,
        );
    }
}

impl crate::ui::section::Shown for Settings {
    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if let Some(missed) = self.unseen.shown(shown) {
            self.heard(missed, cx);
        }
    }
}

#[cfg(test)]
mod tests;
