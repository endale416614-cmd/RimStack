use super::Downloads;
use crate::ui::divider::{Ghost, SETTLED, divider, nudged};
use crate::ui::notices;
use crate::ui::theme::{self, Theme};
use gpui::{
    AppContext, ClickEvent, Context, InteractiveElement, KeyDownEvent, Pixels,
    StatefulInteractiveElement, Window,
};

#[derive(Clone, Copy)]
pub(super) struct SideDivider;

pub(super) fn first_side(theme: &Theme) -> Pixels {
    theme.sized(360.)
}

/// The side pane's width in a window `whole` wide, or None when it goes under the page: it
/// shows once the window holds three of its first width, and keeps the page two of them.
pub(super) fn side_width(
    whole: Pixels,
    wanted: Pixels,
    first: Pixels,
    least: Pixels,
) -> Option<Pixels> {
    let most = whole - first * 2. - theme::HANDLE;
    (whole >= first * 3.).then(|| wanted.min(most).max(least))
}

impl Downloads {
    pub(super) fn side_shown(&self, theme: &Theme, window: &Window) -> Option<Pixels> {
        side_width(
            window.viewport_size().width,
            self.side,
            first_side(theme),
            theme.sized(260.),
        )
    }

    pub(super) fn set_side(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.side == width {
            return;
        }
        self.side = width;
        if !self.demo {
            let kept = f32::from(width);
            self.side_kept.after(SETTLED, cx, move |_, cx| {
                if let Err(problem) = crate::ui::app(cx).keep_downloads_side(kept) {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx);
                }
            });
        }
        cx.notify();
    }

    pub(super) fn nudge_side(
        &mut self,
        event: &KeyDownEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let theme = cx.global::<Theme>();
        let (Some(by), Some(now)) = (nudged(theme, event), self.side_shown(theme, window)) else {
            return;
        };
        cx.stop_propagation();
        self.set_side(now - by, cx);
    }

    pub(super) fn side_divider(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        divider(theme, "downloads-divider", &self.side_keys, window)
            .on_key_down(cx.listener(|d, event, window, cx| d.nudge_side(event, window, cx)))
            .on_drag(SideDivider, |_, _, _, cx| cx.new(|_| Ghost))
            .on_click(cx.listener(|d, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    d.set_side(first_side(cx.global::<Theme>()), cx);
                }
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::side_width;
    use gpui::px;

    #[test]
    fn the_side_pane_keeps_its_least_leaves_the_page_two_first_widths_and_goes_under_it_when_narrow()
     {
        let (first, least) = (px(400.), px(250.));
        let most = px(800.) - crate::ui::theme::HANDLE;
        assert_eq!(side_width(px(1600.), px(100.), first, least), Some(least));
        assert_eq!(side_width(px(1600.), px(900.), first, least), Some(most));
        assert_eq!(side_width(px(1199.), px(300.), first, least), None);
    }
}
