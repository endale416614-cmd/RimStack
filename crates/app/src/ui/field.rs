use super::text_field::TextField;
use super::theme::{self, Theme};
use super::widgets::{TextSized, halo, icon, label};
use gpui::{
    App as Cx, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

pub struct Field<'a> {
    pub id: &'static str,
    pub text: &'a Entity<TextField>,
    pub icon: &'static str,
    pub placeholder: String,
    pub found: Option<String>,
    pub keys: Option<String>,
}

impl Field<'_> {
    pub fn render(self, theme: &Theme, window: &Window, cx: &Cx) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let field = self.text.read(cx);
        let (empty, focused) = (field.text().is_empty(), field.is_focused(window));
        let focus = field.focus_handle(cx);
        let lead = match self.icon.is_empty() {
            true => theme::GAP_SMALL,
            false => theme::GAP_SMALL + theme.icon() + theme::GAP,
        };
        let held = self.text.clone();
        div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .h(theme.field())
            .pl(lead + theme::GAP_TINY)
            .pr(theme::GAP)
            .gap(theme::GAP)
            .rounded(theme::CORNER)
            .bg(c.well)
            .border_1()
            .border_color(c.field_line)
            .child(focus_line(theme, focused))
            .when(focused, |d| d.children(halo(c.focus, theme::CORNER)))
            .cursor_text()
            .on_click(move |_, window, cx| window.focus(&focus, cx))
            .when(!self.icon.is_empty(), |d| {
                d.child(
                    div()
                        .absolute()
                        .left(theme::GAP)
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .child(icon(self.icon, theme.icon() - px(1.), c.ink3)),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .when(empty, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink3)
                                .child(self.placeholder),
                        )
                    })
                    .child(self.text.clone()),
            )
            .when_some(
                self.found.filter(|found| !found.is_empty() && !empty),
                |d, found| d.child(label(theme, theme.small(), c.ink2, found)),
            )
            .when_some(self.keys.filter(|_| empty && !focused), |d, keys| {
                d.child(label(theme, theme.caption(), c.ink3, keys))
            })
            .when(!empty, |d| {
                d.child(super::tip::tipped(
                    div()
                        .id((self.id, 1usize))
                        .role(gpui::Role::Button)
                        .aria_label(crate::words::put(&rimstack_core::say!("Clear (Esc)")))
                        .cursor_pointer()
                        .child(icon("x", theme.sized(12.), c.ink3))
                        .on_click(move |_, window, cx| {
                            held.update(cx, |field, cx| field.set_text("", cx));
                            let focus = held.read(cx).focus_handle(cx);
                            window.focus(&focus, cx);
                        }),
                    crate::words::put(&rimstack_core::say!("Clear (Esc)")).into(),
                ))
            })
    }

    pub fn area(self, theme: &Theme, window: &Window, cx: &Cx) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let field = self.text.read(cx);
        let (empty, focused) = (field.text().is_empty(), field.is_focused(window));
        let focus = field.focus_handle(cx);
        div()
            .id(self.id)
            .relative()
            .py(theme::GAP_SMALL)
            .pl(theme::GAP_SMALL + theme::GAP_TINY)
            .pr(theme::GAP)
            .rounded(theme::CORNER)
            .bg(c.well)
            .border_1()
            .border_color(c.field_line)
            .child(focus_line(theme, focused))
            .when(focused, |d| d.children(halo(c.focus, theme::CORNER)))
            .cursor_text()
            .on_click(move |_, window, cx| window.focus(&focus, cx))
            .when(empty, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(theme::GAP_SMALL)
                        .left(theme::GAP_SMALL + theme::GAP_TINY)
                        .sized_text(theme, theme.body())
                        .text_color(c.ink3)
                        .child(self.placeholder),
                )
            })
            .child(self.text.clone())
    }
}

fn focus_line(theme: &Theme, focused: bool) -> impl IntoElement {
    let edge = -px(1.);
    super::motion::layer(
        div()
            .absolute()
            .top(edge)
            .left(edge)
            .right(edge)
            .bottom(edge)
            .rounded(theme::CORNER)
            .border_1()
            .border_color(theme.colours.focus),
        focused,
    )
}
