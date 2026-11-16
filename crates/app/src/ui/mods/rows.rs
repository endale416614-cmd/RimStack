mod cells;
mod headings;

use super::Mods;
use crate::ui::theme::{self, CATEGORY_ICONS, Theme};
use crate::ui::thumbs;
use crate::ui::widgets::{TextSized, halo, icon, with_alpha};
use crate::words::put;
use crate::workspace::load_order::{RowView, Side};
use gpui::AppContext;
use gpui::{
    AnyElement, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Category;
use rimstack_core::say;

pub struct Columns {
    /// The load order's: a number before the name and the quick chip after the columns.
    pub load_order: bool,
    pub number: Pixels,
    /// The name's text, and its whole cell with the category's icon and the picture.
    pub name_room: Pixels,
    pub name_cell: Pixels,
    pub fitting: Vec<(&'static str, Pixels)>,
    pub quick: Pixels,
    pub wide: bool,
    pub tile: bool,
    /// The cells that scroll sideways from end to end, and the room they have: wider, the
    /// list scrolls sideways.
    pub across: Pixels,
    pub room: Pixels,
    /// What stays at the row's right as the rest scrolls: the quick chip under the View
    /// button, with the row's padding.
    pub fixed: Pixels,
    /// The room the name and the columns share, the least each column but the name may be
    /// dragged to, and what the last column, filling the row, can give a column dragged wider.
    pub cells: Pixels,
    pub leasts: Vec<Pixels>,
    pub give: Pixels,
}

pub fn height(theme: &Theme, row: &RowView, columns: bool) -> Pixels {
    if row.is_sub {
        theme.sized(24.)
    } else if row.again {
        theme.sized(22.)
    } else if row.is_header {
        theme.sized(32.)
    } else if columns && theme.appearance.comfortable {
        theme.sized(52.).max(theme.row() + theme::GAP_MEDIUM)
    } else {
        theme.row()
    }
}

fn category_colour(theme: &Theme, category: Category) -> Hsla {
    theme.category_colour(category as usize)
}

fn category_icon(category: Category) -> &'static str {
    CATEGORY_ICONS
        .get(category as usize)
        .copied()
        .unwrap_or("package")
}

#[derive(Clone)]
pub struct Held {
    pub side: Side,
    pub index: usize,
    pub ids: Vec<rimstack_core::mods::library::ModId>,
}

/// What follows the pointer while mods are dragged: what is held,
/// beside the pointer. `grabbed` is where the row was taken, from the row's corner, which GPUI
/// draws this at.
pub struct HeldGhost {
    pub name: String,
    pub grabbed: gpui::Point<Pixels>,
}

impl gpui::Render for HeldGhost {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let tall = theme.sized(26.);
        div()
            .ml(self.grabbed.x + theme::GAP_MEDIUM)
            .mt(self.grabbed.y - tall / 2.)
            .w(theme.sized(300.))
            .h(tall)
            .px(theme::GAP_MEDIUM)
            .flex()
            .items_center()
            .rounded(theme::CORNER)
            .bg(theme.colours.accent)
            .opacity(0.9)
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(&theme, theme.body())
                    .text_color(gpui::white())
                    .child(self.name.clone()),
            )
    }
}

pub struct Around<'a> {
    /// How far the columns are scrolled sideways.
    pub x: Pixels,
    pub movable: bool,
    pub chosen_ids: &'a [ModId],
    pub marks: &'a Marks,
    pub single: bool,
    pub buttons: bool,
    pub previewing: bool,
    pub headed: bool,
    pub order_shown: bool,
    /// The row the pointer is on: only it has its buttons and its tips made.
    pub pointed: Option<usize>,
}

pub type Marks = std::collections::BTreeMap<
    rimstack_core::mods::library::ModId,
    rimstack_core::mods::details::Mark,
>;

fn hover_buttons(
    theme: &Theme,
    side: Side,
    at: usize,
    row: &RowView,
    pointed: bool,
    mods: &WeakEntity<Mods>,
) -> AnyElement {
    let c = &theme.colours;
    let size = theme.sized(24.);
    let (mod_id, index) = (row.mod_id, row.mod_index);
    let tall = height(theme, row, side == Side::Active);
    let drawn: std::rc::Rc<std::cell::Cell<gpui::Bounds<Pixels>>> = Default::default();
    let placed = drawn.clone();
    let button = |key: &'static str, name: &'static str, tip: String| {
        let shown = div()
            .id((key, at))
            .flex()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(theme::CORNER)
            .bg(c.ctl)
            .hover(|d| d.bg(c.ctl_hi))
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(tip.clone())
            .relative()
            .child(icon(name, theme.sized(13.), c.ink));
        crate::ui::tip::tipped(shown, tip.into())
    };
    let (across, folder, menu) = (mods.clone(), mods.clone(), mods.clone());
    let active = side == Side::Active;
    div()
        .absolute()
        .right(theme::GAP_CARD)
        .top_0()
        .bottom_0()
        .flex()
        .items_center()
        .gap(theme::GAP_HAIR)
        .when(pointed, |d| {
            d.child(
                button(
                    "row-across",
                    if active { "minus" } else { "plus" },
                    match active {
                        true => put(&say!("Deactivate")),
                        false => put(&say!("Activate")),
                    },
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    if let Some(id) = mod_id {
                        let _ = across.update(cx, |mods, cx| mods.toggle_mod(side, id, cx));
                    }
                }),
            )
        })
        .when_some(mod_id.filter(|_| pointed), |d, mod_id| {
            d.child(
                button("row-folder", "folder-open", put(&say!("Open folder"))).on_click(
                    move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = folder.update(cx, |mods, cx| mods.open_folder(mod_id, cx));
                    },
                ),
            )
        })
        .child(
            button("row-menu", "ellipsis", put(&say!("Menu (right-click)")))
                .child(
                    gpui::canvas(move |bounds, _, _| drawn.set(bounds), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let button = placed.get();
                    let at = gpui::point(button.left(), button.center().y + tall / 2.);
                    let _ = menu.update(cx, |mods, cx| {
                        mods.row_menu(side, index, mod_id, at, window, cx)
                    });
                }),
        )
        .into_any_element()
}

pub fn tile_size(window: &Window, theme: &Theme) -> (u32, u32) {
    let scale = window.scale_factor();
    let device = |logical: f32| f32::from(theme.sized(logical) - theme::GAP_LINE * 2.) * scale;
    (device(64.).round() as u32, device(40.).round() as u32)
}

/// The previews of the rows a page and a half above and below `range`, decoded before they are
/// scrolled to: a page of headings holds more rows than one of mods.
pub fn ahead(
    window: &Window,
    theme: &Theme,
    rows: &[RowView],
    range: &std::ops::Range<usize>,
    thumbs: &thumbs::Shared,
    cx: &mut gpui::App,
) {
    let page = range.len().max(1) * 3 / 2;
    let size = tile_size(window, theme);
    let above = range.start.saturating_sub(page)..range.start;
    let below = range.end.min(rows.len())..(range.end + page).min(rows.len());
    for at in below.chain(above.rev()) {
        let row = &rows[at];
        if !row.is_header && !row.picture.is_empty() {
            thumbs::ahead(thumbs, &row.picture, size, cx);
        }
    }
}

pub fn while_idle(
    window: &Window,
    theme: &Theme,
    rows: &std::rc::Rc<Vec<RowView>>,
    thumbs: &thumbs::Shared,
    cx: &mut gpui::App,
) {
    let pictures = rows
        .iter()
        .filter(|row| !row.is_header && !row.picture.is_empty())
        .map(|row| row.picture.as_str());
    let id = std::rc::Rc::as_ptr(rows) as usize;
    thumbs::while_idle(thumbs, id, pictures, tile_size(window, theme), cx);
}

#[allow(clippy::too_many_arguments)]
pub fn row(
    window: &Window,
    theme: &Theme,
    name_room: Pixels,
    side: Side,
    at: usize,
    row: &RowView,
    columns: Option<&Columns>,
    chosen: bool,
    around: &Around<'_>,
    mods: WeakEntity<Mods>,
    thumbs: &thumbs::Shared,
    shown_by: gpui::EntityId,
    cx: &mut gpui::App,
) -> AnyElement {
    let c = &theme.colours;
    let colour = category_colour(theme, row.category);
    let big_header = row.is_header && !row.is_sub && !row.again;
    let mark = row
        .mod_id
        .and_then(|id| around.marks.get(&id))
        .filter(|_| columns.is_some() && !row.is_header);
    let pointed = around.pointed == Some(at);
    let body = if row.is_sub {
        headings::sub(theme, row, side, at, pointed, mods.clone())
    } else if row.again {
        headings::again(theme, row)
    } else if row.is_header {
        headings::heading(theme, row, colour)
    } else {
        let fitted = cells::fit(window, theme, row, name_room, columns, around);
        let picture = match columns.is_some_and(|c| c.tile) && !row.picture.is_empty() {
            true => thumbs::thumb(thumbs, &row.picture, tile_size(window, theme), shown_by, cx),
            false => None,
        };
        let shown = cells::Shown {
            x: around.x,
            columns,
            chosen,
            alone: chosen && around.single,
            pointed,
            colour,
            picture,
            mark,
        };
        cells::cells(theme, row, fitted, shown)
    };
    let (is_header, is_sub, category, mod_id) =
        (row.is_header, row.is_sub, row.category, row.mod_id);
    let (mods_for_menu, at_row, mod_index) = (mods.clone(), at, row.mod_index);
    let previewing = around.previewing;
    let ground = if chosen {
        Some(c.chosen)
    } else if mark.is_some() {
        Some(with_alpha(c.warn, 0.12))
    } else if row.moved_from.is_some() {
        Some(with_alpha(c.moved, 0.1))
    } else {
        None
    };
    div()
        .id((
            match side {
                Side::Active => "active-row",
                Side::Inactive => "inactive-row",
            },
            at,
        ))
        .group("mod-row")
        .role(match is_header {
            true => gpui::Role::Heading,
            false => gpui::Role::ListItem,
        })
        .aria_label(put(&row.accessible_name))
        .when(!is_header, |d| d.aria_selected(chosen))
        .relative()
        .w_full()
        .h(height(theme, row, side == Side::Active))
        .child(
            div()
                .absolute()
                .left(px(2.))
                .top(if big_header {
                    theme::GAP_MEDIUM
                } else {
                    px(0.)
                })
                .bottom_0()
                .w(px(3.))
                .when(big_header, |d| d.rounded(px(1.5)))
                .bg(if row.again {
                    with_alpha(colour, 0.55)
                } else {
                    colour
                }),
        )
        .when(row.outdated && !chosen, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(theme::GAP)
                    .right(theme::GAP_SMALL)
                    .bg(with_alpha(c.warn, 0.06))
                    .child(div().w(px(2.)).h_full().bg(c.warn)),
            )
        })
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(theme::GAP)
                .right(theme::GAP_SMALL)
                .rounded(theme::CORNER)
                .when_some(ground, |d, ground| d.bg(ground))
                .when(ground.is_none() && !is_header, |d| {
                    let hover = with_alpha(c.hover, 0.04);
                    d.group_hover("mod-row", move |s| s.bg(hover))
                }),
        )
        .when(chosen, |d| {
            d.child(
                div()
                    .absolute()
                    .left(theme::GAP)
                    .top(theme::GAP_TINY)
                    .bottom(theme::GAP_TINY)
                    .w(px(3.))
                    .rounded(px(1.5))
                    .bg(c.focus)
                    .children(halo(c.focus, px(1.5))),
            )
        })
        .child(body)
        .when(
            !is_header
                && row.mod_index.is_some()
                && (pointed || chosen && around.single)
                && around.buttons
                && !cx.has_active_drag(),
            |d| d.child(hover_buttons(theme, side, at, row, pointed, &mods)),
        )
        .when(!is_header, |d| {
            let landing = mods.clone();
            let index = row.mod_index.unwrap_or(0);
            d.child(
                div()
                    .absolute()
                    .left(theme::GAP_SMALL)
                    .right(theme::GAP_SMALL)
                    .top(px(-1.5))
                    .h(px(3.))
                    .rounded(px(1.5))
                    .bg(c.focus)
                    .child(
                        div()
                            .absolute()
                            .left(px(-3.))
                            .top(px(-3.))
                            .size(px(9.))
                            .rounded_full()
                            .bg(c.focus),
                    )
                    .invisible()
                    .group_drag_over::<Held>("mod-row", |s| s.visible()),
            )
            .on_drop(move |held: &Held, _, cx| {
                let held = held.clone();
                let _ = landing.update(cx, |mods, cx| {
                    mods.drop_on((held.side, held.index, held.ids), (side, index), cx)
                });
            })
        })
        .when_some(
            row.mod_index.filter(|_| around.movable && !is_header),
            |d, mod_index| {
                let ids = match chosen && around.chosen_ids.len() > 1 {
                    true => around.chosen_ids.to_vec(),
                    false => mod_id.into_iter().collect(),
                };
                let name = match ids.len() > 1 {
                    true => put(&rimstack_core::say_n!(ids.len(), "{} mod", "{} mods")),
                    false => put(&row.name),
                };
                let held = Held {
                    side,
                    index: mod_index,
                    ids,
                };
                d.on_drag(held, move |_, grabbed, _, cx| {
                    cx.new(|_| HeldGhost {
                        name: name.clone(),
                        grabbed,
                    })
                })
            },
        )
        .on_click(move |event, _, cx| {
            let (modifiers, command) = (event.modifiers(), event.modifiers().secondary(cx));
            let twice = event.click_count() >= 2 && !previewing;
            let active = side == Side::Active;
            let _ = mods.update(cx, |mods, cx| match (is_header, is_sub) {
                (true, true) => mods.toggle_sub(side, at, cx),
                (true, false) => mods.toggle_category(side, category, cx),
                (false, _) => match mod_id {
                    Some(id) if twice => mods.toggle_mod(side, id, cx),
                    Some(id) if command => mods.select_also(id, active, cx),
                    Some(id) if modifiers.shift => mods.select_through(id, active, cx),
                    Some(id) => mods.choose(side, id, cx),
                    None if twice || command => {}
                    None => mods.select(None, active, cx),
                },
            });
        })
        .on_mouse_down(gpui::MouseButton::Right, {
            let menu = mods_for_menu;
            move |event, window, cx| {
                let at = event.position;
                let _ = menu.update(cx, |mods, cx| match (is_header, is_sub) {
                    (true, true) => mods.sub_menu(side, at_row, at, window, cx),
                    (true, false) => mods.category_menu(category, at, window, cx),
                    (false, _) => mods.row_menu(side, mod_index, mod_id, at, window, cx),
                });
            }
        })
        .into_any_element()
}
