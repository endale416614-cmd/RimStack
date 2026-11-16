use super::Mods;
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, notice};
use crate::words::put;
use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, IntoElement, ParentElement, Styled,
    StyledText, div, prelude::FluentBuilder, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;
use std::time::Duration;

const NOTE_STAYS: Duration = Duration::from_secs(8);

impl Mods {
    pub(super) fn toggle_term(&mut self, term: &str, cx: &mut Context<Self>) {
        let text = self.filter_text(cx);
        let had = text.split(' ').any(|word| word == term);
        let mut words: Vec<&str> = text
            .split(' ')
            .filter(|w| !w.is_empty() && *w != term)
            .collect();
        if !had {
            words.push(term);
        }
        let text = words.join(" ");
        self.filter_by(&text, cx);
    }

    fn sort_note(&self, cx: &gpui::App) -> Option<(String, String)> {
        let (first, rest) = self.list.read(cx).order.sort_note.split_first()?;
        let more = put(&Sentence::joined(" ", rest.to_vec()));
        Some((put(first), more))
    }

    fn close_note_later(&mut self, note: String, cx: &mut Context<Self>) {
        if self.status.note_timer == note {
            return;
        }
        self.status.note_timer = note.clone();
        cx.spawn(async move |mods, cx| {
            cx.background_executor().timer(NOTE_STAYS).await;
            let _ = mods.update(cx, |mods, cx| {
                if mods.sort_note(cx).is_some_and(|(shown, _)| shown == note) {
                    mods.status.closed_note = note;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn bands(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = &theme.colours;
        let mut shown = Vec::new();
        if self.demo {
            return shown;
        }
        let list = self.list.read(cx);
        let previewing = list.order.previewing();
        let dirty = list.standing.dirty && list.order.is_installed();
        let unsaved = list.standing.unsaved.clone();
        let band = |tone| {
            div()
                .flex_none()
                .px(theme::GAP_LARGE)
                .pb(theme::GAP)
                .child(notice(tone))
        };
        if dirty && !previewing {
            shown.push(
                band(c.warn)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .h(theme.sized(34.))
                            .pl(theme::GAP_LARGE)
                            .pr(theme::GAP_TINY)
                            .child(div().flex_none().size(px(7.)).rounded(px(3.5)).bg(c.warn))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .text_color(c.ink)
                                    .child(put(&say!("Not saved: {}", unsaved))),
                            )
                            .child(
                                Btn::new("show-changed", put(&say!("Show")))
                                    .tip(put(&say!(
                                        "Only the mods moved or activated since the last save"
                                    )))
                                    .on_click(
                                        cx.listener(|m, _, _, cx| m.toggle_term("is:changed", cx)),
                                    ),
                            )
                            .child(
                                Btn::new("discard-changes", put(&say!("Discard")))
                                    .tip(put(&say!(
                                        "Back to the saved list. Undo brings the changes back."
                                    )))
                                    .on_click(cx.listener(|m, _, _, cx| m.discard_changes(cx))),
                            )
                            .child(
                                Btn::new("save-band", put(&say!("Save")))
                                    .primary()
                                    .on_click(cx.listener(|m, _, _, cx| m.save(None, cx))),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let problem = match self.status.save_problem.is_empty() {
            true => self.status.upkeep_problem.clone(),
            false => put(&say!(
                "The list was not saved. {}",
                self.status.save_problem.clone()
            )),
        };
        if !problem.is_empty() {
            shown.push(
                div()
                    .flex_none()
                    .px(theme::GAP_CARD)
                    .pb(theme::GAP)
                    .sized_text(theme, theme.body())
                    .text_color(c.err)
                    .child(problem)
                    .into_any_element(),
            );
        }
        if self.status.file_changed {
            shown.push(
                div()
                    .flex()
                    .flex_none()
                    .gap(theme::GAP)
                    .pl(theme::GAP_CARD)
                    .pb(theme::GAP)
                    .child(
                        Btn::new("use-file", put(&say!("Use the file's")))
                            .tip(put(&say!(
                                "Replaces the list here with the one now in ModsConfig.xml"
                            )))
                            .on_click(cx.listener(|m, _, _, cx| m.keep_file(true, cx))),
                    )
                    .child(
                        Btn::new("keep-mine", put(&say!("Keep mine")))
                            .tip(put(&say!(
                                "Keeps the list here; the next Save overwrites ModsConfig.xml"
                            )))
                            .on_click(cx.listener(|m, _, _, cx| m.keep_file(false, cx))),
                    )
                    .into_any_element(),
            );
        }
        if let Some((note, more)) = self
            .sort_note(cx)
            .filter(|(note, _)| *note != self.status.closed_note)
        {
            if !previewing {
                self.close_note_later(note.clone(), cx);
            }
            let mut text = note.clone();
            let bold = 0..text.len();
            let mut marks = vec![(
                bold,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..HighlightStyle::default()
                },
            )];
            if previewing {
                let start = text.len() + 1;
                text = format!("{text} {}", put(&say!("Nothing has changed yet.")));
                marks.push((
                    start..text.len(),
                    HighlightStyle {
                        color: Some(c.ink2),
                        ..HighlightStyle::default()
                    },
                ));
            }
            if !more.is_empty() {
                text = format!("{text} {more}");
            }
            shown.push(
                band(c.focus)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .py(theme::GAP_SMALL)
                            .pl(theme::GAP_LARGE)
                            .pr(theme::GAP)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .spaced_text(theme, theme.body(), 1.25)
                                    .text_color(c.ink)
                                    .child(StyledText::new(text).with_highlights(marks)),
                            )
                            .when(previewing, |d| {
                                d.child(
                                    Btn::new("apply-sort", put(&say!("Apply")))
                                        .primary()
                                        .on_click(cx.listener(|m, _, _, cx| m.end_sort(true, cx))),
                                )
                                .child(
                                    Btn::new("discard-sort", put(&say!("Discard")))
                                        .on_click(cx.listener(|m, _, _, cx| m.end_sort(false, cx))),
                                )
                            })
                            .when(!previewing, |d| {
                                d.child(Btn::new("close-sort-note", put(&say!("OK"))).on_click(
                                    cx.listener(move |m, _, _, cx| {
                                        m.status.closed_note = note.clone();
                                        cx.notify();
                                    }),
                                ))
                            }),
                    )
                    .into_any_element(),
            );
        }
        shown
    }
}
