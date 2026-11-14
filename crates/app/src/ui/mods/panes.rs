use super::Mods;
use super::rows::Held;
use crate::ui::button::Btn;
use crate::ui::divider::{Ghost, divider, nudged};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::measure;
use crate::words::{count, put};
use crate::workspace::load_order::Side;
use gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, DragMoveEvent, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, SharedString,
    StatefulInteractiveElement, Styled, TransformationMatrix, Window, canvas, div,
    linear_color_stop, linear_gradient, prelude::FluentBuilder, px,
};
use rimstack_core::say;

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Panes {
    pub inactive: Pixels,
    pub details: Pixels,
    pub hidden: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Grip {
    Inactive,
    Details,
}

pub fn rail_width(theme: &Theme) -> Pixels {
    theme.sized(44.)
}

pub fn first_widths(theme: &Theme) -> (Pixels, Pixels) {
    (theme.sized(290.), theme.sized(360.))
}

/// The inactive mods', the load order's and the details' widths in a window this wide: the
/// details give room first, down to their least, then the inactive mods.
pub fn pane_widths(theme: &Theme, window: Pixels, panes: Panes) -> (Pixels, Pixels, Pixels) {
    let (rail, handles) = match panes.hidden {
        true => (rail_width(theme), 1.),
        false => (px(0.), 2.),
    };
    let room = window - theme::HANDLE * handles - rail;
    let inactive = if panes.hidden { px(0.) } else { panes.inactive };
    let (mut inactive, mut details) = (inactive, panes.details);
    let mut over = inactive + details + theme.sized(380.) - room;
    if over > px(0.) {
        let cut = over.min((details - theme.sized(260.)).max(px(0.)));
        details -= cut;
        over -= cut;
    }
    if over > px(0.) && !panes.hidden {
        inactive -= over.min((inactive - theme.sized(200.)).max(px(0.)));
    }
    (inactive, (room - inactive - details).max(px(0.)), details)
}

fn upwards(theme: &Theme, window: &Window, text: String, colour: Hsla) -> AnyElement {
    let size = theme.small();
    let long = measure(window, theme, &text, size, FontWeight::NORMAL).ceil();
    let tall = theme.line(size).ceil();
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let (family, size_px) = (theme.family.clone(), f32::from(size));
    let (w, h) = (f32::from(tall), f32::from(long));
    let drawn = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><text transform="translate({x} {h}) rotate(-90)" font-family="{family}" font-size="{size_px}">{escaped}</text></svg>"#,
        x = w * 0.78,
    );
    let key = SharedString::from(format!("upwards:{family}:{size_px}:{text}"));
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let _ = window.paint_svg(
                bounds,
                key.clone(),
                Some(drawn.as_bytes()),
                TransformationMatrix::unit(),
                colour,
                cx,
            );
        },
    )
    .w(tall)
    .h(long)
    .into_any_element()
}

impl Mods {
    pub(super) fn panes_wanted(&self) -> Panes {
        Panes {
            inactive: self.layout.inactive_width,
            details: self.layout.details_width,
            hidden: self.view.inactive_hidden && self.comparing.is_none(),
        }
    }

    pub(super) fn set_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        self.layout.drawer_open = false;
        self.keep_view(|view| view.inactive_hidden = hidden, cx);
        self.sync_details(cx);
        cx.notify();
    }

    pub fn set_drawer(&mut self, open: bool, cx: &mut Context<Self>) {
        if open && !self.layout.drawer_open {
            self.layout.drawer_turn = crate::ui::motion::opening();
        }
        self.layout.drawer_open = open;
        if open {
            let text = self.filter.read(cx).text().to_string();
            self.mirror
                .update(cx, |field, cx| field.set_text_quietly(&text, cx));
        }
        cx.notify();
    }

    pub(super) fn drag_grip(
        &mut self,
        grip: Grip,
        at: Point<Pixels>,
        area: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let theme = cx.global::<Theme>().clone();
        let half = theme::HANDLE / 2.;
        match grip {
            Grip::Inactive => {
                self.layout.inactive_width = (at.x - area.left() - half).max(theme.sized(200.));
            }
            Grip::Details => {
                self.layout.details_width = (area.right() - at.x - half).max(theme.sized(260.));
            }
        }
        self.keep_widths_soon(cx);
        self.sync_details(cx);
        cx.notify();
    }

    pub(super) fn nudge_grip(
        &mut self,
        grip: Grip,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let theme = cx.global::<Theme>().clone();
        let Some(by) = nudged(&theme, event) else {
            return;
        };
        match grip {
            Grip::Inactive => {
                self.layout.inactive_width =
                    (self.layout.inactive_width + by).max(theme.sized(200.));
            }
            Grip::Details => {
                self.layout.details_width = (self.layout.details_width - by).max(theme.sized(260.))
            }
        }
        cx.stop_propagation();
        self.keep_widths_soon(cx);
        self.sync_details(cx);
        cx.notify();
    }

    fn reset_width(&mut self, grip: Grip, cx: &mut Context<Self>) {
        let (inactive, details) = first_widths(cx.global::<Theme>());
        match grip {
            Grip::Inactive => self.layout.inactive_width = inactive,
            Grip::Details => self.layout.details_width = details,
        }
        self.keep_widths_soon(cx);
        self.sync_details(cx);
        cx.notify();
    }

    fn keep_widths_soon(&mut self, cx: &mut Context<Self>) {
        let (inactive, details) = (self.layout.inactive_width, self.layout.details_width);
        self.keep_view_soon(
            |view| {
                view.inactive = f32::from(inactive);
                view.details = f32::from(details);
            },
            cx,
        );
    }

    fn grip(
        &mut self,
        theme: &Theme,
        grip: Grip,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (id, tip) = match grip {
            Grip::Inactive => ("grip-inactive", put(&say!("Width of the inactive mods"))),
            Grip::Details => ("grip-details", put(&say!("Width of the details"))),
        };
        let handle = self.reachable(id, cx).0;
        divider(theme, id, &handle, window)
            .aria_label(tip)
            .on_key_down(cx.listener(move |mods, event, _, cx| mods.nudge_grip(grip, event, cx)))
            .on_drag(grip, |_, _, _, cx| cx.new(|_| Ghost))
            .on_click(cx.listener(move |mods, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    mods.reset_width(grip, cx);
                }
            }))
    }

    fn rail(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let open = self.layout.drawer_open;
        let (icon, tip) = match open {
            true => ("x", put(&say!("Close the inactive mods"))),
            false => ("plus", put(&say!("Inactive mods"))),
        };
        let said = put(&say!(
            "Inactive {}",
            count(self.list.read(cx).count(Side::Inactive))
        ));
        let focus = c.focus;
        div()
            .id("rail")
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .flex_none()
            .w(rail_width(theme))
            .h_full()
            .pt(theme::GAP + px(1.))
            .gap(theme::GAP_LARGE)
            .bg(c.panel)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(1.))
                    .bg(c.edge),
            )
            .child(
                Btn::new("rail-button", "")
                    .icon(icon)
                    .primary_if(!open)
                    .tip(tip)
                    .on_click(cx.listener(move |mods, _, _, cx| mods.set_drawer(!open, cx))),
            )
            .child(upwards(theme, window, said, c.ink3))
            .drag_over::<Held>(move |s, _, _, _| s.border_2().border_color(focus))
            .on_drop(cx.listener(|mods, held: &Held, _, cx| {
                let end = mods.list.read(cx).count(Side::Inactive);
                mods.drop_on(
                    (held.side, held.index, held.ids.clone()),
                    (Side::Inactive, end),
                    cx,
                );
            }))
    }

    fn drawer(
        &mut self,
        theme: &Theme,
        room: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let rail = rail_width(theme);
        let wide = theme
            .sized(260.)
            .max(self.layout.inactive_width)
            .min(room - rail - theme.sized(60.));
        let shade = linear_gradient(
            90.,
            linear_color_stop(gpui::black().opacity(0.3), 0.),
            linear_color_stop(gpui::black().opacity(0.), 1.),
        );
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(rail)
            .right_0()
            .child(
                div()
                    .id("drawer-scrim")
                    .absolute()
                    .inset_0()
                    .bg(c.scrim)
                    .on_click(cx.listener(|mods, _, _, cx| mods.set_drawer(false, cx))),
            )
            .child(crate::ui::motion::slid_in(
                div()
                    .id("drawer")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(wide)
                    .occlude()
                    .child(
                        self.inactive_pane(theme, wide, true, window, cx)
                            .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(1.))
                            .bg(c.edge),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(-theme::GAP_LARGE)
                            .w(theme::GAP_LARGE)
                            .bg(shade),
                    ),
                wide,
                ("drawer-in", self.layout.drawer_turn),
            ))
            .into_any_element()
    }

    pub(super) fn panes(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let room = window.viewport_size().width;
        let panes = self.panes_wanted();
        let (inactive_width, active_width, details_width) = pane_widths(theme, room, panes);
        let drawer =
            (panes.hidden && self.layout.drawer_open).then(|| self.drawer(theme, room, window, cx));
        div()
            .id("panes")
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .when(panes.hidden, |d| d.child(self.rail(theme, window, cx)))
            .when(!panes.hidden, |d| {
                d.child(
                    self.inactive_pane(theme, inactive_width, false, window, cx)
                        .w(inactive_width)
                        .flex_none(),
                )
                .child(self.grip(theme, Grip::Inactive, window, cx))
            })
            .child(
                self.active_pane(theme, active_width, window, cx)
                    .flex_1()
                    .min_w_0(),
            )
            .child(self.grip(theme, Grip::Details, window, cx))
            .child(
                div()
                    .w(details_width)
                    .flex_none()
                    .h_full()
                    .child(crate::ui::cached(self.details_view.clone())),
            )
            .on_drag_move::<Grip>(cx.listener(|mods, event: &DragMoveEvent<Grip>, _, cx| {
                let grip = *event.drag(cx);
                mods.drag_grip(grip, event.event.position, event.bounds, cx);
            }))
            .on_drop(cx.listener(|_, _: &Grip, _, _| {}))
            .children(drawer)
    }
}
