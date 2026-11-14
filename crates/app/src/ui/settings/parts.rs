use super::Settings;
use super::line::{Line, Row, path, words};
use crate::ui::button::{Btn, Chip};
use crate::ui::controls::{Choice, Slide, Toggle};
use crate::ui::menu::{self, Entry};
use crate::ui::theme::{Theme, ThemeName};
use crate::ui::widgets::label;
use crate::words::{count, put};
use gpui::{App, Context, ParentElement, SharedString, Styled, Window, div};
use rimstack_core::{say, say_n};

pub(super) fn act<T: 'static>(
    cx: &Context<Settings>,
    f: impl Fn(&mut Settings, T, &mut Window, &mut Context<Settings>) + 'static,
) -> impl Fn(T, &mut Window, &mut App) + 'static {
    let me = cx.entity().downgrade();
    move |value, window, cx| {
        let _ = me.update(cx, |settings, cx| f(settings, value, window, cx));
    }
}

pub(super) fn said(options: &[rimstack_core::base::sentence::Sentence]) -> Vec<SharedString> {
    options.iter().map(|o| put(o).into()).collect()
}

impl Settings {
    pub(super) fn game_part(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
        rows: &mut Vec<Row>,
    ) {
        let c = &theme.colours;
        let report = self.report.clone();
        rows.push(Row::Group(0, put(&say!("Folders"))));
        rows.push(Row::Note(
            0,
            put(&say!(
                "Found through Steam. Type a folder to use another; empty the box to find it again."
            )),
        ));
        let version = report
            .game_version
            .as_ref()
            .map(|v| put(&say!("version {}", v.clone())))
            .unwrap_or_default();
        rows.push(Row::Line(self.folder(
            0,
            put(&say!("RimWorld")),
            version,
            theme,
            window,
            cx,
        )));
        rows.push(Row::Line(self.folder(
            1,
            put(&say!("Config and saves")),
            String::new(),
            theme,
            window,
            cx,
        )));
        rows.push(Row::Line(self.folder(
            2,
            put(&say!("Workshop mods")),
            String::new(),
            theme,
            window,
            cx,
        )));

        rows.push(Row::Group(0, put(&say!("Play"))));
        let options = said(&[say!("Through Steam"), say!("Directly")]);
        if report.sandboxed {
            rows.push(Row::Line(
                Line::new(0, "launch", put(&say!("Play starts the game")))
                    .tip(put(&say!("A Flatpak runs the game in a sandbox of its own, so Play asks Steam to start it outside.")))
                    .held(label(theme, theme.body(), c.ink, options[0].clone())),
            ));
        } else {
            rows.push(Row::Line(
            Line::new(0, "launch", put(&say!("Play starts the game")))
                .tip(put(&say!("Directly runs the game's own program from its folder, for a copy not from Steam.")))
                .changed(report.launch_directly, |s, _, cx| s.keep("launch", "steam", cx))
                .held(Choice::new("launch-choice", options, usize::from(report.launch_directly)).within(Self::choice_room(theme, window)).title(put(&say!("Play starts the game"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.keep("launch", if at == 1 { "direct" } else { "steam" }, cx)
                })))
                .filled(),
            ));
        }
        let launch = crate::ui::field::Field {
            id: "launch-arguments",
            text: &self.fields.launch,
            icon: "",
            placeholder: put(&say!("None")),
            found: None,
            keys: None,
        };
        rows.push(Row::Line(
            Line::new(0, "launch-arguments", put(&say!("Launch arguments")))
                .tip(put(&say!("Given to the game when Play starts it, as in Steam's own launch options: -logfile, -popupwindow, -savedatafolder=…")))
                .changed(report.launch_arguments.as_deref().is_some_and(|a| !a.is_empty()), |s, _, cx| {
                    s.fields.launch.update(cx, |field, cx| field.set_text("", cx));
                    s.keep("launchArguments", "", cx);
                })
                .held(div().flex_1().min_w_0().child(launch.render(theme, window, cx))),
        ));

        rows.push(Row::Group(0, put(&say!("Load order"))));
        let file = report.mods_config.clone();
        let room = self.held_room(theme, window);
        rows.push(Row::Line(
            Line::new(0, "mods-config", put(&say!("File")))
                .tip(put(&say!(
                    "The game's list of active mods. Save writes it."
                )))
                .held(path(
                    "mods-config-path",
                    file.clone().unwrap_or_else(|| put(&say!("Not found"))),
                    if file.is_some() { c.ink } else { c.err },
                    room,
                    None,
                    theme,
                    window,
                )),
        ));
        let copies = match report.backup.is_some() {
            true => ".rimstack-backup, .rimstack-original".to_string(),
            false => put(&say!("None yet")),
        };
        rows.push(Row::Line(
            Line::new(0, "copies", put(&say!("Copies")))
                .tip(put(&say!("Before each Save, the old file is copied next to it as .rimstack-backup, and the first time also as .rimstack-original.")))
                .held(words(theme, if report.backup.is_some() { c.ink } else { c.ink3 }, copies)),
        ));

        rows.push(Row::Group(0, put(&say!("Updates"))));
        let auto = report.auto_updates;
        rows.push(Row::Line(
            Line::new(0, "auto-updates", put(&say!("Update by themselves")))
                .tip(put(&say!("Steam is asked about the installed Workshop mods and the mods SteamCMD put in the game's Mods folder (only their Workshop ids are sent) and Git about each checkout, as often as chosen. Steam then downloads the newer versions, SteamCMD downloads again the mods RimStack downloaded with it, and each checkout with no local changes is updated, except while the game runs.")))
                .changed(auto, |s, _, cx| s.keep("autoUpdates", "off", cx))
                .held(Toggle::new("auto-updates-toggle", auto, put(&say!("Update by themselves"))).on_flip(act(cx, |s, on: bool, _, cx| s.keep("autoUpdates", if on { "on" } else { "off" }, cx))))
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(0, "update-every", put(&say!("How often")))
                .tip(put(&say!(
                    "The time between two checks for updates made by themselves."
                )))
                .changed(report.update_every_set, |s, _, cx| {
                    s.keep("updateEvery", "", cx)
                })
                .held(
                    Choice::new(
                        "update-every-choice",
                        said(&report.update_every),
                        report.update_every_at,
                    )
                    .within(Self::choice_room(theme, window))
                    .title(put(&say!("How often")))
                    .on_pick(act(cx, |s, at: usize, _, cx| s.keep_update_every(at, cx))),
                )
                .filled(),
        ));
        let git = match &report.git {
            Some(version) => (c.ink, put(&say!("Installed, version {}", version.clone()))),
            None => (
                c.err,
                put(&say!(
                    "Not installed: Git checkouts cannot be checked or updated"
                )),
            ),
        };
        rows.push(Row::Line(
            Line::new(0, "git", put(&say!("Git")))
                .tip(put(&say!("A mod that is a Git checkout is asked for updates, and updated, with the git program on this computer.")))
                .held(words(theme, git.0, git.1)),
        ));
        let for_workshop = report.git_for_workshop;
        rows.push(Row::Line(
            Line::new(0, "git-for-workshop", put(&say!("Git for Workshop mods")))
                .tip(put(&say!("Check now asks Steam about Workshop mods, and Git about checkouts in the game's Mods folder. On: a Workshop mod that is also a Git checkout is checked with Git too, which fetches into its folder.")))
                .changed(for_workshop, |s, _, cx| s.keep("gitForWorkshop", "off", cx))
                .held(Toggle::new("git-for-workshop-toggle", for_workshop, put(&say!("Git for Workshop mods"))).on_flip(act(cx, |s, on: bool, _, cx| s.keep("gitForWorkshop", if on { "on" } else { "off" }, cx))))
                .filled(),
        ));
    }

    pub(super) fn look_part(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
        rows: &mut Vec<Row>,
    ) {
        let c = &theme.colours;
        let look = theme.appearance.clone();
        let prefs = &self.prefs;
        let report = self.report.clone();
        rows.push(Row::Group(1, put(&say!("Window"))));
        let themes = [
            ThemeName::Rim,
            ThemeName::Dark,
            ThemeName::Oled,
            ThemeName::Light,
        ];
        let labels = said(&[say!("Rim"), say!("Dark"), say!("OLED"), say!("Light")]);
        let current = themes.iter().position(|t| *t == look.theme).unwrap_or(0);
        rows.push(Row::Line(
            Line::new(1, "theme", put(&say!("Theme")))
                .changed(look.theme != ThemeName::Rim, |s, _, cx| {
                    s.keep_look("theme", "rim", cx)
                })
                .held(
                    Choice::new("theme-choice", labels.clone(), current)
                        .within(Self::choice_room(theme, window))
                        .title(put(&say!("Theme")))
                        .on_pick(act(cx, move |s, at: usize, _, cx| {
                            s.keep_look("theme", themes[at].name(), cx)
                        })),
                )
                .filled(),
        ));

        let in_use = match report.language.is_some() {
            true => report.language_in_use.clone(),
            false => put(&say!("{} (system)", report.language_in_use.clone())),
        };
        let languages = self.languages.clone();
        let translated = crate::words::translated();
        let language = Line::new(1, "language", put(&say!("Language")))
            .tip(put(&say!("A language file with the same name in the settings folder replaces this language's texts one by one. A text left empty there keeps the original.")))
            .held(Btn::new("language-menu", in_use).opens(act(cx, move |_, at: gpui::Bounds<gpui::Pixels>, window, cx| {
                let me = cx.entity().downgrade();
                let pick = |name: String| {
                    let me = me.clone();
                    move |_: &mut Window, cx: &mut App| {
                        let _ = me.update(cx, |s, cx| s.use_language(&name, cx));
                    }
                };
                let mut entries: Vec<Entry> = vec![Entry::item(put(&say!("System language")), pick(String::new())).into()];
                entries.extend(languages.iter().map(|name| Entry::item(name.clone(), pick(name.clone())).into()));
                menu::open(at, entries, window, cx);
            })));
        let language = match report.english {
            true => language,
            false => language.held(
                Chip::new(match translated > 0 {
                    true => put(&say!("{} texts translated", count(translated))),
                    false => put(&say!("Not translated yet")),
                })
                .tone(if translated > 0 { c.ink2 } else { c.warn }),
            ),
        };
        rows.push(Row::Line(
            language
                .held(
                    Btn::new("edit-translations", put(&say!("Edit translations")))
                        .icon("external-link")
                        .tip(put(&say!("Opens your corrections to this language: a wording saved there replaces RimStack's at once")))
                        .on_click(cx.listener(|s, _, _, cx| s.start_language(cx))),
                )
                .held(div().flex_1().min_w_0().child(label(theme, theme.body(), c.err, self.said.language.clone()))),
        ));

        let own_on = prefs.flag("Appearance", "ownColours").unwrap_or(false);
        let theme_label = labels[current].clone();
        let own = put(&match look.own.len() {
            0 => say!("None for {}", theme_label.to_string()),
            n => say!("{} for {}", n, theme_label.to_string()),
        });
        let short = self.short_contrast(&look);
        let mut own_line = Line::new(1, "own-colours", put(&say!("Colours")))
            .tip(put(&say!("Your own colours for the chosen theme, read from theme.json in RimStack's settings folder. Edit theme.json opens it; a colour saved there shows at once.")))
            .changed(own_on, |s, _, cx| s.keep_look("ownColours", "false", cx))
            .held(Toggle::new("own-colours-toggle", own_on, put(&say!("Colours"))).on_flip(act(cx, |s, on: bool, _, cx| s.keep_look("ownColours", if on { "true" } else { "false" }, cx))))
            .held(Chip::new(own));
        if own_on && !look.own.is_empty() && !short.is_empty() {
            let tip = short
                .iter()
                .map(|pair| {
                    put(&say!(
                        "{} {}, needs {}",
                        put(&rimstack_core::base::sentence::Sentence::named(pair.what)),
                        format!("{:.1}", pair.found),
                        pair.needed.to_string()
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n");
            own_line = own_line.held(
                Chip::new(put(&say_n!(
                    short.len(),
                    "{} colour pair has too little contrast",
                    "{} colour pairs have too little contrast"
                )))
                .tone(c.warn)
                .icon("triangle-alert")
                .tip(tip),
            );
        }
        rows.push(Row::Line(
            own_line
                .held(
                    Btn::new("edit-theme", put(&say!("Edit theme.json")))
                        .icon("external-link")
                        .tip(put(&say!("Opens the file, first adding this theme's colours if it has none. A change saved there shows at once")))
                        .on_click(cx.listener(|s, _, _, cx| s.start_own_theme(cx))),
                )
                .held(div().flex_1().min_w_0().child(label(theme, theme.body(), c.err, self.said.own_theme.clone()))),
        ));

        let font = report.font.clone().filter(|f| !f.is_empty());
        rows.push(Row::Line(
            Line::new(1, "font", put(&say!("Font")))
                .changed(font.is_some(), |s, _, cx| s.use_font("", cx))
                .held(self.font_picker(
                    font.unwrap_or_else(|| put(&say!("System font"))),
                    theme,
                    window,
                    cx,
                ))
                .filled(),
        ));

        let reading = format!("{}%", (look.text_scale * 100.).round());
        rows.push(Row::Line(
            Line::new(1, "text-size", put(&say!("Text size")))
                .tip(put(&say!(
                    "Resizes every text at once; rows, buttons and bars grow with it."
                )))
                .changed(look.text_scale != 1.0, |s, _, cx| {
                    s.keep_look("textScale", "1", cx)
                })
                .held(
                    Slide::new("text-size-slide", (0.85, 1.6, 0.05), look.text_scale)
                        .said(put(&say!("Text size")), reading.clone())
                        .on_move(act(cx, |s, value: f32, _, cx| {
                            s.keep_look(
                                "textScale",
                                &format!("{}", (value * 100.).round() / 100.),
                                cx,
                            )
                        })),
                )
                .held(label(theme, theme.body(), c.ink2, reading))
                .filled(),
        ));
        let rows_at = if look.comfortable {
            2
        } else {
            usize::from(look.large_rows)
        };
        rows.push(Row::Line(
            Line::new(1, "mod-rows", put(&say!("Mod rows")))
                .tip(put(&say!("Roomy: more space around each row. With pictures: taller rows in the load order, each with the mod's picture.")))
                .changed(!look.comfortable || look.large_rows, |s, _, cx| {
                    s.keep_look("comfortable", "true", cx);
                    s.keep_look("largeRows", "false", cx);
                })
                .held(Choice::new("mod-rows-choice", said(&[say!("Compact"), say!("Roomy"), say!("With pictures")]), rows_at).within(Self::choice_room(theme, window)).title(put(&say!("Mod rows"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.keep_look("comfortable", if at == 2 { "true" } else { "false" }, cx);
                    s.keep_look("largeRows", if at == 1 { "true" } else { "false" }, cx);
                })))
                .filled(),
        ));
        let dim = report.dim_left_out;
        rows.push(Row::Line(
            Line::new(1, "left-out", put(&say!("A list the filter leaves out")))
                .tip(put(&say!("A list where the filter matches no mod. Dimmed: it shows all its mods faded, and mods can still be dropped on it.")))
                .changed(!dim, |s, _, cx| s.keep("dimLeftOut", "on", cx))
                .held(Choice::new("left-out-choice", said(&[say!("Dimmed"), say!("Hidden")]), usize::from(!dim)).within(Self::choice_room(theme, window)).title(put(&say!("A list the filter leaves out"))).on_pick(act(cx, |s, at: usize, _, cx| {
                    s.keep("dimLeftOut", ["on", "off"][at], cx)
                })))
                .filled(),
        ));

        rows.push(Row::Group(1, put(&say!("Ease of use"))));
        rows.push(Row::Line(
            Line::new(1, "motion", put(&say!("Motion")))
                .tip(put(&say!(
                    "Reduced: nothing slides, fades or spins; changes show at once."
                )))
                .changed(look.reduce_motion, |s, _, cx| {
                    s.keep_look("reduceMotion", "false", cx)
                })
                .held(
                    Choice::new(
                        "motion-choice",
                        said(&[say!("Full"), say!("Reduced")]),
                        usize::from(look.reduce_motion),
                    )
                    .within(Self::choice_room(theme, window))
                    .title(put(&say!("Motion")))
                    .on_pick(act(cx, |s, at: usize, _, cx| {
                        s.keep_look("reduceMotion", if at == 1 { "true" } else { "false" }, cx)
                    })),
                )
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(1, "contrast", put(&say!("Contrast")))
                .tip(put(&say!(
                    "High: grey text and thin lines are drawn stronger, and panels get an outline."
                )))
                .changed(look.high_contrast, |s, _, cx| {
                    s.keep_look("highContrast", "false", cx)
                })
                .held(
                    Choice::new(
                        "contrast-choice",
                        said(&[say!("Normal"), say!("High")]),
                        usize::from(look.high_contrast),
                    )
                    .within(Self::choice_room(theme, window))
                    .title(put(&say!("Contrast")))
                    .on_pick(act(cx, |s, at: usize, _, cx| {
                        s.keep_look("highContrast", if at == 1 { "true" } else { "false" }, cx)
                    })),
                )
                .filled(),
        ));
        rows.push(Row::Line(
            Line::new(1, "hints", put(&say!("First-run hints")))
                .tip(put(&say!(
                    "Three notes on the Mods screen, about problems, Sort and Save."
                )))
                .held(
                    Btn::new("hints-again", put(&say!("Show again")))
                        .on_click(cx.listener(|s, _, _, cx| s.say_hints(cx))),
                )
                .filled(),
        ));

        rows.push(Row::Group(1, put(&say!("Links"))));
        let in_steam = prefs.flag("Appearance", "workshopInSteam").unwrap_or(false);
        rows.push(Row::Line(
            Line::new(1, "workshop-in", put(&say!("Workshop pages open in")))
                .changed(in_steam, |s, _, cx| {
                    s.keep_look("workshopInSteam", "false", cx)
                })
                .held(
                    Choice::new(
                        "workshop-in-choice",
                        said(&[say!("Browser"), say!("Steam")]),
                        usize::from(in_steam),
                    )
                    .within(Self::choice_room(theme, window))
                    .title(put(&say!("Workshop pages open in")))
                    .on_pick(act(cx, |s, at: usize, _, cx| {
                        s.keep_look(
                            "workshopInSteam",
                            if at == 1 { "true" } else { "false" },
                            cx,
                        )
                    })),
                )
                .filled(),
        ));
    }

    fn short_contrast(
        &self,
        look: &crate::ui::theme::Appearance,
    ) -> Vec<crate::reports::ShortContrast> {
        let names = [
            "win",
            "panel",
            "bar",
            "ink",
            "ink2",
            "ink3",
            "accent",
            "accentInk",
            "focus",
            "fieldLine",
            "warn",
            "err",
            "ok",
        ];
        let shown: serde_json::Map<String, serde_json::Value> = names
            .iter()
            .filter_map(|name| Some((name.to_string(), look.colour_text(name)?.into())))
            .collect();
        crate::workspace::short_contrast(&shown)
    }
}
