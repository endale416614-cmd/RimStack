use super::Downloads;
use super::screen::text_fill;
use crate::ui::card::Card;
use crate::ui::controls::Choice;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, TextSized, btn_width, icon, label};
use crate::words::{count, put};
use crate::workspace::changed::{ChangedShown, Only, since_shown};
use crate::workspace::logs::{clock, day, moment};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};

const FIRST: usize = 5;

impl Downloads {
    fn changed_row(
        &self,
        theme: &Theme,
        at: usize,
        shown: &ChangedShown,
        today: &str,
        reached: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let row = shown.row;
        let mod_id = row.mod_id;
        let now = row.now.unwrap_or_default();
        let when = match shown.same_day {
            true => format!("{} → {}", clock(row.was), clock(now)),
            false => {
                let to = if day(now) == today {
                    put(&say!("today"))
                } else {
                    day(now)
                };
                format!("{} → {to}", day(row.was))
            }
        };
        let notes = crate::ui::change_notes(&row.id);
        let line = div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .child(icon(
                if row.git { "git-branch" } else { "cloud" },
                theme.icon(),
                if reached { c.focus } else { c.ink3 },
            ))
            .child(
                text_fill(theme, theme.body(), c.ink, row.name.clone())
                    .font_weight(theme.medium)
                    .when(reached, |d| d.underline()),
            )
            .when(!row.active, |d| d.child(Chip::new(put(&say!("inactive")))))
            .when(row.git, |d| {
                d.child(
                    Chip::new(put(&say_n!(row.notes.len(), "{} commit", "{} commits")))
                        .tone(c.moved),
                )
            })
            .when(!row.git, |d| {
                d.child(label(theme, theme.small(), c.ink3, when))
            })
            .child(
                Btn::new(("changed-show", at), put(&say!("Show")))
                    .spoken(put(&say!("Show {}", row.name.clone())))
                    .tip(put(&say!("Enter")))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(mod_id), cx)
                    })),
            )
            .when(!row.git, |d| {
                d.child(
                    Btn::new(("changed-notes", at), "")
                        .spoken(put(&say!("Change notes of {}", row.name.clone())))
                        .tip(put(&say!("The mod's change notes on the Workshop.")))
                        .icon("external-link")
                        .on_click(move |_, _, cx| cx.open_url(&notes)),
                )
            });
        let said = shown.said.iter().map(|note| {
            let text = match note.text.is_empty() {
                true => put(&say!("No note written")),
                false => note.text.clone(),
            };
            let colour = if note.time > 0 { c.ink2 } else { c.err };
            div()
                .flex()
                .gap(theme::GAP)
                .child(div().w(theme.sized(22.)).flex_none())
                .child(
                    label(
                        theme,
                        theme.small(),
                        c.ink3,
                        if note.time > 0 {
                            day(note.time)
                        } else {
                            String::new()
                        },
                    )
                    .w(theme.sized(74.)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .line_clamp(12)
                        .spaced_text(theme, theme.small(), 1.25)
                        .text_color(colour)
                        .child(text),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(theme::GAP_TINY)
            .when(shown.opens_day, |d| {
                let head = format!(
                    "{}  {}",
                    day(now),
                    put(&say_n!(shown.day_count, "{} mod", "{} mods"))
                );
                d.child(
                    label(theme, theme.small(), c.ink2, head)
                        .font_weight(theme.semibold)
                        .when(at > 0, |d| d.pt(theme::GAP))
                        .pb(theme::GAP_THIN),
                )
            })
            .child(line)
            .children(said)
            .child({
                let drawn = self.changed_drawn.clone();
                gpui::canvas(
                    move |bounds, _, _| {
                        let mut drawn = drawn.borrow_mut();
                        if drawn.len() <= at {
                            drawn.resize(at + 1, gpui::Bounds::default());
                        }
                        drawn[at] = bounds;
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
            })
            .relative()
            .into_any_element()
    }

    pub(super) fn changed_card(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let since = since_shown(&self.changed, self.since_only, &self.fetched);
        if since.count == 0 {
            return None;
        }
        let today = day(crate::workspace::now_secs());
        let most = if self.since_open {
            since.rows.len()
        } else {
            FIRST
        };
        let keyed = self.changed_keys.is_focused(window);
        let rows: Vec<AnyElement> = since
            .rows
            .iter()
            .take(most)
            .enumerate()
            .map(|(at, shown)| {
                let reached = keyed && at == self.changed_at;
                self.changed_row(theme, at, shown, &today, reached, cx)
            })
            .collect();
        let shown: Vec<(ModId, Option<String>)> = since
            .rows
            .iter()
            .take(most)
            .map(|shown| {
                let row = shown.row;
                (row.mod_id, (!row.git).then(|| row.id.clone()))
            })
            .collect();
        let mut body = vec![
            div()
                .id("changed-rows")
                .track_focus(&self.changed_keys)
                .role(gpui::Role::List)
                .aria_label(put(&say!("Changed since the game last started")))
                .flex()
                .flex_col()
                .gap(theme::GAP_LARGE)
                .children(rows)
                .on_key_down(cx.listener(move |d, event: &gpui::KeyDownEvent, _, cx| {
                    d.changed_key(&shown, event, cx)
                }))
                .into_any_element(),
        ];
        if since.rows.len() > FIRST {
            let (open, total) = (self.since_open, since.rows.len());
            body.push(super::more(
                "changed-more",
                open,
                total,
                FIRST,
                cx,
                |d, cx| {
                    d.since_open = !d.since_open;
                    cx.notify();
                },
            ));
        }
        let mut actions: Vec<AnyElement> = Vec::new();
        let mut actions_width = Pixels::ZERO;
        if since.active > 0 && since.active < since.count {
            let names = [
                put(&say!("All")),
                put(&say!("Active {}", since.active)),
                put(&say!("Inactive {}", since.count - since.active)),
            ];
            let picked = match self.since_only {
                Only::All => 0,
                Only::Active => 1,
                Only::Inactive => 2,
            };
            let names: Vec<gpui::SharedString> = names.into_iter().map(Into::into).collect();
            actions_width += Choice::width(window, theme, &names);
            let me = cx.entity().downgrade();
            actions.push(
                Choice::new("changed-only", names, picked)
                    .title(put(&say!("Changed mods shown")))
                    .on_pick(move |at, _, cx| {
                        let _ = me.update(cx, |d, cx| {
                            d.since_only = [Only::All, Only::Active, Only::Inactive][at];
                            cx.notify();
                        });
                    })
                    .into_any_element(),
            );
        }
        if since.workshop > 0 {
            let ask = self.ask_steam_label();
            actions_width += btn_width(window, theme, &ask, true, false) + theme::GAP_SMALL;
            actions.push(
                Btn::new("fetch-notes", ask)
                    .tip(put(&say!("Reads each mod's change notes from its Workshop page. Only the Workshop ids are sent.")))
                    .icon("download")
                    .enabled(!self.notes_busy)
                    .on_click(cx.listener(|d, _, _, cx| d.fetch_since_notes(cx)))
                    .into_any_element(),
            );
        }
        let note = put(&say!(
            "{}, since {}",
            count(since.count),
            moment(since.time)
        ));
        let fold = cx.listener(|d, _, _, cx| d.fold("downloads-changed", cx));
        let card = Card::new(
            "downloads-changed",
            put(&say!("Changed since the game last started")),
        )
        .note(note)
        .folds(self.is_folded("downloads-changed", false), fold)
        .actions(actions, actions_width)
        .spacing(theme::GAP_LARGE)
        .body(body);
        Some(card.render(theme, Some((width, window))).into_any_element())
    }

    fn changed_key(
        &mut self,
        shown: &[(ModId, Option<String>)],
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let keys = &event.keystroke;
        if keys.modifiers.modified() || shown.is_empty() {
            return;
        }
        let at = self.changed_at.min(shown.len() - 1);
        match keys.key.as_str() {
            "up" | "down" => {
                self.changed_at = match keys.key.as_str() {
                    "up" => at.saturating_sub(1),
                    _ => (at + 1).min(shown.len() - 1),
                };
                if let Some(drawn) = self.changed_drawn.borrow().get(self.changed_at) {
                    let room = theme::GAP_LARGE;
                    self.paging
                        .contain_drawn(drawn.top() - room, drawn.bottom() + room);
                }
                cx.notify();
            }
            "enter" => crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(shown[at].0), cx),
            "n" => {
                if let Some(id) = &shown[at].1 {
                    cx.open_url(&crate::ui::change_notes(id));
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    pub fn ask_steam_label(&self) -> String {
        let Some(state) = self.notes_state.as_ref().filter(|_| self.notes_busy) else {
            return match self.notes_busy {
                true => put(&say!("Asking Steam…")),
                false => put(&say!("Fetch change notes")),
            };
        };
        let waits = state.resume.saturating_sub(crate::workspace::now_secs());
        if waits > 0 {
            put(&say!("Steam asks to wait, {} s", waits))
        } else if state.asked > 1 {
            put(&say!(
                "Asking Steam, {} of {}",
                (state.done + 1).min(state.asked),
                state.asked
            ))
        } else {
            put(&say!("Asking Steam…"))
        }
    }
}
