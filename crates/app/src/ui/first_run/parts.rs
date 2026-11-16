use crate::reports::SettingsReport;

use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_middle_in, icon, label, measure};
use crate::words::put;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Role,
    SharedString, StatefulInteractiveElement, Styled, StyledImage, Toggled, Window, div,
    prelude::FluentBuilder, px,
};

use rimstack_core::say;

use super::FirstRun;

impl FirstRun {
    pub(super) fn block(theme: &Theme, title: String, lines: Vec<AnyElement>) -> AnyElement {
        let c = &theme.colours;
        let count = lines.len();
        let lines = lines.into_iter().enumerate().map(|(at, line)| {
            div()
                .when(at + 1 < count, |d| d.border_b_1().border_color(c.line))
                .child(line)
        });
        div()
            .flex()
            .flex_col()
            .pt(theme::GAP_WIDE)
            .child(
                label(theme, theme.small(), c.ink2, title)
                    .font_weight(theme.semibold)
                    .pl(theme::GAP_LARGE)
                    .pb(theme::GAP_SMALL),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(theme::CORNER_LARGE)
                    .bg(c.card)
                    .children(lines),
            )
            .into_any_element()
    }

    /// `Found`: a thing found and where, or a sentence; `note` for words that only tell.
    pub(super) fn found(
        theme: &Theme,
        window: &Window,
        what: String,
        place: Option<String>,
        sentence: bool,
        note: bool,
        width: Pixels,
    ) -> AnyElement {
        let c = &theme.colours;
        let (mark, tone) = match (note, place.is_some()) {
            (true, _) => ("•", c.ink3),
            (false, true) => ("✓", c.ok),
            (false, false) => ("!", c.err),
        };
        let row = div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .min_h(theme.sized(36.))
            .py(theme::GAP_SMALL)
            .px(theme::GAP_LARGE)
            .child(
                label(theme, theme.body(), tone, mark.to_string())
                    .font_weight(theme.semibold)
                    .w(theme.sized(18.)),
            );
        match sentence {
            true => {
                let said = match &place {
                    Some(place) => format!("{what}, {place}"),
                    None => what,
                };
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .sized_text(theme, theme.body())
                        .text_color(if note || place.is_some() {
                            c.ink
                        } else {
                            c.err
                        })
                        .child(said),
                )
            }
            false => {
                let room = width
                    - theme::GAP_LARGE * 2.
                    - theme.sized(18.)
                    - theme.sized(220.)
                    - theme::GAP * 2.;
                let path = place.clone().unwrap_or_else(|| put(&say!("Not found")));
                let shown = elide_middle_in(
                    window,
                    theme,
                    theme.mono.clone(),
                    &path,
                    theme.small(),
                    room,
                );
                row.child(
                    div()
                        .w(theme.sized(220.))
                        .flex_none()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(what),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .sized_text(theme, theme.small())
                        .font_family(theme.mono.clone())
                        .text_color(if place.is_some() { c.ink2 } else { c.err })
                        .child(shown),
                )
            }
        }
        .into_any_element()
    }

    pub(super) fn tick(
        theme: &Theme,
        id: SharedString,
        text: String,
        detail: String,
        on: bool,
        toggled: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let c = &theme.colours;
        let size = theme.sized(15.);
        div()
            .id(id)
            .role(Role::CheckBox)
            .aria_label(text.clone())
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
            .flex()
            .items_center()
            .gap(theme::GAP)
            .min_h(theme.sized(36.))
            .py(theme::GAP_SMALL)
            .px(theme::GAP_LARGE)
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(size)
                    .rounded(px(3.))
                    .bg(if on { c.accent } else { c.well })
                    .border_1()
                    .border_color(if on { c.accent } else { c.line })
                    .when(on, |d| {
                        d.child(
                            label(theme, theme.caption(), c.accent_ink, "✓".to_string())
                                .font_weight(theme.semibold),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink)
                    .child(text)
                    .when(!detail.is_empty(), |d| {
                        d.child(div().text_color(c.ink2).child(format!(" {detail}")))
                    }),
            )
            .on_click(move |_, window, cx| toggled(window, cx))
            .into_any_element()
    }

    pub(super) fn pick(
        theme: &Theme,
        window: &Window,
        what: String,
        controls: Vec<AnyElement>,
        width: Pixels,
    ) -> AnyElement {
        let c = &theme.colours;
        let named = measure(window, theme, &what, theme.body(), FontWeight::NORMAL)
            .max(theme.sized(190.))
            .min(width / 2.);
        div()
            .flex()
            .items_center()
            .gap(theme::GAP_LARGE)
            .min_h(theme.sized(44.))
            .py(theme::GAP)
            .px(theme::GAP_LARGE)
            .child(
                div()
                    .w(named)
                    .flex_none()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink)
                    .child(what),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_1()
                    .min_w_0()
                    .gap(theme::GAP_SMALL)
                    .children(controls),
            )
            .into_any_element()
    }

    pub(super) fn glance(&self, theme: &Theme, report: &SettingsReport) -> Option<AnyElement> {
        if report.official.is_empty() {
            return None;
        }
        let c = &theme.colours;
        let comfortable = theme.appearance.comfortable;
        let version = report.game_version.clone().unwrap_or_default();
        let pictures = self.pictures.kept();
        let rows = report.official.iter().enumerate().map(|(at, one)| {
            let chosen = at == 1;
            let (ink, dim) = match chosen {
                true => (c.on_selected, c.on_selected_dim),
                false => (c.ink, c.ink2),
            };
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .h(if comfortable {
                    theme.sized(46.)
                } else {
                    theme.row()
                })
                .px(theme::GAP_MEDIUM)
                .rounded(theme::CORNER)
                .when(chosen, |d| d.bg(c.chosen))
                .child(label(
                    theme,
                    theme.small(),
                    if chosen { dim } else { c.ink3 },
                    (at + 1).to_string(),
                ))
                .child(icon(
                    theme::CATEGORY_ICONS[1],
                    theme.sized(13.),
                    if chosen {
                        ink
                    } else {
                        theme.category_colour(1)
                    },
                ))
                .when(comfortable, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .w(theme.sized(64.))
                            .h(theme.sized(40.))
                            .rounded(theme::CORNER)
                            .bg(c.ctl)
                            .overflow_hidden()
                            .p(theme::GAP_LINE)
                            .when_some(one.picture.clone(), |d, picture| {
                                d.child(
                                    pictures
                                        .img(std::path::PathBuf::from(picture))
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                            }),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(ink)
                        .child(one.name.clone()),
                )
                .child(label(theme, theme.body(), dim, one.author.clone()))
                .child(label(theme, theme.body(), dim, version.clone()))
                .into_any_element()
        });
        Some(Self::block(
            theme,
            put(&say!("As it will look")),
            vec![div().p(theme::GAP).children(rows).into_any_element()],
        ))
    }
}
