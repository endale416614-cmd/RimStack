use super::{FocusFilter, Mods, Redo, Save, Undo, pane_widths};
use crate::ui::theme::{self, Theme};
use gpui::{
    Context, Focusable, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, prelude::FluentBuilder, px,
};

impl Focusable for Mods {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus.clone()
    }
}

impl Render for Mods {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        RENDERS.with(|renders| renders.set(renders.get() + 1));
        let theme = cx.global::<Theme>().clone();
        self.run_when_drawn(window, cx);
        if std::mem::take(&mut self.focus_wanted) {
            let focus = self.active_list.read(cx).focus_handle();
            window.focus(&focus, cx);
        }
        let handle = || {
            div()
                .flex()
                .flex_none()
                .justify_center()
                .w(theme::HANDLE)
                .h_full()
                .child(div().w(px(1.)).h_full().bg(theme.colours.edge))
        };
        let (_, _, details_width) =
            pane_widths(&theme, window.viewport_size().width, self.panes_wanted());
        let bulk = self.bulk_bar(&theme, cx);
        let unread = self.unread(&theme, cx);
        let is_unread = unread.is_some();
        self.hints.anchors.forget_drawn();
        let panes =
            (self.comparing.is_none() && !is_unread).then(|| self.panes(&theme, window, cx));
        let hint = match panes.is_some() {
            true => self.hint(&theme, window, cx),
            false => None,
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .track_focus(&self.focus)
            .on_action(cx.listener(|mods, _: &FocusFilter, window, cx| {
                let field = match mods.layout.drawer_open && mods.view.inactive_hidden {
                    true => &mods.mirror,
                    false => &mods.filter,
                };
                window.focus(&field.focus_handle(cx), cx);
            }))
            .on_key_down(cx.listener(|mods, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key != "escape" || mods.comparing.is_some() {
                    return;
                }
                if mods.layout.drawer_open {
                    mods.set_drawer(false, cx);
                } else if mods.chosen_reads.picking.is_some() {
                    mods.start_picking(None, cx);
                } else if !mods.list.read(cx).selection.is_empty() {
                    mods.select(None, true, cx);
                } else {
                    return;
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|mods, _: &Undo, _, cx| mods.undo(cx)))
            .on_action(cx.listener(|mods, _: &Redo, _, cx| mods.redo(cx)))
            .on_action(cx.listener(|mods, _: &Save, _, cx| {
                if mods.list.read(cx).standing.dirty {
                    mods.save(None, cx);
                }
            }))
            .when_some(self.comparing.clone(), |d, view| {
                d.child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(div().flex_1().min_w_0().h_full().child(view))
                        .child(handle())
                        .child(
                            div()
                                .w(details_width)
                                .flex_none()
                                .h_full()
                                .child(crate::ui::cached(self.details_view.clone())),
                        ),
                )
            })
            .when_some(unread, |d, unread| d.child(unread))
            .when(self.comparing.is_none() && !is_unread, |d| {
                d.children(panes)
            })
            .child(bulk)
            .children(hint)
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
