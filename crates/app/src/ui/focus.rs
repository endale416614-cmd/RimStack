//! A control Tab reaches, and the ring drawn around it
//! while it holds the keys. A click does not give it the keys: the text being typed
//! keeps them.

use super::theme::Theme;
use super::widgets::{TextSized, halo};
use gpui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Pixels, Stateful, Styled, Window, actions, div, px,
};

actions!(focus, [Next, Previous]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Next, None),
        KeyBinding::new("shift-tab", Previous, None),
    ]);
}

pub fn handle(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> FocusHandle {
    let kept = window.use_keyed_state(id, cx, |_, cx| cx.focus_handle().tab_stop(true));
    kept.read(cx).clone()
}

/// Whether the ring shows: the control holds the keys, and a key brought them there.
pub fn ringed(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && window.last_input_was_keyboard()
}

/// `control` reached by Tab and pressed by Enter or Space; with its ring when it holds the keys.
pub fn reachable(
    control: Stateful<gpui::Div>,
    handle: &FocusHandle,
    corner: Pixels,
    on_accent: bool,
    window: &Window,
    theme: &Theme,
) -> Stateful<gpui::Div> {
    let shown = ringed(handle, window);
    control
        .track_focus(handle)
        .capture_any_mouse_down(|_, window, _| window.prevent_default())
        .when(shown, |d| d.child(ring(theme, corner, on_accent)))
}

/// 2 wide, 4 outside, in the focus colour with its halo.
fn ring(theme: &Theme, corner: Pixels, on_accent: bool) -> AnyElement {
    let c = &theme.colours;
    let tone = if on_accent { c.on_selected } else { c.focus };
    let out = px(-4.);
    div()
        .absolute()
        .top(out)
        .bottom(out)
        .left(out)
        .right(out)
        .rounded(corner + px(4.))
        .border_width(px(2.))
        .border_color(tone)
        .children(halo(c.focus, corner + px(4.)))
        .into_any_element()
}

use gpui::prelude::FluentBuilder;
