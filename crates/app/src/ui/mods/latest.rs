use super::chosen::DetailsView;
use crate::ui::card::Card;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, btn_width, label};
use crate::words::put;
use crate::workspace::load_order::Details;
use crate::workspace::logs::{day, moment};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use rimstack_core::say;
use rimstack_core::sources::changes::Note;

impl DetailsView {
    pub(super) fn latest(
        &self,
        theme: &Theme,
        details: &Details,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let workshop = details.workshop;
        let git = !workshop && !details.repository.is_empty();
        if self.shown.demo || !(workshop || git) {
            return None;
        }
        let listed = crate::ui::listed::read(cx);
        let notes: Vec<Note> = match workshop {
            true => listed
                .notes
                .get(&details.folder)
                .cloned()
                .unwrap_or_default(),
            false => self.shown.commits.as_deref().cloned().unwrap_or_default(),
        };
        let mut actions = Vec::new();
        let mut actions_width = gpui::px(0.);
        if workshop {
            let busy = listed.notes_busy;
            let said = match (busy, notes.is_empty()) {
                (true, _) => listed.notes_label.clone(),
                (false, true) => put(&say!("Fetch")),
                (false, false) => put(&say!("Fetch again")),
            };
            actions_width += btn_width(window, theme, &said, true, false) + theme::GAP_SMALL;
            let id = self.shown.chosen_id;
            actions.push(
                Btn::new("fetch-latest", said)
                    .icon("download")
                    .enabled(!busy)
                    .tip(put(&say!(
                        "Reads this mod's change notes from its Workshop page. Only its Workshop id is sent."
                    )))
                    .on_click(move |_, _, cx| {
                        let Some(id) = id else {
                            return;
                        };
                        crate::ui::navigate::go(crate::ui::navigate::Go::FetchNotes(id), cx);
                    })
                    .into_any_element(),
            );
            let page = crate::ui::change_notes(&details.folder);
            actions_width += btn_width(window, theme, "", true, false);
            actions.push(
                Btn::new("latest-page", "")
                    .icon("external-link")
                    .spoken(put(&say!("Change notes of {}", details.name.clone())))
                    .tip(put(&say!("The mod's change notes on the Workshop.")))
                    .on_click(move |_, _, cx| cx.open_url(&page))
                    .into_any_element(),
            );
        }
        let mut body: Vec<AnyElement> = Vec::new();
        if notes.is_empty() {
            let said = match (workshop, details.updated) {
                (false, _) => put(&say!("No commits")),
                (true, Some(at)) => {
                    put(&say!("Last updated {}; its notes are not fetched", day(at)))
                }
                (true, None) => put(&say!("Not fetched")),
            };
            body.push(
                div()
                    .w_full()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink3)
                    .child(said)
                    .into_any_element(),
            );
        }
        for note in notes {
            let text = match note.text.is_empty() {
                true => put(&say!("No note written")),
                false => note.text.clone(),
            };
            let mut one = div().flex().flex_col().gap(theme::GAP_HAIR);
            if note.time > 0 {
                one = one.child(label(theme, theme.small(), c.ink3, moment(note.time)));
            }
            one = one.child(
                div()
                    .w_full()
                    .line_clamp(14)
                    .spaced_text(theme, theme.body(), 1.25)
                    .text_color(if note.time > 0 { c.ink } else { c.err })
                    .child(text),
            );
            body.push(one.into_any_element());
        }
        let (folded, fold) = self.fold("about-changes", false, cx);
        let card = Card::new("about-changes", put(&say!("Latest changes")))
            .actions(actions, actions_width)
            .spacing(theme::GAP)
            .folds(folded, fold)
            .body(body);
        Some(card.render(theme, None).into_any_element())
    }
}
