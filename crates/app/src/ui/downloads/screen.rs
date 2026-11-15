use super::Downloads;
use super::pane::SideDivider;
use crate::ui::card::Card;
use crate::ui::field::Field;
use crate::ui::page::{page, room};
use crate::ui::problem::problem;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, TextSized, btn_width, chip_width, icon, label};
use crate::words::{count, put};
use crate::workspace::downloads::{BehindShown, CheckoutShown, updates_shown};
use crate::workspace::logs::day;
use gpui::{
    AnyElement, Context, DragMoveEvent, InteractiveElement, IntoElement, ParentElement, Pixels,
    Render, Styled, Window, canvas, div, fill, point, prelude::FluentBuilder, size,
};
use rimstack_core::sources::downloads::Origin;
use rimstack_core::{say, say_n};

const FIRST_INACTIVE: usize = 3;

impl Downloads {
    fn paste_row(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> gpui::Div {
        let typed = !self.pasted.read(cx).text().trim().is_empty();
        let field = Field {
            id: "pasted",
            text: &self.pasted,
            icon: "link",
            placeholder: put(&say!(
                "Paste Workshop links, collection links, mod ids or Git links"
            )),
            found: None,
            keys: None,
        };
        let look = match self.queue_busy {
            true => put(&say!("Asking Steam…")),
            false => put(&say!("Look up")),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::GAP)
            .pt(theme::GAP_WIDE)
            .px(theme::GAP_WIDE)
            .pb(theme::GAP_MEDIUM)
            .child(field.render(theme, window, cx).flex_1().min_w_0())
            .child(
                Btn::new("look-up", look)
                    .tip(put(&say!(
                        "Looks up what is pasted beside it: paste a link or an id first."
                    )))
                    .primary_if(typed)
                    .enabled(!self.queue_busy && typed)
                    .on_click(cx.listener(|downloads, _, _, cx| downloads.look_up(cx))),
            )
    }

    fn banner(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let c = &theme.colours;
        if self.report.auto_updates_asked {
            return None;
        }
        let said = put(&say!(
            "Steam is asked about the installed Workshop mods and the mods SteamCMD put in the game's Mods folder (only their Workshop ids are sent) and Git about each checkout, as often as chosen. Steam then downloads the newer versions, SteamCMD downloads again the mods RimStack downloaded with it, and each checkout with no local changes is updated, except while the game runs."
        ));
        Some(
            div()
                .rounded(theme::CORNER)
                .bg(c.banner)
                .px(theme::GAP_LARGE)
                .py(theme::GAP_MEDIUM)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP_MEDIUM)
                        .child(icon("refresh-cw", theme.icon(), c.focus).self_start())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(theme::GAP_HAIR)
                                .child(
                                    div()
                                        .sized_text(theme, theme.body())
                                        .font_weight(theme.semibold)
                                        .text_color(c.ink)
                                        .child(put(&say!("Keep mods up to date by themselves?"))),
                                )
                                .child(
                                    div()
                                        .sized_text(theme, theme.small())
                                        .text_color(c.ink2)
                                        .child(said),
                                ),
                        )
                        .child(
                            Btn::new("auto-on", put(&say!("Turn on")))
                                .primary()
                                .on_click(cx.listener(|d, _, _, cx| {
                                    d.keep_setting("autoUpdates", "on", cx)
                                })),
                        )
                        .child(Btn::new("auto-off", put(&say!("Not now"))).on_click(
                            cx.listener(|d, _, _, cx| d.keep_setting("autoUpdates", "off", cx)),
                        )),
                ),
        )
    }

    fn status(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let c = &theme.colours;
        let shown = updates_shown(&self.updates, &self.asked_of_steam);
        let quiet = !self.updates_busy && shown.problem.is_none();
        let checked = match shown.checked {
            Some(_) => put(&shown.checked_said),
            None => put(&say!("Never checked")),
        };
        let row = div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .when(self.report.auto_updates, |d| {
                let every = put(&self.report.update_every_said);
                d.child(Chip::new(every).tone(c.ok).icon("refresh-cw").tip(put(&say!("Settings, Updates: Update by themselves"))))
            })
            .when(self.steam_waits, |d| {
                d.child(
                    Chip::new(put(&say!("Steam is not running: the updates wait for it")))
                        .tone(c.warn)
                        .icon("clock"),
                )
                .child(
                    Btn::new("start-steam", put(&say!("Start Steam")))
                        .on_click(|_, _, cx| cx.open_url("steam://open/main")),
                )
            })
            .when(self.updates_busy, |d| {
                d.child(Chip::new(put(&say!("Asking Steam…"))))
            })
            .when(!self.updates_busy && shown.problem.is_some(), |d| {
                d.child(
                    Chip::new(put(&say!("Steam not reached")))
                        .tone(c.err)
                        .icon("circle-alert"),
                )
            })
            .when(quiet && shown.checked.is_some() && shown.count == 0, |d| {
                d.child(
                    Chip::new(put(&say!("All Workshop mods are current")))
                        .tone(c.ok)
                        .icon("check"),
                )
            })
            .when(quiet && shown.count > 0, |d| {
                let said = put(&say_n!(
                    shown.count,
                    "{} update available",
                    "{} updates available"
                ));
                d.child(Chip::new(said).tone(c.moved).icon("download"))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(theme, theme.small())
                    .text_color(c.ink3)
                    .child(checked),
            )
            .child(
                Btn::new("check-now", put(&say!("Check now")))
                    .tip(match self.report.git_for_workshop {
                        true => put(&say!("Asks Steam for the current version of each installed Workshop mod and of each SteamCMD download in the game's Mods folder (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder and in the Workshop folder.")),
                        false => put(&say!("Asks Steam for the current version of each installed Workshop mod and of each SteamCMD download in the game's Mods folder (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder.")),
                    })
                    .icon("refresh-cw")
                    .primary_if(shown.checked.is_none())
                    .enabled(!self.updates_busy)
                    .on_click(cx.listener(|downloads, _, _, cx| downloads.check_updates(cx))),
            );
        let problem = shown
            .problem
            .map(|problem| put(&say!("Steam not reached: {}", put(problem))));
        div()
            .flex()
            .flex_col()
            .gap(theme::GAP_SMALL)
            .child(row)
            .when(self.updates_busy, |d| d.child(busy_bar(theme)))
            .when_some(problem, |d, said| {
                d.child(
                    div()
                        .sized_text(theme, theme.body())
                        .text_color(c.err)
                        .child(said),
                )
            })
    }

    fn update_row(
        &self,
        theme: &Theme,
        key: (&'static str, usize),
        shown: &BehindShown,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let row = shown.row;
        let mod_id = row.mod_id;
        let notes = crate::ui::change_notes(&row.id);
        div()
            .flex()
            .items_center()
            .gap(theme::GAP_LARGE)
            .child(match row.origin {
                Origin::Steam => icon("cloud", theme.icon(), c.ink3),
                Origin::SteamCmd | Origin::Linked => icon("package", theme.icon(), c.ink3),
            })
            .child(text_fill(theme, theme.body(), c.ink, row.name.clone()))
            .when(shown.asked, |d| {
                d.child(
                    Chip::new(put(&say!("Sent to Steam")))
                        .tone(c.ok)
                        .icon("check"),
                )
            })
            .when_some(shown.refused, |d, why| {
                d.child(
                    Chip::new(put(&say!("Not updated")))
                        .tone(c.err)
                        .icon("circle-alert")
                        .tip(put(why)),
                )
            })
            .child(
                Chip::new(format!("{} → {}", day(row.have), day(row.now)))
                    .tone(c.moved)
                    .tip(put(&say!(
                        "Published {}",
                        rimstack_core::base::elapsed::ago(
                            crate::workspace::now_secs().saturating_sub(row.now)
                        )
                        .inside()
                    ))),
            )
            .child(
                Btn::new((key.0, key.1 * 2), put(&say!("Show")))
                    .spoken(put(&say!("Show {}", row.name.clone())))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(mod_id), cx)
                    })),
            )
            .child(
                Btn::new((key.0, key.1 * 2 + 1), "")
                    .spoken(put(&say!("What changed in {}", row.name.clone())))
                    .tip(put(&say!("The mod's change notes on the Workshop.")))
                    .icon("external-link")
                    .on_click(move |_, _, cx| cx.open_url(&notes)),
            )
            .into_any_element()
    }

    pub(super) fn checkout_row(
        &self,
        theme: &Theme,
        key: (&'static str, usize),
        width: Pixels,
        shown: &CheckoutShown,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let mod_id = shown.row.mod_id;
        let updating = self.tasks.updating == Some(mod_id);
        let tone = [c.ink2, c.ok, c.moved, c.err][usize::from(shown.standing).min(3)];
        let update = if updating {
            put(&say!("Updating…"))
        } else {
            put(&say!("Update"))
        };
        div()
            .flex()
            .items_center()
            .gap(theme::GAP_LARGE)
            .child(icon("git-branch", theme.icon(), c.ink3))
            .child(text_fill(
                theme,
                theme.body(),
                c.ink,
                shown.row.name.clone(),
            ))
            .child(Chip::new(put(&shown.said)).tone(tone).max_width(width / 2.))
            .when(shown.updatable, |d| {
                d.child(
                    Btn::new((key.0, key.1 * 2), update)
                        .spoken(put(&say!("Update {}", shown.row.name.clone())))
                        .tip(put(&say!(
                            "Updates to the newest commit; stops if the folder has local changes."
                        )))
                        .enabled(self.tasks.updating.is_none())
                        .primary_if(shown.due && !updating)
                        .on_click(cx.listener(move |downloads, _, _, cx| {
                            downloads.update_from_git(mod_id, cx)
                        })),
                )
            })
            .child(
                Btn::new((key.0, key.1 * 2 + 1), put(&say!("Show")))
                    .spoken(put(&say!("Show {}", shown.row.name.clone())))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(mod_id), cx)
                    })),
            )
            .into_any_element()
    }

    fn page_content(
        &self,
        theme: &Theme,
        width: Pixels,
        side: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let shown = updates_shown(&self.updates, &self.asked_of_steam);
        let needed = self.needed();
        let mut cards: Vec<AnyElement> = Vec::new();
        cards.extend(self.banner(theme, cx).map(IntoElement::into_any_element));
        cards.push(self.status(theme, cx).into_any_element());
        let needs = needed.len() + shown.checkouts_failed.len();
        if needs > 0 {
            let mut body: Vec<AnyElement> = needed
                .iter()
                .enumerate()
                .map(|(at, said)| {
                    let act = crate::ui::mods::acting_from_afar(said);
                    problem(theme, at, said, Some((width, window)), Some(act))
                })
                .collect();
            body.extend(
                shown
                    .checkouts_failed
                    .iter()
                    .enumerate()
                    .map(|(at, row)| self.checkout_row(theme, ("failed", at), width, row, cx)),
            );
            let fold = cx.listener(|d, _, _, cx| d.fold("downloads-needs", cx));
            let card = Card::new("needs-you", put(&say!("Needs you")))
                .tone(c.warn)
                .note(count(needs))
                .folds(self.is_folded("downloads-needs", false), fold)
                .body(body);
            cards.push(card.render(theme, None).into_any_element());
        }
        if !side {
            cards.push(self.add_mods(theme, width, window, cx).into_any_element());
        }
        let behind = shown.count + shown.checkouts_due.len();
        if self.updates_busy || behind > 0 {
            let note = match self.updates_busy {
                true => put(&say!("Asking Steam…")),
                false => put(&say!("{} for active mods", shown.active.len())),
            };
            let mut actions: (Vec<AnyElement>, Pixels) = (Vec::new(), Pixels::ZERO);
            let by_steam = self.updates.by_steam().next().is_some();
            if by_steam {
                let update = put(&say!("Update in Steam"));
                actions.1 += btn_width(window, theme, &update, true, false);
                let action = Btn::new("update-in-steam", update)
                    .tip(put(&say!("Steam must be running. It downloads the newer version of each now; otherwise it does so the next time it starts.")))
                    .icon("download")
                    .primary_if(shown.active.iter().any(|row| row.row.origin == Origin::Steam))
                    .enabled(!self.tasks.running)
                    .on_click(cx.listener(|downloads, _, _, cx| downloads.update_in_steam(cx)));
                actions.0.push(action.into_any_element());
            }
            if self.updates.by_steamcmd().next().is_some() {
                let update = put(&say!("Update with SteamCMD"));
                actions.1 += btn_width(window, theme, &update, true, false);
                let action = Btn::new("update-with-steamcmd", update)
                    .tip(put(&say!("For the mods SteamCMD put in the game's Mods folder: each is downloaded again and replaces its folder.")))
                    .icon("download")
                    .primary_if(!by_steam && !shown.active.is_empty())
                    .enabled(self.steamcmd && !self.tasks.running)
                    .on_click(cx.listener(|downloads, _, window, cx| downloads.ask_update_copies(window, cx)));
                actions.0.push(action.into_any_element());
            }
            if actions.0.len() > 1 {
                actions.1 += theme::GAP;
            }
            let mut body: Vec<AnyElement> = shown
                .active
                .iter()
                .enumerate()
                .map(|(at, row)| self.update_row(theme, ("active-update", at), row, cx))
                .collect();
            body.extend(
                shown
                    .checkouts_due
                    .iter()
                    .enumerate()
                    .map(|(at, row)| self.checkout_row(theme, ("due", at), width, row, cx)),
            );
            if !shown.inactive.is_empty() {
                body.push(
                    div()
                        .flex()
                        .gap(theme::GAP)
                        .when(!shown.active.is_empty(), |d| d.pt(theme::GAP))
                        .child(
                            label(theme, theme.small(), c.ink2, put(&say!("Inactive mods")))
                                .font_weight(theme.semibold),
                        )
                        .child(label(
                            theme,
                            theme.small(),
                            c.ink3,
                            shown.inactive.len().to_string(),
                        ))
                        .into_any_element(),
                );
                let most = if self.inactive_open {
                    shown.inactive.len()
                } else {
                    FIRST_INACTIVE
                };
                body.extend(
                    shown
                        .inactive
                        .iter()
                        .take(most)
                        .enumerate()
                        .map(|(at, row)| self.update_row(theme, ("inactive-update", at), row, cx)),
                );
                if shown.inactive.len() > FIRST_INACTIVE {
                    let (open, total) = (self.inactive_open, shown.inactive.len());
                    body.push(super::more(
                        "inactive-more",
                        open,
                        total,
                        FIRST_INACTIVE,
                        cx,
                        |d, cx| {
                            d.inactive_open = !d.inactive_open;
                            cx.notify();
                        },
                    ));
                }
            }
            let fold = cx.listener(|d, _, _, cx| d.fold("downloads-newer", cx));
            let card = Card::new("newer", put(&say!("Newer versions")))
                .note(note)
                .folds(self.is_folded("downloads-newer", false), fold)
                .actions(actions.0, actions.1)
                .body(body);
            cards.push(card.render(theme, Some((width, window))).into_any_element());
        }
        cards.extend(self.untracked_card(theme, width, window, cx));
        cards.extend(self.changed_card(theme, width, window, cx));
        if !shown.checkouts.is_empty() {
            let rows: Vec<AnyElement> = shown
                .checkouts
                .iter()
                .enumerate()
                .map(|(at, row)| self.checkout_row(theme, ("checkout", at), width, row, cx))
                .collect();
            let fold = cx.listener(|d, _, _, cx| d.fold("downloads-git", cx));
            let card = Card::new("downloads-git", put(&say!("From Git")))
                .note(put(&shown.checkouts_said))
                .folds(self.is_folded("downloads-git", true), fold)
                .body(rows);
            cards.push(card.render(theme, None).into_any_element());
        }
        cards.extend(self.replacements_card(theme, width, window, cx));
        cards.extend(self.unlisted_card(theme, cx));
        div()
            .flex()
            .flex_col()
            .gap(theme::GAP_PAGE)
            .px(theme::GAP_WIDE)
            .pb(theme::GAP_WIDE)
            .children(cards)
    }

    fn replacements_card(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let ignored = self.report.replacements_ignored.len();
        if self.replacements.is_empty() && ignored == 0 {
            return None;
        }
        let title = put(&say_n!(
            self.replacements.len(),
            "{} mod has a replacement",
            "{} mods have a replacement"
        ));
        let again = put(&say!("{} ignored", ignored));
        let actions = match ignored > 0 {
            true => (
                vec![
                    Chip::new(again.clone())
                        .icon("eye-off")
                        .tip(put(&say!("Suggest them again")))
                        .on_click(
                            "suggest-again",
                            cx.listener(|d, _, _, cx| d.ignore_replacement(None, cx)),
                        )
                        .into_any_element(),
                ],
                chip_width(window, theme, &again, true),
            ),
            false => (Vec::new(), Pixels::ZERO),
        };
        let rows = self.replacements.iter().enumerate().map(|(at, old)| {
            let mod_id = old.mod_id;
            let page = crate::ui::workshop_page(&old.new_id);
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_LARGE)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(theme::GAP_HAIR)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .child(
                                    text_fill(theme, theme.body(), c.ink2, old.name.clone())
                                        .flex_initial(),
                                )
                                .when(!old.active, |d| d.child(Chip::new(put(&say!("inactive"))))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .child(icon("corner-down-right", theme.icon(), c.ink3))
                                .child(
                                    text_fill(theme, theme.body(), c.ink, old.new_name.clone())
                                        .font_weight(theme.medium)
                                        .flex_initial(),
                                )
                                .when(old.have_new, |d| {
                                    d.child(
                                        Chip::new(put(&say!("installed"))).tone(c.ok).icon("check"),
                                    )
                                }),
                        ),
                )
                .when(!old.have_new, |d| {
                    d.child(
                        Btn::new(("replacement", at), put(&say!("Replacement"))).tip(put(&say!("Workshop page of the replacement")))
                            .icon("external-link")
                            .on_click(move |_, _, cx| cx.open_url(&page)),
                    )
                })
                .child(
                    Btn::new(("show-old", at), put(&say!("Show old mod")))
                        .spoken(put(&say!("Show {}", old.name.clone())))
                        .tip(put(&say!("The installed mod the list would replace")))
                        .on_click(cx.listener(move |_, _, _, cx| crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(mod_id), cx))),
                )
                .child(
                    Btn::new(("ignore-replacement", at), put(&say!("Ignore")))
                        .spoken(put(&say!("Ignore the replacement of {}", old.name.clone())))
                        .tip(put(&say!("No replacement is suggested for this mod again")))
                        .icon("eye-off")
                        .on_click(
                            cx.listener(move |d, _, _, cx| d.ignore_replacement(Some(mod_id), cx)),
                        ),
                )
                .child(
                    Btn::new(("unsubscribe", at), put(&say!("Unsubscribe…")))
                        .spoken(put(&say!("Unsubscribe from {}", old.name.clone())))
                        .tip(put(&say!("Removes the replaced mod: Steam unsubscribes from it and deletes its folder")))
                        .danger()
                        .on_click({
                            let name = old.name.clone();
                            cx.listener(move |d, _, window, cx| {
                                d.ask_unsubscribe(mod_id, name.clone(), window, cx)
                            })
                        }),
                )
                .into_any_element()
        });
        let fold = cx.listener(|d, _, _, cx| d.fold("downloads-replacements", cx));
        let card = Card::new("replacements", title)
            .folds(self.is_folded("downloads-replacements", false), fold)
            .actions(actions.0, actions.1)
            .body(rows.collect());
        Some(card.render(theme, Some((width, window))).into_any_element())
    }
}

pub(super) fn text_fill(
    theme: &Theme,
    size: Pixels,
    colour: gpui::Hsla,
    text: String,
) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .sized_text(theme, size)
        .text_color(colour)
        .child(text)
}

fn busy_bar(theme: &Theme) -> impl IntoElement {
    let (ground, slider) = (theme.colours.chip, theme.colours.accent);
    let reduce = theme.appearance.reduce_motion;
    let height = theme::GAP_THIN;
    div()
        .w_full()
        .h(height)
        .rounded(height / 2.)
        .bg(ground)
        .overflow_hidden()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let width = bounds.size.width / 4.;
                    let part = match reduce {
                        true => 0.,
                        false => {
                            (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                                % 1200) as f32
                                / 1200.
                        }
                    };
                    let x = bounds.left() - width + (bounds.size.width + width) * part;
                    window.paint_quad(
                        fill(
                            gpui::Bounds::new(point(x, bounds.top()), size(width, height)),
                            slider,
                        )
                        .corner_radii(height / 2.),
                    );
                    if !reduce {
                        window.request_animation_frame();
                    }
                },
            )
            .size_full(),
        )
}

impl Render for Downloads {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let whole = window.viewport_size().width;
        let pane = self.side_shown(&theme, window);
        let page_width = match pane {
            Some(pane) => whole - pane - theme::HANDLE,
            None => whole,
        } - theme::GAP_WIDE * 2.;
        let view = cx.entity_id();
        let content = self.page_content(&theme, page_width, pane.is_some(), window, cx);
        let paste = self.paste_row(&theme, window, cx);
        div().flex().flex_col().size_full().child(paste).child(
            div()
                .id("downloads-panes")
                .flex()
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().h_full().child(page(
                    "downloads-page",
                    &self.paging,
                    &theme,
                    view,
                    content,
                )))
                .when_some(pane, |d, pane| {
                    let inner = pane - theme::GAP_WIDE * 2. - room(&theme);
                    let adding = self.add_mods(&theme, inner, window, cx);
                    d.child(self.side_divider(&theme, window, cx)).child(
                        div().flex_none().w(pane).h_full().bg(c.panel).child(page(
                            "downloads-side",
                            &self.side_paging,
                            &theme,
                            view,
                            div()
                                .p(theme::GAP_WIDE)
                                .pr(theme::GAP_WIDE + room(&theme))
                                .child(adding),
                        )),
                    )
                })
                .on_drag_move::<SideDivider>(cx.listener(
                    |d, event: &DragMoveEvent<SideDivider>, _, cx| {
                        let at = event.event.position.x;
                        d.set_side(event.bounds.right() - at - theme::HANDLE / 2., cx);
                    },
                ))
                .on_drop(cx.listener(|_, _: &SideDivider, _, _| {})),
        )
    }
}
