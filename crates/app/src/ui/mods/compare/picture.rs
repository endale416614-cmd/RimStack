use super::{CompareView, Cutting};
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::label;
use crate::words::put;
use crate::workspace::textures::Shared;
use gpui::{
    AppContext, Bounds, Context, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent,
    ObjectFit, ParentElement, Pixels, StatefulInteractiveElement, Styled, StyledImage, canvas, div,
    fill, point, prelude::FluentBuilder, px, size,
};
use rimstack_core::say;
use std::path::PathBuf;

impl CompareView {
    pub(super) fn picture(
        &self,
        theme: &Theme,
        id: &'static str,
        file: String,
        kept: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let found = self.pixels_of(&file);
        let room = self.boxes.borrow().get(id).copied();
        let (boxes, me) = (self.boxes.clone(), cx.entity_id());
        let measure = canvas(
            move |bounds, _, cx| {
                if boxes.borrow_mut().insert(id, bounds.size) != Some(bounds.size) {
                    crate::ui::redraw_after(me, cx);
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let placed = found.zip(room).map(|((width, height), room)| {
            let margin = theme::GAP_CARD;
            let ratio = width as f32 / height.max(1) as f32;
            let fitted = (room.width - margin * 2.)
                .min((room.height - margin * 2.) * ratio)
                .max(px(1.));
            let shown = size(
                fitted * self.zoom.max(1.),
                fitted * self.zoom.max(1.) / ratio,
            );
            let at = |free: Pixels, share: f32| match free > px(0.) {
                true => free / 2.,
                false => free * share,
            };
            (
                point(
                    at(room.width - shown.width, self.pan.x),
                    at(room.height - shown.height, self.pan.y),
                ),
                shown,
            )
        });
        let (dark, light) = (c.ctl, c.seg);
        let me = cx.entity().downgrade();
        div()
            .id(id)
            .role(gpui::Role::Image)
            .aria_label(put(&match id {
                "compare-difference" => say!("Where the two pictures differ"),
                "compare-later" | "slider-over" => say!("The later mod's picture"),
                _ => say!("The earlier mod's picture"),
            }))
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .rounded(theme::CORNER_LARGE)
            .bg(c.well)
            .border_color(if kept { c.focus } else { c.line })
            .map(|d| if kept { d.border_2() } else { d.border_1() })
            .overflow_hidden()
            .child(measure)
            .when_some(placed, |d, (at, shown)| {
                let checker = canvas(
                    |_, _, _| (),
                    move |area: Bounds<Pixels>, _, window, _| {
                        let step = px(8.);
                        window.paint_quad(fill(area, dark));
                        let (across, down) = (
                            (area.size.width / step).ceil() as usize,
                            (area.size.height / step).ceil() as usize,
                        );
                        for row in 0..down {
                            for column in (row % 2..across).step_by(2) {
                                let corner = point(
                                    area.left() + step * column as f32,
                                    area.top() + step * row as f32,
                                );
                                let square = size(
                                    step.min(area.right() - corner.x),
                                    step.min(area.bottom() - corner.y),
                                );
                                window.paint_quad(fill(Bounds::new(corner, square), light));
                            }
                        }
                    },
                );
                let place = |e: gpui::Div| {
                    e.absolute()
                        .left(at.x)
                        .top(at.y)
                        .w(shown.width)
                        .h(shown.height)
                };
                let held = self.pictures.borrow_mut().entry(id).or_default().clone();
                let pictures = held.of(&file, cx);
                d.child(place(div()).child(checker.size_full())).child(
                    place(div()).child(
                        pictures
                            .img(PathBuf::from(&file))
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
                )
            })
            .when(found.is_none() && !file.is_empty(), |d| {
                d.flex().items_center().justify_center().child(label(
                    theme,
                    theme.body(),
                    c.ink2,
                    put(&say!("This picture cannot be shown.")),
                ))
            })
            .when(self.zoom > 0., |d| {
                d.cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, event: &gpui::MouseDownEvent, _, cx| {
                            view.dragged = Some((event.position, view.pan));
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                        let _ = me.update(cx, |view, cx| view.pan_to(event, id, cx));
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, _| view.dragged = None),
                    )
            })
    }

    fn pan_to(&mut self, event: &MouseMoveEvent, id: &'static str, cx: &mut Context<Self>) {
        let Some((from, pan)) = self.dragged.filter(|_| event.dragging()) else {
            return;
        };
        let Some(room) = self.boxes.borrow().get(id).copied() else {
            return;
        };
        let moved = event.position - from;
        let overflow = |side: Pixels| f32::from(side * (self.zoom - 1.).max(0.01));
        self.pan = point(
            (pan.x - f32::from(moved.x) / overflow(room.width)).clamp(0., 1.),
            (pan.y - f32::from(moved.y) / overflow(room.height)).clamp(0., 1.),
        );
        cx.notify();
    }

    pub(super) fn slider(
        &self,
        theme: &Theme,
        texture: &Shared,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let names = self
            .comparison
            .as_ref()
            .map(|c| (c.earlier.name.clone(), c.later.name.clone()))
            .unwrap_or_default();
        let [keep_earlier, keep_later] =
            [true, false].map(|earlier| match texture.keeps_earlier == earlier {
                true => label(theme, theme.body(), c.ok, put(&say!("✓ Kept")))
                    .font_weight(theme.medium)
                    .into_any_element(),
                false => Btn::new(
                    ("slider-keep", usize::from(earlier)),
                    put(&say!("Keep this one")),
                )
                .on_click(cx.listener(move |view, _, _, cx| view.keep(earlier, cx)))
                .into_any_element(),
            });
        let width = self
            .boxes
            .borrow()
            .get("slider-under")
            .map_or(px(0.), |room| room.width);
        let cut = width * self.cut;
        let handle = theme.sized(28.);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(theme::GAP)
            .p(theme::GAP_WIDE)
            .pt(theme::GAP_TINY)
            .child(
                div()
                    .id("slider-area")
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.picture(
                        theme,
                        "slider-under",
                        texture.earlier_shown.clone(),
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .left(cut)
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .right_0()
                                    .w(width)
                                    .flex()
                                    .child(self.picture(
                                        theme,
                                        "slider-over",
                                        texture.later_shown.clone(),
                                        false,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("slider-handle")
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(cut - handle / 2.)
                            .w(handle)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_col_resize()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(handle / 2. - px(1.))
                                    .w(px(2.))
                                    .bg(c.focus),
                            )
                            .child(
                                div()
                                    .w(theme::GAP_CARD)
                                    .h(theme.sized(34.))
                                    .rounded(theme::CORNER)
                                    .bg(c.focus),
                            )
                            .on_drag(Cutting, |_, _, _, cx| cx.new(|_| crate::ui::divider::Ghost)),
                    )
                    .on_drag_move::<Cutting>(cx.listener(
                        |view, event: &gpui::DragMoveEvent<Cutting>, _, cx| {
                            let bounds = event.bounds;
                            view.cut = ((event.event.position.x - bounds.left())
                                / bounds.size.width.max(px(1.)))
                            .clamp(0., 1.);
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .min_h(theme.sized(28.))
                    .child(label(theme, theme.body(), c.ink, format!("◀ {}", names.0)))
                    .child(keep_earlier)
                    .child(div().flex_1())
                    .child(keep_later)
                    .child(label(theme, theme.body(), c.ink, format!("{} ▶", names.1))),
            )
    }
}
