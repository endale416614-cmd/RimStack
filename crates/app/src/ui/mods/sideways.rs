//! How far a list and its headings are scrolled sideways, once its columns are wider than it.
//! Both read it while drawn, so both are drawn again when it moves.

use crate::ui::rows::bar_room;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::with_alpha;
use gpui::{
    App, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollDelta,
    ScrollEvent, Styled, canvas, fill, point, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
pub struct Sideways {
    x: Pixels,
    /// How far the columns reach past the list, as last drawn.
    most: Rc<Cell<Pixels>>,
    /// While the bar's thumb is held: how far right of the thumb's left it was taken.
    held: Rc<Cell<Option<Pixels>>>,
}

impl Sideways {
    pub fn x(&self) -> Pixels {
        self.x.clamp(px(0.), self.most.get())
    }

    /// Where a list tells how far its columns reach past it, while drawn: a cell, which draws
    /// nothing.
    pub fn reach(&self) -> Rc<Cell<Pixels>> {
        self.most.clone()
    }

    fn move_to(&mut self, x: Pixels) -> bool {
        let to = x.clamp(px(0.), self.most.get());
        let moved = to != self.x();
        self.x = to;
        moved
    }
}

/// A wheel turned sideways, or with Shift held; a wheel turned only up or down is the list's.
pub fn is_sideways(event: &ScrollEvent) -> bool {
    let (x, y) = match event.delta {
        ScrollDelta::Pixels(moved) => (moved.x, moved.y),
        ScrollDelta::Lines(lines) => (px(lines.x), px(lines.y)),
    };
    event.modifiers.shift || x.abs() > y.abs()
}

pub fn wheel(sideways: &Entity<Sideways>, event: &ScrollEvent, line: Pixels, cx: &mut App) {
    let moved = match event.delta {
        ScrollDelta::Pixels(moved) => moved,
        ScrollDelta::Lines(lines) => point(line * lines.x, line * lines.y),
    };
    let by = match moved.x == px(0.) {
        true => moved.y,
        false => moved.x,
    };
    sideways.update(cx, |sideways, cx| {
        if sideways.move_to(sideways.x - by) {
            cx.notify();
        }
    });
}

/// The bar under a list whose columns are wider than it, its thumb held to scroll.
pub fn bar(theme: &Theme, sideways: &Entity<Sideways>, cx: &App) -> impl gpui::IntoElement {
    let (colour, strong) = (with_alpha(theme.colours.ink3, 0.6), theme.colours.ink2);
    let thick = theme.sized(9.);
    let read = sideways.read(cx);
    let (x, most, held) = (read.x(), read.most.clone(), read.held.clone());
    let sideways = sideways.clone();
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let most_now = most.get();
            if most_now <= px(0.) {
                return;
            }
            let track = bounds.size.width - theme::GAP_HAIR * 2.;
            let length = (track * (track / (track + most_now))).max(thick * 3.);
            let room = (track - length).max(px(1.));
            let left = room * (x / most_now);
            let at = point(bounds.left() + theme::GAP_HAIR + left, bounds.top());
            let shown = if held.get().is_some() { strong } else { colour };
            window.paint_quad(
                fill(gpui::Bounds::new(at, size(length, thick)), shown).corner_radii(thick / 2.),
            );
            let to = move |within: Pixels, grip: Pixels| {
                most_now * ((within - grip) / room).clamp(0., 1.)
            };
            let (pressed, dragged) = (sideways.clone(), sideways.clone());
            let (taken, moving, letting) = (held.clone(), held.clone(), held.clone());
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if !phase.bubble()
                    || event.button != MouseButton::Left
                    || !bounds.contains(&event.position)
                {
                    return;
                }
                let within = event.position.x - bounds.left() - theme::GAP_HAIR;
                let grip = match within >= left && within <= left + length {
                    true => within - left,
                    false => length / 2.,
                };
                taken.set(Some(grip));
                pressed.update(cx, |sideways, cx| {
                    sideways.move_to(to(within, grip));
                    cx.notify();
                });
                cx.stop_propagation();
            });
            window.on_mouse_event(move |event: &MouseMoveEvent, _, _, cx| {
                let Some(grip) = moving.get() else {
                    return;
                };
                if !event.dragging() {
                    moving.set(None);
                    return;
                }
                let within = event.position.x - bounds.left() - theme::GAP_HAIR;
                dragged.update(cx, |sideways, cx| {
                    if sideways.move_to(to(within, grip)) {
                        cx.notify();
                    }
                });
            });
            window.on_mouse_event(move |_: &MouseUpEvent, _, _, _| letting.set(None));
        },
    )
    .absolute()
    .left_0()
    .right(bar_room(theme))
    .bottom(theme::GAP_HAIR)
    .h(thick)
}
