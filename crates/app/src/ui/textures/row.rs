use super::screen::Columns;
use super::{Textures, megabytes};
use crate::ui::button::{Btn, Chip};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label, with_alpha};
use crate::words::{count, put};
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, Pixels, Role,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::{say, say_n};

#[allow(clippy::too_many_arguments)]
pub(super) fn texture_row(
    theme: &Theme,
    at: usize,
    (row, said): &(rimstack_core::mods::textures::Row, String),
    usual: &str,
    selected: Option<rimstack_core::mods::library::ModId>,
    busy: bool,
    converts: bool,
    columns: &Columns,
    width: Pixels,
    me: gpui::WeakEntity<Textures>,
    window: &Window,
) -> AnyElement {
    let c = &theme.colours;
    let current = selected == Some(row.id);
    let (quiet, faint) = if current {
        (c.on_selected_dim, c.on_selected_dim)
    } else {
        (c.ink2, c.ink3)
    };
    let settled = row.not_converted == 0 && row.out_of_date == 0;
    let verb = if row.out_of_date > 0 {
        put(&say!("Rebuild"))
    } else {
        put(&say!("Convert"))
    };
    let badge = (row.state != usual).then(|| {
        let tone = if row.complete {
            c.ok
        } else if row.stale {
            c.warn
        } else {
            c.ink2
        };
        let chip = Chip::new(said.clone())
            .max_width(theme.sized(190.))
            .on_accent(current)
            .tone(tone);
        match (row.complete, row.stale) {
            (true, _) => chip.icon("check"),
            (false, true) => chip.icon("triangle-alert"),
            _ => chip,
        }
    });
    let badge_width = badge.as_ref().map_or(px(0.), |_| {
        crate::ui::widgets::chip_width(window, theme, said, row.complete || row.stale)
            .min(theme.sized(190.))
            + theme::GAP
    });
    let id = row.id;
    let hover = with_alpha(c.hover, 0.04);
    let offers = converts && row.not_converted + row.out_of_date > 0 && !row.skipped;
    let (pick, convert) = (me.clone(), me);
    let right = |text: String, colour, size, width: Pixels| {
        div()
            .w(width)
            .flex_none()
            .flex()
            .justify_end()
            .child(label(theme, size, colour, text))
    };
    div()
        .id(("texture-row", at))
        .role(Role::ListItem)
        .aria_label(format!(
            "{}, {}, {}",
            row.name,
            put(&say_n!(row.textures, "{} texture", "{} textures")),
            said
        ))
        .relative()
        .w(width)
        .h(theme.row())
        .flex()
        .items_center()
        .gap(theme::GAP_LARGE)
        .px(theme::GAP_WIDE)
        .when(settled && !current, |d| d.opacity(0.6))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(theme::GAP_SMALL)
                .right(theme::GAP_SMALL)
                .rounded(theme::CORNER)
                .when(current, |d| d.bg(c.chosen))
                .when(!current, |d| d.hover(move |s| s.bg(hover))),
        )
        .child(
            div()
                .w(columns.name)
                .flex_none()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(
                    div()
                        .max_w(columns.name - badge_width)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(if current { c.on_selected } else { c.ink })
                        .child(row.name.clone()),
                )
                .children(badge),
        )
        .child(right(
            count(row.textures),
            quiet,
            theme.body(),
            columns.count,
        ))
        .child(right(
            megabytes(row.png_mb),
            quiet,
            theme.body(),
            columns.png,
        ))
        .child(match (row.dds_mb > 0.0, converts) {
            (true, _) => right(megabytes(row.dds_mb), quiet, theme.body(), columns.dds),
            (false, false) => right(String::new(), faint, theme.small(), columns.dds),
            (false, true) => right(
                put(&say!("Not converted")),
                faint,
                theme.small(),
                columns.dds,
            ),
        })
        .when(offers, |d| {
            d.child(
                Btn::new(("convert-row", at), verb.clone())
                    .enabled(!busy)
                    .spoken(put(&say!("{}: {}", verb, row.name.clone())))
                    .on_click(move |_, _, cx| {
                        let _ = convert.update(cx, |t, cx| t.work(vec![id], false, cx));
                    }),
            )
        })
        .on_click(move |_, _, cx| {
            let _ = pick.update(cx, |t, cx| {
                t.selected = if t.selected == Some(id) {
                    None
                } else {
                    Some(id)
                };
                cx.notify();
            });
        })
        .into_any_element()
}
