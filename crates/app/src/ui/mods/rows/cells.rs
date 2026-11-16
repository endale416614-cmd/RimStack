use super::headings::tipped_label;
use super::{Around, Columns, category_icon};
use crate::ui::theme::{self, CATEGORY_NAMES, Theme};
use crate::ui::widgets::{
    Chip, TextSized, chip_width, elide_middle, icon, label, measure, with_alpha,
};
use crate::words::{count, put};
use crate::workspace::load_order::{Level, Origin, RowView};
use gpui::{
    AnyElement, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, RenderImage,
    SharedString, Styled, StyledImage, TextOverflow, Window, div, img, prelude::FluentBuilder, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::details::Mark;
use rimstack_core::mods::list::Category;
use rimstack_core::say;
use std::sync::Arc;

pub(super) enum Tags {
    None,
    All(Vec<String>),
    Folded(String),
}

pub(super) struct Fitted {
    pub name: String,
    pub cut: bool,
    pub tags: Tags,
    pub ordered_by: Option<String>,
    /// Half the name's cell, the most the value the list is ordered by takes.
    pub half: Pixels,
}

pub(super) struct Shown<'a> {
    pub x: Pixels,
    pub columns: Option<&'a Columns>,
    pub chosen: bool,
    pub alone: bool,
    pub pointed: bool,
    pub colour: Hsla,
    pub picture: Option<Arc<RenderImage>>,
    pub mark: Option<&'a Mark>,
}

/// How a name fits the row: what is beside it takes its room first, the tags only while
/// they leave the name its whole width.
pub(super) fn fit(
    window: &Window,
    theme: &Theme,
    row: &RowView,
    room: Pixels,
    columns: Option<&Columns>,
    around: &Around<'_>,
) -> Fitted {
    let half = room / 2.;
    let mut room = room;
    let load_order = columns.is_some_and(|c| c.load_order);
    let small = |text: &str| measure(window, theme, text, theme.small(), gpui::FontWeight::NORMAL);
    if row.outdated {
        room -= chip_width(window, theme, &row.made_for, false) + theme::GAP_SMALL;
    }
    if row.fresh {
        room -= chip_width(window, theme, &put(&say!("New")), false) + theme::GAP_SMALL;
    }
    if row.version_ignored {
        room -= theme.sized(12.) + theme::GAP_SMALL;
    }
    if row.twin && !load_order {
        room -= small(&put(&row.version_line)) + theme::GAP_SMALL;
    }
    if !load_order && row.source == Origin::Git {
        room -= theme.sized(12.) + theme::GAP_SMALL;
    }
    let ordered_by = (!row.ordered_by.is_empty() && !(columns.is_some() && around.order_shown))
        .then(|| put(&row.ordered_by));
    if let Some(value) = &ordered_by {
        room -= small(value).min(half) + theme::GAP_SMALL;
    }
    let full = put(&row.name);
    let wide = measure(window, theme, &full, theme.body(), gpui::FontWeight::NORMAL);
    let all: Vec<&str> = row.tags.split(", ").filter(|t| !t.is_empty()).collect();
    let most = theme.sized(110.);
    let shown: Vec<String> = all.iter().take(2).map(|t| t.to_string()).collect();
    let across = shown.iter().fold(px(0.), |sum, tag| {
        sum + chip_width(window, theme, tag, false).min(most) + theme::GAP_SMALL
    });
    let tags = if all.is_empty() {
        Tags::None
    } else if wide + across + theme::GAP * 5. < room {
        room -= across;
        Tags::All(shown)
    } else if wide + theme::GAP * 5. + theme::GAP_TINY < room {
        let headed = columns.is_some() && around.headed;
        let said = match (headed, all.len()) {
            (true, n) => count(n),
            (false, 1) => all[0].to_string(),
            (false, n) => format!("{}  +{}", all[0], n - 1),
        };
        room -= chip_width(window, theme, &said, true).min(theme.sized(120.)) + theme::GAP_SMALL;
        Tags::Folded(said)
    } else {
        Tags::None
    };
    let name = elide_middle(window, theme, &full, theme.body(), room);
    Fitted {
        cut: name != full,
        name,
        tags,
        ordered_by,
        half,
    }
}

fn category_name(category: Category) -> String {
    CATEGORY_NAMES
        .get(category as usize)
        .map(|name| put(&Sentence::said(name, Vec::new())))
        .unwrap_or_default()
}

fn tile(
    theme: &Theme,
    row: &RowView,
    colour: Hsla,
    picture: Option<Arc<RenderImage>>,
) -> AnyElement {
    let first: String = put(&row.name)
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(theme.sized(64.))
        .h(theme.sized(40.))
        .rounded(theme::CORNER)
        .bg(theme.colours.ctl)
        .map(|d| match row.picture.is_empty() {
            true => d
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(theme::CORNER)
                        .bg(with_alpha(colour, 0.16))
                        .border_1()
                        .border_color(with_alpha(colour, 0.28)),
                )
                .child(label(theme, theme.title(), colour, first).font_weight(theme.semibold)),
            false => d.children(picture.map(|picture| {
                img(picture)
                    .absolute()
                    .left(theme::GAP_LINE)
                    .top(theme::GAP_LINE)
                    .w(theme.sized(64.) - theme::GAP_LINE * 2.)
                    .h(theme.sized(40.) - theme::GAP_LINE * 2.)
                    .rounded(theme::CORNER)
                    .object_fit(gpui::ObjectFit::Cover)
            })),
        })
        .into_any_element()
}

pub(super) fn cells(theme: &Theme, row: &RowView, fitted: Fitted, shown: Shown<'_>) -> AnyElement {
    let c = &theme.colours;
    let Shown {
        x,
        columns,
        chosen,
        alone,
        pointed,
        colour,
        picture,
        mark,
    } = shown;
    let load_order = columns.is_some_and(|c| c.load_order);
    let quiet = if chosen { c.on_selected_dim } else { c.ink2 };
    let ink = if chosen { c.on_selected } else { c.ink };
    let name_text = crate::ui::widgets::watched(
        div()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_overflow(TextOverflow::TruncateMiddle("…".into()))
            .sized_text(theme, theme.body())
            .text_color(ink),
        put(&row.name).into(),
        fitted.name,
        true,
    );
    let full_name = match fitted.cut && pointed {
        true => put(&row.name),
        false => String::new(),
    };
    let name = match columns {
        Some(columns) => div().flex_none().w(columns.name_cell),
        None => div().flex_1(),
    };
    let name = name
        .flex()
        .min_w_0()
        .items_center()
        .gap(theme::GAP_SMALL)
        .map(|d| {
            let shown = icon(
                category_icon(row.category),
                theme.sized(13.),
                if chosen { c.on_selected } else { colour },
            );
            match pointed {
                true => d.child(tipped_label(
                    format!("category-{}", row.number),
                    div().child(shown),
                    category_name(row.category),
                )),
                false => d.child(shown),
            }
        })
        .when(columns.is_some_and(|c| c.tile), |d| {
            d.child(tile(theme, row, colour, picture))
        })
        .when(row.version_ignored, |d| {
            d.child(tipped_label(
                format!("ignored-{}", row.number),
                div().child(icon(
                    "eye-off",
                    theme.sized(12.),
                    if chosen { c.on_selected } else { c.ink3 },
                )),
                put(&say!(
                    "Made for another game version; the warning is ignored"
                )),
            ))
        })
        .map(|d| match full_name.is_empty() {
            true => d.child(name_text),
            false => d.child(crate::ui::tip::tipped(
                name_text.id(SharedString::from(format!("name-{}", row.number))),
                full_name.into(),
            )),
        })
        .when(row.outdated, |d| {
            d.child(
                Chip::new(row.made_for.clone())
                    .tone(c.warn)
                    .on_accent(chosen)
                    .tip(put(&say!("Made for another game version"))),
            )
        })
        .when(row.fresh, |d| {
            d.child(
                Chip::new(put(&say!("New")))
                    .tone(c.ok)
                    .on_accent(chosen)
                    .tip(put(&say!(
                        "Installed since the game was last started from RimStack"
                    ))),
            )
        })
        .map(|d| match fitted.tags {
            Tags::None => d,
            Tags::All(tags) => d.children(tags.into_iter().map(|tag| {
                Chip::new(tag)
                    .max_width(theme.sized(110.))
                    .on_accent(chosen)
            })),
            Tags::Folded(said) => d.child(
                Chip::new(said)
                    .icon("tag")
                    .max_width(theme.sized(120.))
                    .on_accent(chosen)
                    .tip(row.tags.clone()),
            ),
        })
        .when(row.twin && !load_order, |d| {
            d.child(label(theme, theme.small(), quiet, put(&row.version_line)))
        })
        .child(div().flex_1())
        .when_some(fitted.ordered_by, |d, value| {
            d.child(
                div()
                    .flex_none()
                    .max_w(fitted.half)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(theme, theme.small())
                    .text_color(quiet)
                    .child(value),
            )
        })
        .when(!load_order && row.source == Origin::Git, |d| {
            d.child(tipped_label(
                format!("git-{}", put(&row.name)),
                div().child(icon(
                    "git-branch",
                    theme.sized(12.),
                    if chosen { c.on_selected } else { c.ink3 },
                )),
                put(&say!("A Git checkout")),
            ))
        });
    let Some(columns) = columns else {
        return div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .px(theme::GAP_WIDE)
            .gap(theme::GAP_LARGE)
            .child(name)
            .into_any_element();
    };
    let value = |key: &str| -> SharedString {
        match key {
            "author" => row.author.clone().into(),
            "version" => row.version.clone().into(),
            "load" => row.load_time.clone().into(),
            "updated" => put(&row.updated_ago).into(),
            "downloaded" => put(&row.downloaded_ago).into(),
            "activated" => put(&row.activated_ago).into(),
            _ => put(&row.size_text).into(),
        }
    };
    let margin = match alone {
        true => theme.sized(26.),
        false => px(0.),
    };
    let quick = quick_chip(theme, row, columns.wide, chosen, mark)
        .map(|chip| chip.max_width(columns.quick - margin));
    let scrolled = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(-x)
        .w(columns.across)
        .flex()
        .items_center()
        .pl(theme::GAP_WIDE)
        .gap(theme::GAP_LARGE)
        .when(load_order, |d| {
            d.child(
                div()
                    .flex()
                    .flex_none()
                    .justify_end()
                    .w(columns.number)
                    .child(label(
                        theme,
                        theme.small(),
                        if chosen { c.on_selected_dim } else { c.ink3 },
                        row.number.to_string(),
                    )),
            )
        })
        .child(name)
        .children(columns.fitting.iter().map(|(key, width)| {
            let right = matches!(*key, "load" | "size");
            let colour = match (*key, chosen) {
                ("load", true) => c.on_selected,
                ("load", false) if row.load_seconds >= 0.1 => c.ink2,
                ("load", false) => c.ink3,
                _ => quiet,
            };
            let bar = *key == "load" && row.load_part > 0. && !row.load_time.is_empty();
            let cell = div()
                .id(SharedString::from(format!("cell-{key}")))
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .h_full()
                .w(*width)
                .when(right, |d| d.justify_end())
                .child({
                    let said = value(key);
                    crate::ui::widgets::watched(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, if right { theme.small() } else { theme.body() })
                            .text_color(colour),
                        said.clone(),
                        said,
                        true,
                    )
                })
                .when(bar, |d| {
                    d.child(
                        div()
                            .absolute()
                            .right_0()
                            .bottom(theme::GAP_LINE)
                            .h(px(2.))
                            .w((*width * row.load_part as f32).max(px(2.)))
                            .rounded(px(1.))
                            .bg(if chosen { c.on_selected_dim } else { c.ink3 }),
                    )
                });
            let tip = match (*key, row.load_time.is_empty() || !pointed) {
                ("load", false) => put(&say!("{} at the game's last start", row.load_time.clone())),
                _ => String::new(),
            };
            crate::ui::tip::tipped(cell, tip.into())
        }));
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .right(columns.fixed)
                .overflow_hidden()
                .child(scrolled),
        )
        .when(load_order, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(theme::GAP_WIDE)
                    .w(columns.quick)
                    .flex()
                    .items_center()
                    .justify_end()
                    .pr(margin)
                    .children(quick),
            )
        })
        .into_any_element()
}

fn quick_chip(
    theme: &Theme,
    row: &RowView,
    wide: bool,
    chosen: bool,
    mark: Option<&Mark>,
) -> Option<Chip> {
    let c = &theme.colours;
    let chip = if let Some(from) = row.moved_from {
        let up = from > row.number;
        let tip = match row.why_moved.is_empty() {
            true => say!("Sort moves it from {} to {}", from, row.number),
            false => say!(
                "Sort moves it from {} to {}: {}",
                from,
                row.number,
                row.why_moved.inside()
            ),
        };
        Chip::new(count(from.abs_diff(row.number)))
            .tone(c.moved)
            .icon(if up { "arrow-up" } else { "arrow-down" })
            .tip(put(&tip))
    } else if let Some(mark) = mark {
        let (said, tip) = match mark.loses {
            true => (
                say!("loses textures"),
                say!("The selected mod loads later and its textures are used"),
            ),
            false => (
                say!("wins textures"),
                say!("This mod loads later than the selected one and its textures are used"),
            ),
        };
        Chip::new(if wide { put(&said) } else { String::new() })
            .tone(c.warn)
            .icon("image")
            .tip(put(&tip))
    } else if row.trouble > Level::None {
        let error = row.trouble == Level::Error;
        Chip::new(if wide {
            put(&row.trouble_text)
        } else {
            String::new()
        })
        .tip(put(&row.trouble_text))
        .tone(if error { c.err } else { c.warn })
        .icon(if error {
            "circle-alert"
        } else {
            "triangle-alert"
        })
    } else {
        return None;
    };
    Some(chip.on_accent(chosen))
}
