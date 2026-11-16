use super::Find;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::measure_in;
use gpui::{FontWeight, Pixels, Window, px};

#[derive(Clone, Copy)]
pub(super) struct Layout {
    /// Five digits of the monospace family at the small size: a line number's column.
    pub digits: Pixels,
    pub preview_width: Pixels,
    pub wide: bool,
    pub side_shown: bool,
    pub side_fills: bool,
    pub side_width: Pixels,
    pub preview_shown: bool,
    pub results_shown: bool,
    pub middle: Pixels,
}

/// What is shown, given the room left once the filters and a line of matches are laid side by
/// side (`room`, below zero when they do not fit), the preview's width at its fullest and at its
/// narrowest, whether there is a preview, and whether the filters are asked for.
fn arranged(
    room: Pixels,
    full_preview: Pixels,
    least_preview: Pixels,
    preview: bool,
    filters: bool,
) -> (bool, bool, bool, Option<Pixels>) {
    let wide = room >= if preview { least_preview } else { px(0.) };
    let side_shown = wide || filters;
    let side_fills = side_shown && room < px(0.);
    let preview_width = match (wide, filters) {
        (true, _) => Some(full_preview.min(room)),
        (false, false) => Some(full_preview),
        (false, true) => None,
    };
    (
        wide,
        side_shown,
        side_fills,
        preview_width.filter(|_| preview),
    )
}

impl Find {
    pub(super) fn layout(&self, theme: &Theme, window: &Window) -> Layout {
        let width = window.viewport_size().width;
        let digits = measure_in(
            window,
            theme,
            theme.mono.clone(),
            "00000",
            theme.small(),
            FontWeight::NORMAL,
        );
        let full_preview = (width * 0.36).min(theme.sized(620.));
        let line_width = digits * 16.;
        let side_width = theme.sized(250.);
        let room = width - side_width - line_width - theme::GAP_LARGE * 4.;
        let (wide, side_shown, side_fills, preview) = arranged(
            room,
            full_preview,
            full_preview.min(theme.sized(360.)),
            self.preview.is_some(),
            self.filters,
        );
        let results_shown = !side_fills;
        let preview_shown = preview.is_some() && results_shown;
        let preview_width = preview.unwrap_or(full_preview);
        let middle = width
            - theme::GAP_LARGE
            - if side_shown {
                side_width + theme::GAP_LARGE
            } else {
                px(0.)
            }
            - if preview_shown {
                preview_width + theme::GAP_LARGE
            } else {
                px(0.)
            };
        Layout {
            digits,
            preview_width,
            wide,
            side_shown,
            side_fills,
            side_width,
            preview_shown,
            results_shown,
            middle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::arranged;
    use gpui::px;

    #[test]
    fn the_filters_stay_beside_the_matches_while_both_fit() {
        let (full, least) = (px(500.), px(400.));
        assert_eq!(
            arranged(px(450.), full, least, true, false),
            (true, true, false, Some(px(450.)))
        );
        assert_eq!(
            arranged(px(100.), full, least, false, false),
            (true, true, false, None)
        );
        assert_eq!(
            arranged(px(100.), full, least, true, false),
            (false, false, false, Some(full))
        );
        assert_eq!(
            arranged(px(100.), full, least, true, true),
            (false, true, false, None)
        );
        assert_eq!(
            arranged(px(-1.), full, least, true, true),
            (false, true, true, None)
        );
    }
}
