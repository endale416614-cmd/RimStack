use super::chosen::DetailsView;
use super::edit::acting;
use crate::ui::button::Btn;
use crate::ui::card::Card;
use crate::ui::problem::problem;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, Pixels, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::{say, say_n};

impl DetailsView {
    pub(super) fn problems_pane(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &Window,
    ) -> Option<AnyElement> {
        let overview = self.shown.overview.clone()?;
        let c = &theme.colours;
        let mods = self.mods.clone();
        let run = |f: fn(&mut super::Mods, &mut gpui::Context<super::Mods>)| {
            let mods = mods.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                let _ = mods.update(cx, f);
            }
        };
        let none = overview.troubles.is_empty();
        let (errors, warnings) = (overview.error_count, self.shown.warnings);
        let summary = match none {
            true => put(&say!("None")),
            false => put(&say!("{} with errors, {} with warnings", errors, warnings)),
        };
        let version = overview.game_version.clone();
        let groups = overview.trouble_groups.iter().enumerate().map(|(at, group)| {
            let tone = if group.error { c.err } else { c.warn };
            let mut body: Vec<AnyElement> = Vec::new();
            if group.trusted > 1 {
                let version = version.clone();
                body.push(
                    div().flex().child(Btn::new(("ignore-all", at), put(&say!("Ignore all {} for RimWorld {}", group.trusted, version.clone())))
                        .tip(put(&say!("No warning about the game version while RimWorld stays on this version.")))
                        .on_click({
                            let mods = mods.clone();
                            move |_, _, cx| {
                                let version = version.clone();
                                let _ = mods.update(cx, |mods, cx| mods.set_version_aside(version, cx));
                            }
                        }))
                        .into_any_element(),
                );
            }
            if group.shared {
                body.push(
                    div()
                        .spaced_text(theme, theme.body(), 1.25)
                        .text_color(c.ink2)
                        .child(put(&say!(
                            "None of these lists RimWorld {} among its game versions. Many such mods still work.",
                            version.clone()
                        )))
                        .into_any_element(),
                );
                body.extend(group.lines.iter().enumerate().map(|(line, said)| {
                    let act = acting(mods.clone(), said);
                    let (shown, ignored) = (act.clone(), act);
                    div()
                        .id(("shared-problem", at * 1000 + line))
                        .group("shared-problem")
                        .relative()
                        .flex()
                        .items_center()
                        .h(theme.control())
                        .px(theme::GAP_SMALL)
                        .rounded(theme::CORNER)
                        .hover(|d| d.bg(crate::ui::widgets::with_alpha(c.hover, 0.05)))
                        .cursor_pointer()
                        .on_click(move |_, window, cx| shown(crate::ui::problem::Act::Show, window, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(said.name.clone()),
                        )
                        .child(
                            div().invisible().group_hover("shared-problem", |s| s.visible()).child(
                                Btn::new(("ignore-version", at * 1000 + line), put(&say!("Ignore")))
                                    .tip(put(&say!("No warning about the game version for this mod")))
                                    .on_click(move |_, window, cx| ignored(crate::ui::problem::Act::Fix, window, cx)),
                            ),
                        )
                        .into_any_element()
                }));
            } else {
                body.extend(group.lines.iter().enumerate().map(|(line, said)| {
                    problem(theme, at * 1000 + line, said, Some((width, window)), Some(acting(mods.clone(), said)))
                }));
            }
            Card::new(("problems", at), put(&Sentence::said(group.name, Vec::new())))
                .tone(tone)
                .note(group.lines.len().to_string())
                .spacing(theme::GAP)
                .body(body)
                .render(theme, None)
                .into_any_element()
        });
        let needed = overview.needed.len();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(theme::GAP_LARGE)
                .child(
                    div()
                        .flex()
                        .items_start()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(label(theme, theme.heading(), c.ink, put(&say!("Problems"))).font_weight(theme.semibold))
                                .child(label(theme, theme.body(), c.ink2, summary).pt(theme::GAP_HAIR)),
                        )
                        .child(
                            Btn::new("close-problems", "")
                                .icon("x")
                                .tip(put(&say!("Close (Esc)")))
                                .on_click(run(super::Mods::close_problems)),
                        ),
                )
                .when(needed > 0, |d| {
                    d.child(
                        div().flex().child(
                            Btn::new("fix-all", put(&say_n!(needed, "Activate the {} needed mod", "Activate the {} needed mods")))
                                .primary()
                                .tip(put(&say!("Activates every needed mod that is installed, each where its rules put it. One Undo takes them all back.")))
                                .on_click(run(super::Mods::fix_all)),
                        ),
                    )
                })
                .when(!self.shown.demo && !self.shown.hunting, |d| {
                    d.child(
                        div().flex().child(
                            Btn::new("find-broken-panel", put(&say!("Find the broken mod")))
                                .icon("search")
                                .tip(put(&say!("Deactivates half of the mods; you play and say whether it still breaks. This repeats until one mod is left. Your list is kept and comes back at the end.")))
                                .on_click(run(|_, cx| crate::ui::navigate::go(crate::ui::navigate::Go::FindBroken, cx))),
                        ),
                    )
                })
                .children(groups)
                .when(none, |d| {
                    d.child(crate::ui::widgets::empty_state(theme, width, "", put(&say!("No problems")), String::new()))
                })
                .into_any_element(),
        )
    }
}
