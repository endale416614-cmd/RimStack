use super::theme::{self, Theme};
use super::widgets::TextSized;
use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div,
};
use std::time::Duration;

const DELAY: Duration = Duration::from_millis(600);

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let c = &theme.colours;
        div()
            .max_w(theme.sized(340.) + theme::GAP * 2.)
            .p(theme::GAP)
            .rounded(theme::CORNER)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .child(
                div()
                    .spaced_text(theme, theme.small(), 1.25)
                    .text_color(c.ink)
                    .child(self.0.clone()),
            )
    }
}

/// The tip of what the pointer is on, as `Notices.hint`: the status bar shows its first line. An
/// entity, so that only what shows it is drawn again as the pointer moves.
#[derive(Default)]
pub struct Hint(pub SharedString);

struct Held(gpui::Entity<Hint>);

impl gpui::Global for Held {}

pub fn install(cx: &mut App) {
    let hint = cx.new(|_| Hint::default());
    cx.set_global(Held(hint));
}

/// What the status bar says of the control pointed at, empty when none.
pub fn hint(cx: &App) -> SharedString {
    cx.try_global::<Held>()
        .map(|held| held.0.read(cx).0.clone())
        .unwrap_or_default()
}

/// The status bar's hint is `text`'s first line.
pub fn point_at(text: SharedString, cx: &mut App) {
    let Some(held) = cx.try_global::<Held>().map(|held| held.0.clone()) else {
        return;
    };
    held.update(cx, |hint, cx| {
        if hint.0 != text {
            hint.0 = text;
            cx.notify();
        }
    });
}

/// The hint let go of, as a press, a key or the wheel may take the control away under a pointer
/// that does not move, which would then never tell it left.
pub fn forget_hint(cx: &mut App) {
    point_at(SharedString::default(), cx);
}

/// `control` with its tip; an empty one shows none.
pub fn tipped(control: Stateful<gpui::Div>, text: SharedString) -> Stateful<gpui::Div> {
    if text.is_empty() {
        return control;
    }
    let pointed = text.clone();
    control
        .on_hover(move |over, _, cx| {
            if *over {
                point_at(pointed.clone(), cx);
            } else if hint(cx) == pointed {
                forget_hint(cx);
            }
        })
        .tooltip(move |_: &mut Window, cx: &mut App| -> AnyView {
            cx.new(|_| Tip(text.clone())).into()
        })
        .tooltip_show_delay(DELAY)
}
