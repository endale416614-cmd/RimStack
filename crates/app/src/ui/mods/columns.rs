use super::Mods;
use super::list::bar_room;
use super::rows::Columns;
use crate::ui::menu::{self, Entry};
use crate::ui::notices;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{btn_width, measure};
use crate::words::put;
use crate::workspace::ModsView;
use crate::workspace::load_order::{OrderEntry, Orders, Side};
use gpui::{
    AppContext, Bounds, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::columns as kept;
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default, Clone)]
pub struct Placed {
    pub view: Rc<Cell<Bounds<Pixels>>>,
    pub order: Rc<Cell<Bounds<Pixels>>>,
}

pub(super) fn placed(control: impl IntoElement, at: Rc<Cell<Bounds<Pixels>>>) -> gpui::Div {
    div().relative().child(control).child(
        canvas(move |bounds, _, _| at.set(bounds), |_, _, _, _| {})
            .absolute()
            .inset_0(),
    )
}

/// What Tab reaches beside the lists, by name: a column's heading, a divider. Each keeps its
/// keys' handle and where it was last drawn, which a menu opened by a key is put under.
pub(super) type Reachable =
    std::collections::HashMap<&'static str, (gpui::FocusHandle, Rc<Cell<Bounds<Pixels>>>)>;

#[derive(Clone, Copy)]
pub struct ColumnEdge;

#[derive(Clone, Copy)]
pub struct ColumnHeld {
    pub side: Side,
    pub key: &'static str,
}

/// An edge taken: where, which column's, and the name and the columns but the last as drawn then,
/// with the room they share with the last, what it may shrink to and what the last can give.
#[derive(Clone)]
pub struct EdgeDrag {
    pub side: Side,
    pub from: Pixels,
    pub key: &'static str,
    pub drawn: Vec<(&'static str, f64)>,
    pub room: f64,
    pub least: f64,
    pub give: f64,
    pub sizes: String,
}

/// The edge right of `key`'s cell (a column, or "name"), over the gap after it: dragged, that one
/// changes width and the last column gives or takes it; clicked twice, it takes its natural width
/// again.
pub(super) fn edge(
    theme: &Theme,
    side: Side,
    key: &'static str,
    columns: &Columns,
    cx: &mut Context<Mods>,
) -> gpui::Stateful<gpui::Div> {
    let c = &theme.colours;
    let as_f64 = |width: Pixels| f64::from(f32::from(width));
    let others = columns.fitting.len().saturating_sub(1);
    let drawn: Vec<(&'static str, f64)> = std::iter::once(("name", as_f64(columns.name_room)))
        .chain(
            columns.fitting[..others]
                .iter()
                .map(|(k, w)| (*k, as_f64(*w))),
        )
        .collect();
    let least = match columns.fitting.iter().position(|(k, _)| *k == key) {
        Some(at) => as_f64(columns.leasts[at]),
        None => as_f64(name_least(theme)),
    };
    let (room, give) = (as_f64(columns.cells), as_f64(columns.give));
    let group = SharedString::from(format!("edge-{}-{key}", side as usize));
    let focus = c.focus;
    let tip = put(&say!("Drag to size the column"));
    crate::ui::tip::tipped(
        div()
            .id(group.clone())
            .group(group.clone())
            .absolute()
            .right(-theme::GAP_LARGE)
            .top_0()
            .bottom_0()
            .w(theme::GAP_LARGE)
            .flex()
            .justify_center()
            .cursor_col_resize()
            .child(
                div()
                    .mt(px(2.))
                    .mb(px(2.))
                    .w(px(1.))
                    .h_full()
                    .bg(c.field_line)
                    .group_hover(group, move |s| s.w(px(2.)).bg(focus)),
            )
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |mods, event: &gpui::MouseDownEvent, _, cx| {
                    mods.layout.edge_drag = Some(EdgeDrag {
                        side,
                        from: event.position.x,
                        key,
                        drawn: drawn.clone(),
                        room,
                        least,
                        give,
                        sizes: mods.view.columns_of(side).1.to_string(),
                    });
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |mods, event: &gpui::ClickEvent, _, cx| {
                cx.stop_propagation();
                if event.click_count() >= 2 {
                    mods.natural_width(side, key, cx);
                }
            }))
            .on_drag(ColumnEdge, |_, _, _, cx| {
                cx.new(|_| crate::ui::divider::Ghost)
            }),
        tip.into(),
    )
}

const ALL: [&str; 7] = [
    "author",
    "version",
    "load",
    "updated",
    "downloaded",
    "activated",
    "size",
];

/// The columns a list may show: an inactive mod has no load time nor date it was activated.
fn offered(side: Side) -> impl Iterator<Item = &'static str> {
    ALL.into_iter()
        .filter(move |key| side == Side::Active || !matches!(*key, "load" | "activated"))
}

/// The columns chosen for a list, in their order; Load only once the game's log timed the mods.
pub(super) fn chosen(view: &ModsView, orders: &Orders, side: Side) -> Vec<&'static str> {
    let timed = orders.active_by.contains_key("load");
    kept::chosen(view.columns_of(side).0)
        .into_iter()
        .filter_map(|key| offered(side).find(|known| *known == key))
        .filter(|key| *key != "load" || timed)
        .collect()
}

pub(super) fn column_name(key: &str) -> Sentence {
    match key {
        "author" => say!("Author"),
        "version" => say!("Version"),
        "load" => say!("Load"),
        "updated" => say!("Updated"),
        "downloaded" => say!("Downloaded"),
        "activated" => say!("Activated"),
        _ => say!("Size"),
    }
}

fn ago_widest(window: &Window, theme: &Theme) -> Pixels {
    [
        put(&say_n!(59, "{} minute ago", "{} minutes ago")),
        put(&say_n!(47, "{} hour ago", "{} hours ago")),
        put(&say_n!(59, "{} day ago", "{} days ago")),
        put(&say_n!(11, "{} month ago", "{} months ago")),
        put(&say_n!(99, "{} year ago", "{} years ago")),
    ]
    .iter()
    .map(|text| measure(window, theme, text, theme.body(), FontWeight::NORMAL))
    .fold(px(0.), Pixels::max)
}

/// The least room the name gives way to as the list narrows: past it, the list scrolls sideways.
/// The name is as wide as its longest name, or as kept, and the last column fills the row, down
/// to its contents' width; past that, the name gives way.
fn name_least(theme: &Theme) -> Pixels {
    theme.sized(160.)
}

pub(super) fn columns(
    theme: &Theme,
    orders: &Orders,
    view: &ModsView,
    side: Side,
    width: Pixels,
    window: &Window,
) -> Columns {
    let load_order = side == Side::Active;
    let heading = |key: &str| {
        let text = format!("{} ▾", put(&column_name(key)));
        measure(window, theme, &text, theme.caption(), theme.medium)
    };
    let widest = |key: &str, size: Pixels| {
        let text = orders.widest(side).get(key).cloned().unwrap_or_default();
        measure(window, theme, &text, size, FontWeight::NORMAL)
    };
    let natural = |key: &str| match key {
        "author" | "version" => heading(key).max(widest(key, theme.body())),
        "load" => heading(key).max(widest(key, theme.small())),
        "size" => heading(key).max(measure(
            window,
            theme,
            &put(&say!("{} MB", 999)),
            theme.small(),
            FontWeight::NORMAL,
        )),
        _ => heading(key).max(ago_widest(window, theme)),
    };
    let shared = kept::shares(view.columns_of(side).1);
    let keys = chosen(view, orders, side);
    let shown = width - bar_room(theme);
    let wide = load_order && width >= theme.sized(620.);
    let view_button = btn_width(window, theme, &put(&say!("View")), false, true);
    let (number, quick) = match (load_order, wide) {
        (true, true) => (theme.sized(28.), view_button.max(theme.sized(124.))),
        (true, false) => (theme.sized(28.), view_button.max(theme.sized(44.))),
        (false, _) => (px(0.), px(0.)),
    };
    let tile = load_order && theme.appearance.comfortable && shown >= theme.sized(420.);
    let beside_name = theme.sized(13.)
        + theme::GAP_SMALL * 2.
        + match tile {
            true => theme.sized(64.) + theme::GAP_SMALL,
            false => px(0.),
        };
    let (around, fixed) = match load_order {
        true => (number + theme::GAP_LARGE, theme::GAP_WIDE + quick),
        false => (px(0.), px(0.)),
    };
    let room = match load_order {
        true => shown - fixed - theme::GAP_LARGE,
        false => shown - theme::GAP_WIDE,
    };
    let beside = theme::GAP_WIDE + around + beside_name + theme::GAP_LARGE * keys.len() as f32;
    let cells = room - beside;
    let as_f64 = |width: Pixels| f64::from(f32::from(width));
    let name_natural = widest("name", theme.body()).max(name_least(theme));
    let naturals: Vec<f64> = std::iter::once(name_natural)
        .chain(keys.iter().map(|key| natural(key)))
        .map(as_f64)
        .collect();
    let leasts: Vec<Pixels> = keys.iter().map(|key| heading(key)).collect();
    let least_all: Vec<f64> = std::iter::once(name_least(theme))
        .chain(leasts.iter().copied())
        .map(as_f64)
        .collect();
    let wanted: Vec<Option<f64>> = std::iter::once("name")
        .chain(keys.iter().copied())
        .map(|key| shared.get(key).copied())
        .collect();
    let laid = kept::laid(as_f64(cells), &wanted, &naturals, &least_all);
    let drawn: Vec<Pixels> = laid.widths.iter().map(|width| px(*width as f32)).collect();
    let name_room = drawn[0];
    let fitting: Vec<(&'static str, Pixels)> =
        keys.into_iter().zip(drawn[1..].iter().copied()).collect();
    let across = drawn.iter().fold(beside, |all, width| all + *width);
    Columns {
        load_order,
        number,
        name_room,
        name_cell: name_room + beside_name,
        fitting,
        quick,
        wide,
        tile,
        across,
        room,
        fixed,
        cells,
        leasts,
        give: px(laid.give as f32),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    View,
    InactiveOrder,
}

impl Mods {
    /// The menu under its button; false while the button is not drawn yet.
    pub fn open_menu(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let at = match menu {
            Menu::View => self.layout.menu_buttons.view.get(),
            Menu::InactiveOrder => self.layout.menu_buttons.order.get(),
        };
        if at.size.width <= px(0.) {
            return false;
        }
        match menu {
            Menu::View => self.view_menu(at, window, cx),
            Menu::InactiveOrder => self.inactive_order_menu(at, window, cx),
        }
        true
    }

    pub(super) fn keep_view(&mut self, change: impl FnOnce(&mut ModsView), cx: &mut Context<Self>) {
        let before = (*self.view).clone();
        change(Rc::make_mut(&mut self.view));
        if self.demo || *self.view == before {
            return;
        }
        if let Err(problem) = super::super::app(cx).keep_mods_view(&self.view) {
            notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
        }
    }

    /// The view changed now and kept once the dragging that changes it rests.
    pub(super) fn keep_view_soon(
        &mut self,
        change: impl FnOnce(&mut ModsView),
        cx: &mut Context<Self>,
    ) {
        change(Rc::make_mut(&mut self.view));
        if self.demo {
            return;
        }
        self.view_kept
            .after(crate::ui::divider::SETTLED, cx, |mods, cx| {
                if let Err(problem) = super::super::app(cx).keep_mods_view(&mods.view) {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
                }
            });
    }

    pub(super) fn set_headings(&mut self, headings: bool, cx: &mut Context<Self>) {
        self.keep_view(|view| view.headings = headings, cx);
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.set_headings(&app, headings));
        self.show(cx);
    }

    /// The load order shown by a column, as `then` keeps it; "" for the load order itself.
    pub(super) fn order_active(&mut self, then: &str, cx: &mut Context<Self>) {
        self.edit_list(cx, |list, _| {
            list.order.set_active_by(kept::from_kept(then))
        });
        self.show(cx);
    }

    pub(super) fn order_inactive(&mut self, then: &str, cx: &mut Context<Self>) {
        let then = then.to_string();
        let order = kept::from_kept(&then);
        self.keep_view(|view| view.inactive_order = then, cx);
        let app = super::super::app(cx);
        self.edit_list(cx, |list, _| list.order.set_inactive_by(&app, order));
        self.show(cx);
    }

    /// The edge taken last as dragged to `x`, from where it was taken.
    pub(super) fn size_column(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(drag) = self.layout.edge_drag.clone() else {
            return;
        };
        let by = f64::from(f32::from(x - drag.from));
        self.keep_view_soon(
            |view| {
                *view.columns_of_mut(drag.side).1 = kept::resized(
                    &drag.sizes,
                    &drag.drawn,
                    drag.room,
                    drag.key,
                    by,
                    drag.least,
                    drag.give,
                );
            },
            cx,
        );
        self.sync_lists(cx);
        cx.notify();
    }

    pub(super) fn natural_width(&mut self, side: Side, key: &str, cx: &mut Context<Self>) {
        self.keep_view(
            |view| {
                let sizes = view.columns_of_mut(side).1;
                *sizes = kept::natural(sizes, key);
            },
            cx,
        );
        self.sync_lists(cx);
        cx.notify();
    }

    /// A heading dropped at `x`: its column goes among the others where it was let go.
    pub(super) fn drop_column(&mut self, held: ColumnHeld, x: Pixels, cx: &mut Context<Self>) {
        let side = held.side;
        let before = self.layout.shown[side as usize]
            .iter()
            .filter(|key| **key != held.key)
            .filter(|key| {
                let drawn = self
                    .keys
                    .get(heading_reach(side, key))
                    .map(|(_, at)| at.get());
                drawn.is_some_and(|at| at.center().x < x)
            })
            .count();
        let at = kept::chosen(self.view.columns_of(side).0)
            .iter()
            .position(|shown| *shown == held.key);
        if let Some(at) = at.filter(|at| *at != before) {
            self.move_column(side, held.key, before as i32 - at as i32, cx);
        }
    }

    pub(super) fn toggle_column(
        &mut self,
        side: Side,
        column: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.keep_view(
            |view| {
                let chosen = view.columns_of_mut(side).0;
                *chosen = kept::toggled(chosen, column);
            },
            cx,
        );
        self.sync_lists(cx);
        cx.notify();
    }

    pub(super) fn heading_pressed(&mut self, side: Side, column: &str, cx: &mut Context<Self>) {
        let then = self
            .orders
            .by(side)
            .get(column)
            .map(|entry| entry.then.clone())
            .unwrap_or_default();
        match side {
            Side::Active => self.order_active(&then, cx),
            Side::Inactive => self.order_inactive(&then, cx),
        }
    }

    fn order_entries(
        &self,
        entries: &[OrderEntry],
        run: fn(&mut Mods, &str, &mut Context<Mods>),
        cx: &Context<Self>,
    ) -> Vec<Entry> {
        let me = cx.entity().downgrade();
        entries
            .iter()
            .map(|entry| {
                let (me, then) = (me.clone(), entry.then.clone());
                Entry::item(put(&entry.label), move |_, cx| {
                    let _ = me.update(cx, |mods, cx| run(mods, &then, cx));
                })
                .icon(entry.icon)
                .into()
            })
            .collect()
    }

    /// A list's columns, by name; `noted` puts how load times are had under Load, as a menu
    /// shows it.
    fn column_entries(&self, side: Side, noted: bool, cx: &Context<Self>) -> Vec<Entry> {
        let me = cx.entity().downgrade();
        let chosen = kept::chosen(self.view.columns_of(side).0);
        let how = self.load_times_how(cx);
        let mut named: Vec<(&'static str, String)> = offered(side)
            .map(|column| (column, put(&column_name(column))))
            .collect();
        named.sort_by(|a, b| a.1.cmp(&b.1));
        named
            .into_iter()
            .map(|(column, name)| {
                let me = me.clone();
                let item = Entry::item(name, move |_, cx| {
                    let _ = me.update(cx, |mods, cx| mods.toggle_column(side, column, cx));
                })
                .icon(if chosen.contains(&column) {
                    "check"
                } else {
                    ""
                });
                match (column, &how, noted) {
                    ("load", Some(how), true) => item.note(how.clone()).into(),
                    ("load", Some(how), false) => item.tip(how.clone()).into(),
                    _ => item.into(),
                }
            })
            .collect()
    }

    fn load_times_how(&self, cx: &gpui::App) -> Option<String> {
        self.list
            .read(cx)
            .overview
            .as_ref()
            .filter(|o| !o.load_times)
            .map(|o| put(&o.load_times_how))
    }

    pub(super) fn view_menu(
        &self,
        below: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity().downgrade();
        let headings = |on: bool, text: Sentence, checked: bool| -> Entry {
            let me = me.clone();
            Entry::item(put(&text), move |_, cx| {
                let _ = me.update(cx, |mods, cx| mods.set_headings(on, cx));
            })
            .icon(if checked { "check" } else { "" })
            .into()
        };
        let mut entries = vec![
            Entry::Title(put(&say!("Load order")).into()),
            headings(true, say!("Under category headings"), self.view.headings),
            headings(false, say!("One numbered list"), !self.view.headings),
            Entry::Title(put(&say!("Order")).into()),
        ];
        entries.extend(self.order_entries(&self.orders.active, Mods::order_active, cx));
        entries.push(Entry::Title(put(&say!("Columns")).into()));
        entries.extend(self.column_entries(Side::Active, false, cx));
        if let Some(how) = self.load_times_how(cx) {
            entries.push(Entry::Note(how.into()));
        }
        entries.push(Entry::Note(
            put(&say!(
                "A heading orders the list when clicked and moves when dragged."
            ))
            .into(),
        ));
        menu::open_buttons_under_end(below, Some(40), entries, window, cx);
    }

    pub(super) fn columns_menu(
        &self,
        side: Side,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        menu::open_at(at, self.column_entries(side, true, cx), window, cx);
    }

    pub(super) fn inactive_order_menu(
        &self,
        below: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut entries = vec![Entry::Title(put(&say!("Order")).into())];
        entries.extend(self.order_entries(&self.orders.inactive, Mods::order_inactive, cx));
        entries.push(Entry::Title(put(&say!("Columns")).into()));
        entries.extend(self.column_entries(Side::Inactive, false, cx));
        menu::open_buttons_under_end(below, None, entries, window, cx);
    }

    pub(super) fn activate_fresh(&mut self, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        let list = self.list.read(cx);
        let fresh: Vec<ModId> = list.order.fresh(&app, Side::Inactive);
        if fresh.is_empty() || list.order.previewing() {
            return;
        }
        self.edit_list(cx, |list, _| {
            list.order.take_all(&app, Side::Inactive, &fresh, None)
        });
        self.changed(cx);
        self.done_said(
            say_n!(fresh.len(), "Activated {} new mod", "Activated {} new mods"),
            cx,
        );
        self.filter_by("", cx);
    }
}

impl Mods {
    pub(super) fn reachable(
        &mut self,
        name: &'static str,
        cx: &mut Context<Self>,
    ) -> (gpui::FocusHandle, Rc<Cell<Bounds<Pixels>>>) {
        self.keys
            .entry(name)
            .or_insert_with(|| (cx.focus_handle().tab_stop(true), Rc::default()))
            .clone()
    }

    /// A heading's keys: the menu key opens the columns' menu,
    /// Ctrl + Alt + Left or Right moves the column. Enter and Space press it, as GPUI presses a
    /// control that holds the keys.
    pub(super) fn heading_key(
        &mut self,
        side: Side,
        column: &'static str,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = &event.keystroke;
        let held = keys.modifiers;
        match keys.key.as_str() {
            "menu" | "f10" if keys.key == "menu" || held.shift => {
                let drawn = self.reachable(heading_reach(side, column), cx).1.get();
                let at = gpui::point(drawn.left(), drawn.bottom() + px(2.));
                self.columns_menu(side, at, window, cx);
            }
            "left" | "right" if held.control && held.alt => {
                let by = if keys.key == "right" { 1 } else { -1 };
                self.move_column(side, column, by, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// The name a heading's keys are reached by: the load order's by its column, the inactive
/// mods' apart from them.
pub(super) fn heading_reach(side: Side, column: &'static str) -> &'static str {
    match (side, column) {
        (Side::Active, _) => column,
        (Side::Inactive, "name") => "inactive-name",
        (Side::Inactive, "author") => "inactive-author",
        (Side::Inactive, "version") => "inactive-version",
        (Side::Inactive, "updated") => "inactive-updated",
        (Side::Inactive, "downloaded") => "inactive-downloaded",
        (Side::Inactive, _) => "inactive-size",
    }
}

impl Mods {
    pub(super) fn move_column(
        &mut self,
        side: Side,
        column: &'static str,
        by: i32,
        cx: &mut Context<Self>,
    ) {
        let Some(at) = kept::chosen(self.view.columns_of(side).0)
            .iter()
            .position(|shown| *shown == column)
        else {
            return;
        };
        self.keep_view(
            |view| {
                let chosen = view.columns_of_mut(side).0;
                *chosen = kept::moved(chosen, column, at as i32 + by);
            },
            cx,
        );
        self.sync_lists(cx);
        cx.notify();
    }
}
