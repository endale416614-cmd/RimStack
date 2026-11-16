use super::rows::{Scrolling, bar};
use super::theme::Theme;
use gpui::{
    ElementId, EntityId, HitboxBehavior, InteractiveElement, IntoElement, ParentElement,
    ScrollEvent, ScrollHandle, StatefulInteractiveElement, Styled, canvas, div, point, px,
};

#[derive(Clone, Default)]
pub struct Paging {
    pub scroll: Scrolling,
    handle: ScrollHandle,
}

impl Paging {
    pub fn new(reduce_motion: bool) -> Self {
        let paging = Self::default();
        paging.scroll.borrow_mut().set_reduce_motion(reduce_motion);
        paging
    }

    pub fn contain_drawn(&self, top: gpui::Pixels, bottom: gpui::Pixels) {
        let shift = self.scroll.borrow().top() - self.handle.bounds().top();
        self.scroll
            .borrow_mut()
            .contain(top + shift, bottom + shift);
    }
}

pub fn room(theme: &Theme) -> gpui::Pixels {
    theme.sized(9.) + super::theme::GAP_HAIR * 2.
}

/// `content` in a scrolled area that fills its place; `view` is drawn again as it glides.
pub fn page(
    id: impl Into<ElementId>,
    paging: &Paging,
    theme: &Theme,
    view: EntityId,
    content: impl IntoElement,
) -> gpui::Div {
    let line = theme.sized(40.);
    let (follow, handle) = (paging.scroll.clone(), paging.handle.clone());
    let (wheeled, wheel_bounds) = (paging.scroll.clone(), paging.handle.clone());
    // Laid out after the content: the handle knows how tall it is by then.
    let steer = canvas(
        move |_, window, _| {
            let shown = handle.bounds().size.height;
            let total = shown + handle.max_offset().y;
            let gliding = {
                let mut scroll = follow.borrow_mut();
                scroll.fit(total, shown);
                scroll.advance()
            };
            let offset = point(px(0.), -follow.borrow().top());
            // gpui-fast takes a write as a scroll, which draws the page's view again.
            if handle.offset() != offset {
                handle.set_offset(offset);
            }
            if gliding {
                window.request_animation_frame();
            }
            window.insert_hitbox(wheel_bounds.bounds(), HitboxBehavior::Normal)
        },
        move |_, hitbox, window, _| {
            // Before the area's own handler: the wheel glides instead of jumping.
            window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                if phase.capture() && hitbox.should_handle_scroll(window) {
                    wheeled.borrow_mut().wheel(event.delta, line);
                    cx.notify(view);
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_0();
    div()
        .relative()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id(id)
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&paging.handle)
                .child(content),
        )
        .child(steer)
        .child(bar(theme, paging.scroll.clone(), view))
}
