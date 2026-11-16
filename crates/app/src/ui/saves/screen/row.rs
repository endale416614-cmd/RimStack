use crate::ui::button::Chip;

use crate::ui::saves::Saves;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, icon, label, with_alpha};
use crate::words::put;
use crate::workspace::logs::moment;
use crate::workspace::saves::SaveShown;
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, Pixels, Role,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px,
};
use rimstack_core::rimworld::saves::Verdict;
use rimstack_core::say;

/// The chips a save's row shows, each with its tone: 0 the
/// error colour, 1 the good one, 2 the warning, 3 the other game's, 4 the plain one.
pub(super) fn chips_of<'a>(save: &'a SaveShown<'_>) -> impl Iterator<Item = (Chip, usize)> + 'a {
    let row = save.row;
    let compared = row.compared.as_ref();
    let added = compared.map_or(0, |c| c.added.len());
    [
        (row.verdict == Verdict::Unreadable).then(|| {
            (
                say!("cannot be read"),
                0,
                Some("circle-alert"),
                String::new(),
            )
        }),
        (row.verdict == Verdict::Same)
            .then(|| (say!("same mods as now"), 1, Some("check"), String::new())),
        (save.not_installed > 0).then(|| {
            (
                say!("{} not installed", save.not_installed),
                0,
                Some("circle-alert"),
                put(&say!(
                    "Mods the save was played with that are not installed"
                )),
            )
        }),
        (save.not_active > 0).then(|| {
            (
                say!("{} not active", save.not_active),
                2,
                Some("minus"),
                put(&say!(
                    "Mods the save was played with that are not active now"
                )),
            )
        }),
        compared
            .filter(|c| c.other_game)
            .map(|_| (say!("other game version"), 3, None, String::new())),
        (added > 0).then(|| {
            (
                say!("{} added", added),
                4,
                None,
                put(&say!("Mods active now that the save was not played with")),
            )
        }),
        compared
            .filter(|c| c.reordered)
            .map(|_| (say!("reordered"), 4, None, String::new())),
    ]
    .into_iter()
    .flatten()
    .map(|(text, tone, icon, tip)| {
        let chip = Chip::new(put(&text)).tip(tip);
        let chip = match icon {
            Some(name) => chip.icon(name),
            None => chip,
        };
        (chip, tone)
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save_row(
    theme: &Theme,
    at: usize,
    save: &SaveShown<'_>,
    current: bool,
    (pictured, all_from): (bool, bool),
    width: Pixels,
    height: Pixels,
    thumbs: &crate::ui::thumbs::Shared,
    view: gpui::EntityId,
    me: gpui::WeakEntity<Saves>,
    window: &Window,
    cx: &mut gpui::App,
) -> AnyElement {
    let c = &theme.colours;
    let row = save.row;
    let quiet = if current { c.on_selected_dim } else { c.ink2 };
    let end = theme::GAP_PAGE + theme::GAP_SMALL;
    let hover = with_alpha(c.hover, 0.05);
    let picture = row
        .preview
        .as_deref()
        .filter(|_| pictured)
        .and_then(|preview| {
            let scale = window.scale_factor();
            let size = (
                (f32::from(theme.sized(104.)) * scale).round() as u32,
                (f32::from(theme.sized(62.)) * scale).round() as u32,
            );
            crate::ui::thumbs::thumb(thumbs, preview, size, view, cx)
        });
    let about = format!("{}   {}", moment(row.modified), put(&save.about));
    let other = if all_from { c.ink2 } else { c.warn };
    let tones = [c.err, c.ok, c.warn, other, c.ink2];
    let chips: Vec<Chip> = chips_of(save)
        .map(|(chip, tone)| chip.tone(tones[tone]).on_accent(current))
        .collect();
    div()
        .id(("save", at))
        .role(Role::ListItem)
        .aria_label(format!("{}, {}", row.name, put(&save.said)))
        .relative()
        .w(width)
        .h(height)
        .flex()
        .items_center()
        .gap(theme::GAP_LARGE)
        .pl(theme::GAP_PAGE)
        .pr(end)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(theme::GAP_LARGE)
                .right(theme::GAP_CARD)
                .rounded(theme::CORNER)
                .when(current, |d| d.bg(c.chosen))
                .when(!current, |d| d.hover(move |s| s.bg(hover))),
        )
        .when(!current, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom(px(-1.))
                    .left(theme::GAP_PAGE)
                    .right(end)
                    .h(px(1.))
                    .bg(with_alpha(c.line, 0.6)),
            )
        })
        .when(pictured, |d| {
            d.child(
                div()
                    .relative()
                    .flex_none()
                    .w(theme.sized(104.))
                    .h(theme.sized(62.))
                    .rounded(theme::CORNER)
                    .bg(c.card)
                    .overflow_hidden()
                    .when(row.preview.is_none(), |d| {
                        d.border_1()
                            .border_color(c.line)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("save", theme.sized(22.), c.ink3))
                    })
                    .children(picture.map(|picture| img(picture).absolute().inset_0().size_full())),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(theme::GAP_TINY)
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.title())
                        .font_weight(theme.semibold)
                        .text_color(if current { c.on_selected } else { c.ink })
                        .child(row.name.clone()),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.small())
                        .text_color(quiet)
                        .child(about),
                )
                .when(!chips.is_empty(), |d| {
                    d.child(div().flex().gap(theme::GAP_TINY).children(chips))
                }),
        )
        .when(row.megabytes > 0.0, |d| {
            d.child(label(
                theme,
                theme.small(),
                quiet,
                put(&say!("{} MB", format!("{:.1}", row.megabytes))),
            ))
        })
        .on_click(move |_, _, cx| {
            let _ = me.update(cx, |saves, cx| {
                saves.selected = at;
                cx.notify();
            });
        })
        .into_any_element()
}
