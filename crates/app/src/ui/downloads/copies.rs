//! The mods SteamCMD put in the game's Mods folder: updated through SteamCMD, and those whose
//! version is not known yet, or that Steam no longer says anything of.

use super::Downloads;
use super::screen::text_fill;
use crate::ui::card::Card;
use crate::ui::prompt::{self, Line, Question};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, btn_width, icon};
use crate::words::{count, put};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, Window, div};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::sources::downloads::Named;
use rimstack_core::{say, say_n};

const FIRST_ROWS: usize = 5;

impl Downloads {
    pub(super) fn update_copies(&mut self, wanted: Vec<String>, cx: &mut Context<Self>) {
        if wanted.is_empty() {
            return;
        }
        let app = crate::ui::app(cx);
        self.task(
            say_n!(
                wanted.len(),
                "Updating {} mod with SteamCMD…",
                "Updating {} mods with SteamCMD…"
            ),
            None,
            move || app.update_copies(&wanted),
            cx,
        );
    }

    fn ask_steamcmd(
        &mut self,
        title: Sentence,
        wanted: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let folder = self.report.local_mods.clone().unwrap_or_default();
        let lines = vec![
            Line::Text(put(&say!(
                "SteamCMD downloads each again, and the new copy replaces its folder in the game's Mods folder:"
            ))),
            Line::Path(folder),
        ];
        let run = self.answer(move |d, cx| d.update_copies(wanted, cx), cx);
        prompt::ask(
            Question {
                title: put(&title),
                lines,
                label: put(&say!("Download")),
                danger: false,
                run,
            },
            window,
            cx,
        );
    }

    pub fn ask_update_copies(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted: Vec<String> = self
            .updates
            .by_steamcmd()
            .map(|row| row.id.clone())
            .collect();
        let title = say_n!(
            wanted.len(),
            "Update {} mod with SteamCMD",
            "Update {} mods with SteamCMD"
        );
        self.ask_steamcmd(title, wanted, window, cx);
    }

    pub fn ask_track(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted: Vec<String> = self
            .updates
            .untracked
            .iter()
            .map(|row| row.id.clone())
            .collect();
        let title = say_n!(
            wanted.len(),
            "Download {} mod again with SteamCMD",
            "Download {} mods again with SteamCMD"
        );
        self.ask_steamcmd(title, wanted, window, cx);
    }

    fn named_rows(
        &self,
        theme: &Theme,
        key: &'static str,
        rows: &[Named],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let most = match self.is_folded(key, true) {
            true => FIRST_ROWS,
            false => rows.len(),
        };
        let mut shown: Vec<AnyElement> = rows
            .iter()
            .take(most)
            .enumerate()
            .map(|(at, row)| {
                let mod_id = row.mod_id;
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP_LARGE)
                    .child(icon("package", theme.icon(), c.ink3))
                    .child(text_fill(theme, theme.body(), c.ink, row.name.clone()))
                    .child(
                        Btn::new((key, at), put(&say!("Show")))
                            .spoken(put(&say!("Show {}", row.name.clone())))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                crate::ui::navigate::go(
                                    crate::ui::navigate::Go::ShowMod(mod_id),
                                    cx,
                                )
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        if rows.len() > FIRST_ROWS {
            let all = most == rows.len();
            shown.push(super::more(
                (key, usize::MAX),
                all,
                rows.len(),
                FIRST_ROWS,
                cx,
                move |d, cx| d.fold(key, cx),
            ));
        }
        shown
    }

    pub(super) fn untracked_card(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let untracked = &self.updates.untracked;
        if untracked.is_empty() {
            return None;
        }
        let again = put(&say!("Download again"));
        let action = Btn::new("track-copies", again.clone())
            .tip(put(&say!(
                "Their version is known from then on, so Check now finds their updates."
            )))
            .icon("download")
            .enabled(self.steamcmd && !self.tasks.running)
            .on_click(cx.listener(|d, _, window, cx| d.ask_track(window, cx)));
        let mut body = vec![
            div()
                .sized_text(theme, theme.small())
                .text_color(c.ink3)
                .child(put(&say!(
                    "These mods in the game's Mods folder were not downloaded by RimStack, so which version they are is not known."
                )))
                .into_any_element(),
        ];
        body.extend(self.named_rows(theme, "untracked", untracked, cx));
        let fold = cx.listener(|d, _, _, cx| d.fold("downloads-untracked", cx));
        let card = Card::new("untracked", put(&say!("Version not known")))
            .note(count(untracked.len()))
            .folds(self.is_folded("downloads-untracked", false), fold)
            .actions(
                vec![action.into_any_element()],
                btn_width(window, theme, &again, true, false),
            )
            .body(body);
        Some(card.render(theme, Some((width, window))).into_any_element())
    }

    pub(super) fn unlisted_card(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let unanswered = &self.updates.unanswered;
        if unanswered.is_empty() {
            return None;
        }
        let mut body = vec![
            div()
                .sized_text(theme, theme.small())
                .text_color(c.ink3)
                .child(put(&say!(
                    "Their authors made them unlisted: their Workshop pages open from a link, but Steam does not tell RimStack whether they have updates."
                )))
                .into_any_element(),
        ];
        body.extend(self.named_rows(theme, "unanswered", unanswered, cx));
        let fold = cx.listener(|d, _, _, cx| d.fold("downloads-unlisted", cx));
        let card = Card::new("unanswered", put(&say!("Unlisted on the Workshop")))
            .note(count(unanswered.len()))
            .folds(self.is_folded("downloads-unlisted", false), fold)
            .body(body);
        Some(card.render(theme, None).into_any_element())
    }
}
