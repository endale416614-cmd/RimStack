use crate::ui::button::Btn;
use crate::ui::controls::Choice;
use crate::ui::theme::{Theme, ThemeName};

use crate::words::{count, put};
use gpui::{AnyElement, Context, IntoElement, Pixels, SharedString, Window};

use rimstack_core::say;

use super::FirstRun;

impl FirstRun {
    pub(super) fn content(
        &self,
        theme: &Theme,
        window: &Window,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let report = self.report.clone();
        let found = |what: String, place: Option<String>, sentence: bool, note: bool| {
            Self::found(theme, window, what, place, sentence, note, width)
        };
        let mut blocks = Vec::new();
        match self.step {
            0 => {
                let languages = self.languages.iter().map(|name| {
                    let chosen = report.language_in_use == *name;
                    let held = name.clone();
                    Btn::new(SharedString::from(format!("language-{name}")), name.clone())
                        .primary_if(chosen)
                        .on_click(cx.listener(move |first, _, _, cx| {
                            first.keep("language", &held, cx);
                            let app = crate::ui::app(cx);
                            crate::words::show_in(crate::words::Words::of(&app.speak()));
                            cx.refresh_windows();
                        }))
                        .into_any_element()
                });
                blocks.push(Self::block(
                    theme,
                    put(&say!("Interface")),
                    vec![Self::pick(
                        theme,
                        window,
                        put(&say!("Language")),
                        languages.collect(),
                        width,
                    )],
                ));
                let game = match &report.game_version {
                    Some(version) => format!("RimWorld {version}"),
                    None => "RimWorld".to_string(),
                };
                blocks.push(Self::block(
                    theme,
                    put(&say!("Game")),
                    vec![
                        found(game, report.game.clone(), false, false),
                        found(
                            put(&say!("Workshop mods")),
                            report.workshop.clone(),
                            false,
                            false,
                        ),
                        found(
                            put(&say!("Config and saves")),
                            report.config.clone(),
                            false,
                            false,
                        ),
                    ],
                ));
                let (active, inactive, problem) = &self.mods;
                let line = match problem.is_empty() {
                    true => found(
                        put(&say!(
                            "{} active mods, {} inactive",
                            count(*active),
                            count(*inactive)
                        )),
                        Some(put(&say!("kept as they are"))),
                        true,
                        false,
                    ),
                    false => found(
                        put(&say!("Mods could not be read: {}", problem.clone())),
                        None,
                        true,
                        false,
                    ),
                };
                blocks.push(Self::block(
                    theme,
                    put(&say!("Current mod list")),
                    vec![line],
                ));
                let mut programs = Vec::new();
                match &report.git {
                    Some(version) => programs.push(found("Git".into(), Some(put(&say!("version {}", version.clone()))), true, false)),
                    None => programs.push(found(put(&say!("Git is not installed: a mod kept as a Git checkout cannot be checked or updated")), None, true, true)),
                }
                match (&report.steamcmd_refused, report.steamcmd) {
                    (_, true) => programs.push(found("SteamCMD".into(), Some(put(&say!("installed"))), true, false)),
                    (Some(refused), false) => programs.push(found(put(refused), None, true, true)),
                    (None, false) => programs.push(found(put(&say!("SteamCMD is not installed: Downloads can install it, to fetch a mod without subscribing")), None, true, true)),
                }
                blocks.push(Self::block(theme, put(&say!("Programs")), programs));
            }
            1 => {
                if report.rimsort_rules {
                    let on = self.with_rimsort;
                    let me = cx.entity().downgrade();
                    blocks.push(Self::block(
                        theme,
                        put(&say!("Import from RimSort")),
                        vec![Self::tick(
                            theme,
                            "rimsort".into(),
                            put(&say!("Your load order rules")),
                            put(&say!("(RimSort's file is only read)")),
                            on,
                            move |_, cx| {
                                let _ = me.update(cx, |first, cx| {
                                    first.with_rimsort = !first.with_rimsort;
                                    cx.notify();
                                });
                            },
                        )],
                    ));
                }
                let ticks = self.databases.iter().map(|database| {
                    let mut detail = put(&database.used_for);
                    if database.updated > 0 {
                        detail = format!(
                            "{detail}, {}",
                            put(&say!(
                                "downloaded {}",
                                crate::workspace::logs::day(database.updated)
                            ))
                        );
                    }
                    let (id, on) = (database.id, database.on);
                    let me = cx.entity().downgrade();
                    Self::tick(
                        theme,
                        SharedString::from(format!("database-{id}")),
                        database.name.to_string(),
                        detail,
                        on,
                        move |_, cx| {
                            let _ = me.update(cx, |first, cx| {
                                if let Err(problem) = crate::ui::app(cx).use_database(id, !on) {
                                    crate::ui::notices::say(
                                        crate::words::said_of(&problem),
                                        crate::ui::notices::Tone::Err,
                                        cx,
                                    );
                                }
                                first.read(cx);
                            });
                        },
                    )
                });
                blocks.push(Self::block(
                    theme,
                    put(&say!("Community data")),
                    ticks.collect(),
                ));
            }
            2 => {
                let me = cx.entity().downgrade();
                let flip = |name: &'static str, on: bool| {
                    let me = me.clone();
                    move |_: &mut Window, cx: &mut gpui::App| {
                        let _ = me.update(cx, |first, cx| {
                            first.keep(name, if on { "off" } else { "on" }, cx)
                        });
                    }
                };
                blocks.push(Self::block(
                    theme,
                    put(&say!("Sort follows")),
                    vec![
                        found(put(&say!("Each mod's own rules, and the code it uses or patches in another mod: always")), None, true, true),
                        Self::tick(theme, "community-rules".into(), put(&say!("Community rules")), put(&say!("load-order rules written by players, for mods whose files do not say enough")), report.community_rules, flip("communityRules", report.community_rules)),
                        Self::tick(theme, "file-hints".into(), put(&say!("Patch and def hints")), put(&say!("a patch is placed after what it patches, read from the files")), report.file_hints, flip("fileHints", report.file_hints)),
                    ],
                ));
                blocks.push(Self::block(
                    theme,
                    put(&say!("Either way")),
                    vec![
                        found(put(&say!("Sort shows every move and the rule behind it before anything changes")), None, true, true),
                        found(put(&say!("Hints that Sort does not use still show in a mod's Load order tab")), None, true, true),
                    ],
                ));
            }
            3 => {
                blocks.extend(self.glance(theme, &report));
                let look = cx.global::<Theme>().appearance.clone();
                let names: Vec<SharedString> =
                    [say!("Rim"), say!("Dark"), say!("OLED"), say!("Light")]
                        .iter()
                        .map(|name| put(name).into())
                        .collect();
                let themes = [
                    ThemeName::Rim,
                    ThemeName::Dark,
                    ThemeName::Oled,
                    ThemeName::Light,
                ];
                let current = themes.iter().position(|t| *t == look.theme).unwrap_or(0);
                let sizes = [1.0_f32, 1.1, 1.25, 1.5];
                let size_at = sizes
                    .iter()
                    .position(|s| (s - look.text_scale).abs() < 0.03)
                    .unwrap_or(0);
                let rows = if look.comfortable {
                    2
                } else if look.large_rows {
                    1
                } else {
                    0
                };
                let in_steam = crate::ui::app(cx)
                    .prefs()
                    .flag("Appearance", "workshopInSteam")
                    .unwrap_or(false);
                let choice = |id: &'static str,
                              title: String,
                              options: Vec<SharedString>,
                              at: usize,
                              picked: fn(usize, &mut gpui::App)| {
                    Choice::new(id, options, at)
                        .title(title)
                        .on_pick(move |at, _, cx| picked(at, cx))
                        .into_any_element()
                };
                blocks.push(Self::block(
                    theme,
                    put(&say!("Appearance")),
                    vec![
                        Self::pick(
                            theme,
                            window,
                            put(&say!("Theme")),
                            vec![choice(
                                "first-theme",
                                put(&say!("Theme")),
                                names,
                                current,
                                |at, cx| {
                                    let theme = [
                                        ThemeName::Rim,
                                        ThemeName::Dark,
                                        ThemeName::Oled,
                                        ThemeName::Light,
                                    ][at];
                                    crate::ui::keep_appearance("theme", theme.name(), cx)
                                },
                            )],
                            width,
                        ),
                        Self::pick(
                            theme,
                            window,
                            put(&say!("Text size")),
                            vec![choice(
                                "first-size",
                                put(&say!("Text size")),
                                ["100%", "110%", "125%", "150%"]
                                    .map(SharedString::from)
                                    .to_vec(),
                                size_at,
                                |at, cx| {
                                    crate::ui::keep_appearance(
                                        "textScale",
                                        ["1", "1.1", "1.25", "1.5"][at],
                                        cx,
                                    )
                                },
                            )],
                            width,
                        ),
                        Self::pick(
                            theme,
                            window,
                            put(&say!("Mod rows")),
                            vec![choice(
                                "first-rows",
                                put(&say!("Mod rows")),
                                ["Compact", "Larger", "Comfortable"]
                                    .map(SharedString::from)
                                    .to_vec(),
                                rows,
                                |at, cx| {
                                    crate::ui::keep_appearance(
                                        "comfortable",
                                        if at == 2 { "true" } else { "false" },
                                        cx,
                                    );
                                    crate::ui::keep_appearance(
                                        "largeRows",
                                        if at == 1 { "true" } else { "false" },
                                        cx,
                                    );
                                },
                            )],
                            width,
                        ),
                        Self::pick(
                            theme,
                            window,
                            put(&say!("Workshop pages open in")),
                            vec![choice(
                                "first-pages",
                                put(&say!("Workshop pages open in")),
                                ["Browser", "Steam"].map(SharedString::from).to_vec(),
                                usize::from(in_steam),
                                |at, cx| {
                                    crate::ui::keep_appearance(
                                        "workshopInSteam",
                                        if at == 1 { "true" } else { "false" },
                                        cx,
                                    )
                                },
                            )],
                            width,
                        ),
                    ],
                ));
            }
            _ => {
                blocks.push(Self::block(
                    theme,
                    put(&say!("After Start")),
                    vec![
                        found(
                            put(&say!(
                                "Problems are counted above the load order, each with its fix"
                            )),
                            None,
                            true,
                            true,
                        ),
                        found(
                            put(&say!(
                                "Sort shows every move, and why, before anything changes"
                            )),
                            None,
                            true,
                            true,
                        ),
                        found(
                            put(&say!(
                                "Save writes ModsConfig.xml and copies the file as it was first"
                            )),
                            None,
                            true,
                            true,
                        ),
                    ],
                ));
            }
        }
        blocks
    }
}
