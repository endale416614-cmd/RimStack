use super::Mods;
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use gpui::{
    AnyElement, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Styled,
    Window, anchored, canvas, deferred, div, point, prelude::FluentBuilder, px,
};
use rimstack_core::say;
use std::cell::Cell;
use std::rc::Rc;

/// Where each hint's button was last drawn; empty while it is not shown.
#[derive(Default, Clone)]
pub struct Anchors {
    pub problems: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub sort: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub save: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub drawn: Rc<Cell<[bool; 3]>>,
}

impl Anchors {
    fn of(&self, step: u8) -> &Rc<Cell<Option<Bounds<Pixels>>>> {
        match step {
            1 => &self.problems,
            2 => &self.sort,
            _ => &self.save,
        }
    }

    pub fn forget_drawn(&self) {
        self.drawn.set([false; 3]);
    }

    fn mark(&self, at: &Rc<Cell<Option<Bounds<Pixels>>>>) {
        let mut drawn = self.drawn.get();
        for (nth, one) in [&self.problems, &self.sort, &self.save]
            .into_iter()
            .enumerate()
        {
            if Rc::ptr_eq(one, at) {
                drawn[nth] = true;
            }
        }
        self.drawn.set(drawn);
    }
}

pub(super) fn anchored_at(
    control: impl IntoElement,
    anchors: &Anchors,
    at: Rc<Cell<Option<Bounds<Pixels>>>>,
) -> gpui::Div {
    anchors.mark(&at);
    div().relative().child(control).child(
        canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .inset_0(),
    )
}

impl Mods {
    pub fn start_hints(&mut self, cx: &mut Context<Self>) {
        let seen = crate::ui::app(cx)
            .prefs()
            .flag("Appearance", "hintsSeen")
            .unwrap_or(false);
        if !seen && !self.demo {
            self.hints.step = 1;
            cx.notify();
        }
    }

    pub fn show_hint(&mut self, step: u8, cx: &mut Context<Self>) {
        self.hints.step = step.min(3);
        cx.notify();
    }

    fn end_hints(&mut self, cx: &mut Context<Self>) {
        self.hints.step = 0;
        crate::ui::keep_appearance("hintsSeen", "true", cx);
        cx.notify();
    }

    pub(super) fn next_hint(&mut self, cx: &mut Context<Self>) {
        match self.hints.step {
            3 => self.end_hints(cx),
            step => {
                self.hints.step = step + 1;
                cx.notify();
            }
        }
    }

    /// The hint due, under its button; one whose button is not shown passes to the next.
    pub(super) fn hint(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.hints.step == 0 {
            return None;
        }
        let step = self.hints.step;
        if !self.hints.anchors.drawn.get()[usize::from(step - 1)] {
            let list = self.list.read(cx);
            if list.files_read && list.overview.is_some() {
                cx.defer_in(window, move |mods, _, cx| {
                    if mods.hints.step == step {
                        mods.next_hint(cx);
                    }
                });
            }
            return None;
        }
        let Some(below) = self.hints.anchors.of(step).get() else {
            crate::ui::redraw_after(cx.entity_id(), cx);
            return None;
        };
        let c = &theme.colours;
        let at_right = step > 1;
        let wide = theme.sized(280.);
        let said = put(&match step {
            1 => say!(
                "The arrows beside it go from one mod with a problem to the next. Its details say what is wrong and offer the fix."
            ),
            2 => say!(
                "Sort shows the order the mods' rules ask for and what would move. Apply moves the mods; Discard leaves the list as it was."
            ),
            _ => say!(
                "Nothing reaches the game before Save. Until then every change can be undone or discarded."
            ),
        });
        let corner = match at_right {
            true => point(below.right() - wide, below.bottom() + px(9.)),
            false => point(below.left(), below.bottom() + px(9.)),
        };
        let accent = c.accent;
        let notch = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let mut path = gpui::PathBuilder::fill();
                let (mid, top) = (bounds.center().x, bounds.top());
                let half = bounds.size.width / 2.;
                path.move_to(point(mid, top));
                path.line_to(point(mid + half, top + half));
                path.line_to(point(mid - half, top + half));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, accent);
                }
            },
        )
        .absolute()
        .top(-theme::GAP_MEDIUM / 2.)
        .size(theme::GAP_MEDIUM)
        .when(at_right, |d| d.right(theme::GAP_WIDE))
        .when(!at_right, |d| d.left(theme::GAP_WIDE));
        Some(
            deferred(
                anchored().position(corner).child(
                    div()
                        .relative()
                        .w(wide)
                        .flex()
                        .flex_col()
                        .gap(theme::GAP_LARGE)
                        .px(theme::GAP_LARGE)
                        .py(theme::GAP_MEDIUM)
                        .rounded(theme::CORNER_LARGE)
                        .bg(accent)
                        .occlude()
                        .child(notch)
                        .child(
                            div()
                                .spaced_text(theme, theme.body(), 1.25)
                                .text_color(c.accent_ink)
                                .child(said),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .child(
                                    label(
                                        theme,
                                        theme.small(),
                                        c.accent_ink,
                                        format!("{step} / 3"),
                                    )
                                    .flex_1(),
                                )
                                .child(
                                    Btn::new("hint-skip", put(&say!("Skip")))
                                        .on_click(cx.listener(|m, _, _, cx| m.end_hints(cx))),
                                )
                                .child(
                                    Btn::new(
                                        "hint-next",
                                        put(&match step {
                                            3 => say!("Done"),
                                            _ => say!("Next"),
                                        }),
                                    )
                                    .on_click(cx.listener(|m, _, _, cx| m.next_hint(cx))),
                                ),
                        ),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}
