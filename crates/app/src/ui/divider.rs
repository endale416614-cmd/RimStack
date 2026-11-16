use super::theme::{self, Theme};
use gpui::{
    Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Stateful, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use std::time::Duration;

/// The quiet after a divider stops before its width is kept.
pub const SETTLED: Duration = Duration::from_millis(400);

/// What a drag shows under the pointer when what it moves moves in place: nothing.
pub struct Ghost;

impl gpui::Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// How far Left or Right moves a divider, rightwards; None for any other key.
pub fn nudged(theme: &Theme, event: &KeyDownEvent) -> Option<Pixels> {
    match event.keystroke.key.as_str() {
        "left" => Some(-theme.sized(24.)),
        "right" => Some(theme.sized(24.)),
        _ => None,
    }
}

pub fn divider(
    theme: &Theme,
    id: &'static str,
    handle: &FocusHandle,
    window: &Window,
) -> Stateful<Div> {
    let c = &theme.colours;
    let focus = c.focus;
    let held = handle.is_focused(window);
    div()
        .id(id)
        .group(id)
        .role(gpui::Role::Splitter)
        .track_focus(handle)
        .capture_any_mouse_down(|_, window, _| window.prevent_default())
        .flex()
        .flex_none()
        .justify_center()
        .w(theme::HANDLE)
        .h_full()
        .cursor_col_resize()
        .child(
            div()
                .w(px(1.))
                .h_full()
                .bg(c.edge)
                .when(held, |d| d.w(px(3.)).bg(focus))
                .group_hover(id, move |s| s.w(px(3.)).bg(focus)),
        )
}
