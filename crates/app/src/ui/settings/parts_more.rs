use super::Settings;
use super::line::{Line, Row, group, path, under, words};
use super::parts::{act, said};
use crate::reports::ReleaseSaid;
use crate::ui::button::Btn;
use crate::ui::controls::{Choice, Toggle};
use crate::ui::field::Field;
use crate::ui::prompt::{self, Line as Asked, Question};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use crate::workspace::logs::day;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div};
use rimstack_core::{say, say_n};

const ISSUES: &str = "https://github.com/TheForgotten69/RimStack/issues";
const KEY_PAGE: &str = "https://steamcommunity.com/dev/apikey";
const VERIFY: &str = "steam://validate/294100";
const LOGS_KEPT: [usize; 5] = [0, 5, 10, 25, 50];

impl Settings {
    pub(super) fn sort_part(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
        rows: &mut Vec<Row>,
    ) {
        let c = &theme.colours;
        let report = self.report.clone();
        rows.push(Row::Group(2, put(&say!("Sort follows"))));
        let community = report.community_rules;
        rows.push(Row::Line(
            Line::new(2, "community-rules", put(&say!("Community rules")))
                .changed(!community, |s, _, cx| s.keep("communityRules", "on", cx))
                .held(
                    Toggle::new(
                        "community-rules-toggle",
                        community,
                        put(&say!("Community databases used by Sort")),
                    )
                    .on_flip(act(cx, |s, on: bool, _, cx| {
                        s.keep("communityRules", if on { "on" } else { "off" }, cx)
                    })),
                )
                .filled(),
        ));
        let hints = report.file_hints;
        rows.push(Row::Line(
            Line::new(2, "file-hints", put(&say!("Patch and def hints")))
                .tip(put(&say!("A mod whose patches or defs point at another mod's defs. The game applies patches after every mod's defs are loaded, so these are hints, not requirements.")))
                .changed(!hints, |s, _, cx| s.keep("fileHints", "on", cx))
                .held(Choice::new("file-hints-choice", said(&[say!("Shown only"), say!("Used by Sort")]), usize::from(hints)).within(Self::choice_room(theme, window)).title(put(&say!("Patch and def hints"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.keep("fileHints", if at == 1 { "on" } else { "off" }, cx)
                })))
                .filled(),
        ));

        rows.push(Row::Group(2, put(&say!("Community databases"))));
        let overview = crate::ui::order::read(cx).overview;
        let (mods, time) = overview
            .as_ref()
            .map_or((None, None), |o| (o.community_mods, o.community_time));
        let downloaded = match mods {
            Some(n) if n > 0 => (
                c.ink,
                put(&say_n!(
                    n,
                    "{} mod, published {}",
                    "{} mods, published {}",
                    day(time.unwrap_or_default().max(0) as u64)
                )),
            ),
            _ => (c.ink3, put(&say!("Not downloaded · 49 MB"))),
        };
        let (busy, current) = (self.community_busy, self.community_current);
        let fetch = match (busy, current, mods.is_some_and(|n| n > 0)) {
            (true, _, _) => put(&say!("Downloading…")),
            (false, true, _) => put(&say!("Up to date")),
            (false, false, true) => put(&say!("Update")),
            (false, false, false) => put(&say!("Download")),
        };
        rows.push(Row::Line(
            Line::new(2, "downloaded", put(&say!("Downloaded")))
                .tip(put(&say!("From GitHub, on request: Community Rules Database, No Version Warning, Use This Instead, Steam Workshop Database (49 MB).")))
                .held(label(theme, theme.body(), downloaded.0, downloaded.1))
                .held(
                    Btn::new("community-download", fetch)
                        .icon(if current { "check" } else { "download" })
                        .enabled(!busy && !current)
                        .on_click(cx.listener(|s, _, _, cx| s.download_community(cx))),
                )
                .filled(),
        ));
        for (at, database) in self.databases.iter().enumerate() {
            let id = database.id;
            rows.push(Row::Line(
                Line::new(2, "database", database.name.to_string())
                    .held(
                        Toggle::new(("database", at), database.on, database.name)
                            .dimmed(database.waiting)
                            .when_silent(database.waiting)
                            .on_flip(act(cx, move |s, on: bool, _, cx| {
                                s.problem = crate::ui::app(cx)
                                    .use_database(id, on)
                                    .err()
                                    .map(|e| e.sentence().clone());
                                cx.notify();
                            })),
                    )
                    .held(words(theme, c.ink, put(&database.used_for)))
                    .when_held(
                        database.updated > 0,
                        label(theme, theme.small(), c.ink2, day(database.updated)),
                    ),
            ));
        }
        if let Some(problem) = &self.community_problem {
            rows.push(Row::Under(
                2,
                under(
                    theme,
                    c.err,
                    theme.body(),
                    put(&say!("Not updated: {}", problem.inside())),
                ),
            ));
        }

        if report.rimsort_rules {
            rows.push(Row::Group(2, put(&say!("RimSort"))));
            rows.push(Row::Line(
                Line::new(2, "rimsort", put(&say!("Your rules in RimSort")))
                    .tip(put(&say!("Copies the rules you made in RimSort into RimStack. RimSort's file is only read.")))
                    .held(Btn::new("rimsort-import", put(&say!("Import"))).icon("download").on_click(cx.listener(|s, _, _, cx| {
                        s.said.rimsort = put(&crate::ui::app(cx).import_rimsort_rules());
                        cx.notify();
                    })))
                    .held(words(theme, c.ink2, self.said.rimsort.clone())),
            ));
        }

        rows.push(Row::Group(2, put(&say!("Game version"))));
        let game = report.game_version.clone().unwrap_or_default();
        let aside = !game.is_empty() && report.version_aside_for.as_deref() == Some(game.as_str());
        let current = if !report.version_warnings {
            2
        } else {
            usize::from(aside)
        };
        rows.push(Row::Line(
            Line::new(2, "version-warnings", put(&say!("Game version mismatch")))
                .tip(put(&say!("A mod that does not list the installed game version. For this version: the warnings come back when the game is updated. A single mod can be ignored from its menu.")))
                .changed(!report.version_warnings || aside, |s, _, cx| s.version_warnings(0, cx))
                .held(Choice::new("version-choice", said(&[say!("Warn"), say!("Ignore for this version"), say!("Ignore")]), current).within(Self::choice_room(theme, window)).title(put(&say!("Game version mismatch"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.version_warnings(at as i32, cx)
                })))
                .filled(),
        ));
        let trusted = overview
            .as_ref()
            .map(|o| o.trusted.clone())
            .unwrap_or_default();
        if !trusted.is_empty() {
            rows.push(Row::Under(
                2,
                group(
                    theme,
                    put(&say!("Marked as working on RimWorld {}", game.clone())),
                ),
            ));
            for (at, named) in trusted.into_iter().enumerate() {
                let id = named.mod_id;
                let tone = if named.trouble { c.err } else { c.ink3 };
                let note = format!(
                    "{}{}",
                    if named.trouble { "! " } else { "" },
                    put(&named.note)
                );
                rows.push(Row::Under(
                    2,
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP)
                        .h(theme.sized(28.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(named.name),
                        )
                        .child(label(theme, theme.small(), tone, note))
                        .child(Btn::new(("untrust", at), put(&say!("Remove"))).on_click(
                            move |_, _, cx| {
                                if let Some(id) = id {
                                    crate::ui::navigate::go(
                                        crate::ui::navigate::Go::Distrust(id),
                                        cx,
                                    );
                                }
                            },
                        ))
                        .into_any_element(),
                ));
            }
        }
    }

    pub(super) fn key_part(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
        rows: &mut Vec<Row>,
    ) {
        let c = &theme.colours;
        let key = self.key_shown().unwrap_or_default();
        rows.push(Row::Line(Line::new(3, "key-adds", put(&say!("What a key adds"))).held(words(theme, c.ink, put(&say!("With a key, Browse searches Steam itself, with Steam's sort orders. It also shows mods newer than the downloaded database, collections, more by the same author, votes and screenshots. Without a key, Browse searches the downloaded database."))))));
        rows.push(Row::Line(Line::new(3, "key-private", put(&say!("Keep it private"))).held(words(theme, c.warn, put(&say!("Whoever has this key can read and cancel your trade offers. Never share it with anyone. This is Steam's own warning. RimStack keeps the key in this computer's credential store, never in its settings file."))))));
        rows.push(Row::Line(
            Line::new(3, "key-get", put(&say!("Get a key")))
                .tip(put(&say!(
                    "Steam's page asks you to sign in and name a domain; any name will do."
                )))
                .held(
                    Btn::new("key-page", put(&say!("Open Steam's key page")))
                        .icon("external-link")
                        .on_click(|_, _, cx| cx.open_url(KEY_PAGE)),
                )
                .filled(),
        ));
        let state = match (key.available, key.stored) {
            (true, true) => (c.ok, put(&say!("A key is kept."))),
            (true, false) => (c.ink3, put(&say!("No key is kept."))),
            (false, _) => (
                c.err,
                put(&say!(
                    "The credential store does not work, so a key cannot be kept."
                )),
            ),
        };
        let mut yours =
            Line::new(3, "key-yours", put(&say!("Your key"))).held(words(theme, state.0, state.1));
        if key.stored {
            yours = yours.held(
                Btn::new("key-forget", put(&say!("Forget the key")))
                    .enabled(!key.checking)
                    .on_click(cx.listener(|s, _, _, cx| s.forget_key(cx))),
            );
        }
        rows.push(Row::Line(yours));
        if key.available {
            let typed = !self.fields.key.read(cx).text().is_empty();
            let field = Field {
                id: "key-field",
                text: &self.fields.key,
                icon: "",
                placeholder: put(&say!("Steam Web API key")),
                found: None,
                keys: None,
            };
            rows.push(Row::Line(
                Line::new(3, "key-type", put(&say!("Type a key")))
                    .held(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(field.render(theme, window, cx)),
                    )
                    .held(
                        Btn::new("key-check", put(&say!("Check and keep")))
                            .primary()
                            .enabled(typed && !key.checking)
                            .on_click(cx.listener(|s, _, _, cx| s.check_key(cx))),
                    ),
            ));
        }
        if key.checking {
            rows.push(Row::Under(
                3,
                under(theme, c.ink3, theme.small(), put(&say!("Asking Steam…"))),
            ));
        }
        if let Some(note) = &key.note {
            rows.push(Row::Under(3, under(theme, c.err, theme.body(), put(note))));
        }
        let mature = self.report.show_mature;
        rows.push(Row::Line(
            Line::new(3, "mature", put(&say!("Show mods Steam marks as mature")))
                .tip(put(&say!(
                    "Works only with a key; the database does not mark mature mods."
                )))
                .changed(mature, |s, _, cx| s.keep("showMature", "off", cx))
                .held(
                    Toggle::new(
                        "mature-toggle",
                        mature,
                        put(&say!("Show mods Steam marks as mature")),
                    )
                    .on_flip(act(cx, |s, on: bool, _, cx| {
                        s.keep("showMature", if on { "on" } else { "off" }, cx)
                    })),
                )
                .filled(),
        ));
    }

    pub(super) fn rimstack_part(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
        rows: &mut Vec<Row>,
    ) {
        let c = &theme.colours;
        let report = self.report.clone();
        rows.push(Row::Group(4, put(&say!("When the game misbehaves"))));
        rows.push(Row::Line(
            Line::new(4, "verify", put(&say!("Game files")))
                .tip(put(&say!("Steam compares every file of the game with its own copy and downloads what differs.")))
                .held(Btn::new("verify-steam", put(&say!("Verify with Steam"))).on_click(|_, _, cx| cx.open_url(VERIFY)))
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(4, "mod-settings", put(&say!("Mod settings")))
                .tip(put(&say!("Moves every Mod_*.xml from the game's Config folder into a dated folder in RimStack's data folder. Mods start from their default settings.")))
                .held(Btn::new("mod-settings-aside", put(&say!("Set aside…"))).danger().on_click(cx.listener(|s, _, window, cx| s.ask_mod_settings_aside(window, cx))))
                .held(div().flex_1().min_w_0().child(label(theme, theme.body(), c.ink2, self.said.mod_settings.clone()))),
        ));

        rows.push(Row::Group(4, put(&say!("Load order"))));
        let backup = report.backup.is_some();
        rows.push(Row::Line(
            Line::new(4, "original-list", put(&say!("List from before RimStack")))
                .tip(put(&say!("The load order the game had before RimStack first saved, kept in ModsConfig.xml.rimstack-original. Using it changes the list here; Save writes it.")))
                .held(Btn::new("use-original", put(&say!("Use"))).enabled(backup).on_click(cx.listener(|_, _, _, cx| {
                    if let Some(ids) = crate::ui::app(cx).original_list() {
                        crate::ui::navigate::go(crate::ui::navigate::Go::UseList(ids), cx);
                    }
                })))
                .when_held(!backup, label(theme, theme.body(), c.ink3, put(&say!("None yet"))))
                .filled(),
        ));

        rows.push(Row::Group(4, put(&say!("Files"))));
        let kept = LOGS_KEPT
            .iter()
            .position(|n| *n == report.logs_kept)
            .unwrap_or(0);
        let mut options = said(&[say!("None")]);
        options.extend(LOGS_KEPT[1..].iter().map(|n| n.to_string().into()));
        rows.push(Row::Line(
            Line::new(4, "logs-kept", put(&say!("Logs kept")))
                .tip(put(&say!("The game keeps the logs of its last two launches. RimStack copies each log after the game stops, so older launches can still be read in Logs; the oldest copies are deleted first.")))
                .changed(report.logs_kept != 10, |s, _, cx| s.keep("logsKept", "10", cx))
                .held(Choice::new("logs-kept-choice", options, kept).within(Self::choice_room(theme, window)).title(put(&say!("Logs kept"))).on_pick(act(cx, |s, at: usize, _, cx| s.keep("logsKept", &LOGS_KEPT[at].to_string(), cx))))
                .held(div().flex_1().min_w_0().child(label(theme, theme.small(), c.ink2, put(&say_n!(report.logs_kept_now, "{} kept now", "{} kept now"))))),
        ));
        let room = self.held_room(theme, window);
        rows.push(Row::Line(
            Line::new(4, "data-folder", put(&say!("Downloads and records"))).held(path(
                "data-path",
                report.data_folder.clone(),
                c.ink,
                room,
                Some(("folder-open", put(&say!("Open folder")))),
                theme,
                window,
            )),
        ));
        rows.push(Row::Line(
            Line::new(4, "settings-file", put(&say!("Settings"))).held(path(
                "settings-path",
                report.settings_file.clone(),
                c.ink,
                room,
                None,
                theme,
                window,
            )),
        ));
        rows.push(Row::Line(
            Line::new(4, "diagnostics", put(&say!("Diagnostics"))).held(path(
                "diagnostics-path",
                report.diagnostics_file.clone(),
                c.ink,
                room,
                Some(("scroll-text", put(&say!("Open file")))),
                theme,
                window,
            )),
        ));

        rows.push(Row::Group(4, put(&say!("About"))));
        let (newer, problem) = match &self.release {
            Some(ReleaseSaid::Newer { version, page }) => {
                (Some((version.clone(), page.clone())), None)
            }
            Some(ReleaseSaid::Failed { problem }) => (None, Some(put(problem))),
            Some(ReleaseSaid::Latest {}) => {
                (None, Some(put(&say!("No newer version is published"))))
            }
            None => (None, None),
        };
        let busy = self.release_busy;
        let failed = matches!(self.release, Some(ReleaseSaid::Failed { .. }));
        rows.push(Row::Line(
            Line::new(4, "version", put(&say!("Version")))
                .held(label(theme, theme.body(), c.ink, report.version.clone()))
                .held(
                    Btn::new(
                        "look-now",
                        if busy {
                            put(&say!("Asking GitHub…"))
                        } else {
                            put(&say!("Look now"))
                        },
                    )
                    .enabled(!busy)
                    .on_click(cx.listener(|s, _, _, cx| s.check_release(cx))),
                )
                .when_held_some(newer, |(version, page)| {
                    Btn::new("get-newer", put(&say!("Get {}", version)))
                        .primary()
                        .icon("external-link")
                        .on_click(move |_, _, cx| cx.open_url(&page))
                        .into_any_element()
                })
                .held(div().flex_1().min_w_0().child(label(
                    theme,
                    theme.small(),
                    if failed { c.err } else { c.ink2 },
                    problem.unwrap_or_default(),
                ))),
        ));
        let self_check = report.self_check;
        rows.push(Row::Line(
            Line::new(4, "self-check", put(&say!("Look for a new version at each start")))
                .tip(put(&say!("On: at each start RimStack asks GitHub whether a newer version is published. It installs nothing by itself.")))
                .changed(!self_check, |s, _, cx| s.keep("selfCheck", "on", cx))
                .held(Toggle::new("self-check-toggle", self_check, put(&say!("Look for a new version at each start"))).on_flip(act(cx, |s, on: bool, _, cx| s.keep("selfCheck", if on { "on" } else { "off" }, cx))))
                .filled(),
        ));
        let nightly = report.nightly_updates;
        rows.push(Row::Line(
            Line::new(4, "versions", put(&say!("Versions to look for")))
                .tip(put(&say!("Stable: a numbered release. Nightly: the newest build of the main branch, which is made often and may break. Nightly compares against the commit this program was built from.")))
                .changed(nightly, |s, _, cx| s.keep("updates", "stable", cx))
                .held(Choice::new("versions-choice", said(&[say!("Stable"), say!("Nightly")]), usize::from(nightly)).within(Self::choice_room(theme, window)).title(put(&say!("Versions to look for"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.keep("updates", ["stable", "nightly"][at], cx);
                    s.check_release(cx);
                })))
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(4, "issues", put(&say!("Source and issues")))
                .held(
                    Btn::new("report-problem", put(&say!("Report a problem")))
                        .icon("external-link")
                        .tip(ISSUES)
                        .on_click(|_, _, cx| cx.open_url(ISSUES)),
                )
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(4, "support", put(&say!("Support RimStack")))
                .held(
                    Btn::new("support-links", put(&say!("Discord and donations")))
                        .icon("heart")
                        .icon_tone(c.err)
                        .opens(|below, window, cx| {
                            crate::ui::menu::open(below, crate::ui::support::menu(), window, cx)
                        }),
                )
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(4, "licence", put(&say!("Licence"))).held(words(
                theme,
                c.ink,
                put(&say!("GNU General Public License, version 3 or later")),
            )),
        ));
        rows.push(Row::Line(
            Line::new(4, "icons", put(&say!("Icons"))).held(words(
                theme,
                c.ink,
                put(&say!("Lucide, ISC licence")),
            )),
        ));
    }

    fn version_warnings(&mut self, choice: i32, cx: &mut Context<Self>) {
        self.problem = crate::ui::app(cx)
            .version_warnings(choice)
            .err()
            .map(|e| e.sentence().clone());
        cx.notify();
    }

    fn ask_mod_settings_aside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let report = &self.report;
        let lines = vec![
            Asked::Text(put(&say!("Every Mod_*.xml is moved out of"))),
            Asked::Path(report.config_folder.clone().unwrap_or_default()),
            Asked::Text(put(&say!(
                "into a new folder, named mod-settings and the time, in"
            ))),
            Asked::Path(report.data_folder.clone()),
            Asked::Text(put(&say!(
                "Mods start from their defaults. Moving the files back restores them."
            ))),
        ];
        let me = cx.entity().downgrade();
        let run = Box::new(move |_: &mut Window, cx: &mut gpui::App| {
            let _ = me.update(cx, |s, cx| {
                s.said.mod_settings = put(&crate::ui::app(cx).set_mod_settings_aside());
                cx.notify();
            });
        });
        let question = Question {
            title: put(&say!("Set the mods' settings aside")),
            lines,
            label: put(&say!("Set aside")),
            danger: false,
            run,
        };
        prompt::ask(question, window, cx);
    }
}

impl Line {
    fn when_held(self, shown: bool, held: impl IntoElement) -> Self {
        match shown {
            true => self.held(held),
            false => self,
        }
    }

    fn when_held_some<T>(self, value: Option<T>, held: impl FnOnce(T) -> gpui::AnyElement) -> Self {
        match value {
            Some(value) => self.held(held(value)),
            None => self,
        }
    }
}

trait Silent {
    fn when_silent(self, silent: bool) -> Self;
}

impl Silent for Toggle {
    fn when_silent(self, silent: bool) -> Self {
        if silent { self.silent() } else { self }
    }
}
