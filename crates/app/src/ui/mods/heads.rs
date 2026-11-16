use super::Mods;
use super::columns::{ColumnEdge, ColumnHeld, edge, heading_reach, placed};
use super::list::bar_room;
use super::rows::Columns;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, watched};
use crate::words::put;
use crate::workspace::load_order::Side;
use gpui::{
    Context, InteractiveElement, MouseButton, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use rimstack_core::say;

impl Mods {
    fn heading_label(&self, side: Side, key: &str) -> String {
        self.orders
            .by(side)
            .get(key)
            .map(|entry| put(&entry.label))
            .unwrap_or_default()
    }

    /// A list's headings over its columns; the load order's begin with its numbers and end with
    /// the View menu.
    pub(super) fn heads(
        &mut self,
        theme: &Theme,
        side: Side,
        columns: &Columns,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        self.layout.shown[side as usize] = columns.fitting.iter().map(|(key, _)| *key).collect();
        let me = cx.entity().downgrade();
        let tip = put(&say!(
            "Click to order the list by this column; click again to reverse it. Right-click to choose the columns. Drag it, or press Ctrl + Alt + Left or Right, to move it."
        ));
        let named = ["", "name"]
            .into_iter()
            .chain(columns.fitting.iter().map(|(key, _)| *key));
        let reach: std::collections::HashMap<&'static str, _> = named
            .map(|key| (key, self.reachable(heading_reach(side, key), cx)))
            .collect();
        let row = side as usize;
        let heading = |key: &'static str, text: String, on: bool| {
            let (pressed, asked, keyed) = (me.clone(), me.clone(), me.clone());
            let said = match key {
                "" => put(&say!("The load order itself")),
                _ => tip.clone(),
            };
            let (handle, drawn) = reach[key].clone();
            let focused = handle.is_focused(window);
            crate::ui::tip::tipped(
                div()
                    .id(gpui::SharedString::from(format!("heading-{row}-{key}")))
                    .relative()
                    .role(gpui::Role::ColumnHeader)
                    .aria_label(text.clone())
                    .track_focus(&handle)
                    .capture_any_mouse_down(|_, window, _| window.prevent_default())
                    .cursor_pointer()
                    .child(watched(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .sized_text(theme, theme.caption())
                            .text_color(if on { c.ink } else { c.ink3 })
                            .font_weight(theme.medium)
                            .when(focused, |d| d.underline()),
                        text.clone().into(),
                        text,
                        true,
                    ))
                    .child(
                        gpui::canvas(move |bounds, _, _| drawn.set(bounds), |_, _, _, _| {})
                            .absolute()
                            .inset_0(),
                    )
                    .on_key_down(move |event, window, cx| {
                        let _ = keyed.update(cx, |mods, cx| {
                            mods.heading_key(side, key, event, window, cx)
                        });
                    })
                    .on_click(move |_, _, cx| {
                        let _ = pressed.update(cx, |mods, cx| mods.heading_pressed(side, key, cx));
                    })
                    .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                        let at = event.position;
                        let _ =
                            asked.update(cx, |mods, cx| mods.columns_menu(side, at, window, cx));
                    }),
                said.into(),
            )
        };
        let on = |key: &str| self.orders.by(side).get(key).is_some_and(|e| e.on);
        let view_menu = cx.entity().downgrade();
        let cells = columns
            .fitting
            .iter()
            .enumerate()
            .map(|(at, (key, width))| {
                let last = at + 1 == columns.fitting.len();
                let (key, width) = (*key, *width);
                div()
                    .id(gpui::SharedString::from(format!("column-{row}-{key}")))
                    .relative()
                    .flex()
                    .flex_none()
                    .h_full()
                    .items_center()
                    .w(width)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .when(matches!(key, "load" | "size"), |d| d.justify_end())
                            .child(heading(key, self.heading_label(side, key), on(key))),
                    )
                    .when(!last, |d| d.child(edge(theme, side, key, columns, cx)))
                    .on_drag(ColumnHeld { side, key }, |_, _, _, cx| {
                        gpui::AppContext::new(cx, |_| crate::ui::divider::Ghost)
                    })
            })
            .collect::<Vec<_>>();
        let view_button = Btn::new("view", put(&say!("View")))
            .spoken(put(&say!("View options")))
            .opens(move |below, window, cx| {
                let _ = view_menu.update(cx, |mods, cx| mods.view_menu(below, window, cx));
            });
        let view_button = placed(view_button, self.layout.menu_buttons.view.clone());
        let view_fixed = columns.load_order.then(|| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right(theme::GAP_WIDE + bar_room(theme))
                .w(columns.quick)
                .flex()
                .items_center()
                .justify_end()
                .child(view_button)
        });
        let number = columns.load_order.then(|| {
            div()
                .flex()
                .flex_none()
                .justify_end()
                .w(columns.number)
                .child(heading("", "#".into(), false))
        });
        let name = div()
            .relative()
            .flex()
            .flex_none()
            .h_full()
            .items_center()
            .w(columns.name_cell)
            .child(div().flex_1().min_w_0().overflow_hidden().child(heading(
                "name",
                self.heading_label(side, "name"),
                on("name"),
            )))
            .child(edge(theme, side, "name", columns, cx));
        let sideways = self.sideways[row].clone();
        let x = sideways.read(cx).x();
        let line = theme.sized(40.);
        div()
            .id(("headings", row))
            .relative()
            .flex_none()
            .h(theme.control())
            .on_drag_move::<ColumnEdge>(cx.listener(
                |mods, event: &gpui::DragMoveEvent<ColumnEdge>, _, cx| {
                    mods.size_column(event.event.position.x, cx);
                },
            ))
            .on_drop(cx.listener(|mods, _: &ColumnEdge, _, _| {
                mods.layout.edge_drag = None;
            }))
            .on_drop(cx.listener(|mods, held: &ColumnHeld, window, cx| {
                mods.drop_column(*held, window.mouse_position().x, cx);
            }))
            .on_scroll(move |event, _, cx| {
                if super::sideways::is_sideways(event) {
                    super::sideways::wheel(&sideways, event, line, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right(columns.fixed + bar_room(theme))
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(-x)
                            .w(columns.across)
                            .flex()
                            .items_center()
                            .pl(theme::GAP_WIDE)
                            .gap(theme::GAP_LARGE)
                            .children(number)
                            .child(name)
                            .children(cells),
                    ),
            )
            .children(view_fixed)
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(theme::GAP_SMALL)
                    .right(theme::GAP_SMALL)
                    .h(px(1.))
                    .bg(c.line),
            )
    }
}
