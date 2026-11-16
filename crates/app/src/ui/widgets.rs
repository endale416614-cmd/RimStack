use super::theme::{self, Theme};
use gpui::{
    AbsoluteLength, AnyElement, FontWeight, Hsla, IntoElement, ParentElement, Pixels, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px, svg,
};

pub trait TextSized: Styled + Sized {
    fn sized_text(self, theme: &Theme, size: Pixels) -> Self {
        self.text_size(size).line_height(theme.line(size))
    }

    /// A text whose lines are `factor` times as tall, the added room under each line: GPUI puts
    /// half of it above, so the text is raised by that half.
    fn spaced_text(self, theme: &Theme, size: Pixels, factor: f32) -> Self {
        let half = theme.line(size) * (factor - 1.) / 2.;
        self.text_size(size)
            .line_height(theme.line(size) * factor)
            .mt(-half)
            .mb(half)
    }

    fn border_width(mut self, width: Pixels) -> Self {
        let width: AbsoluteLength = width.into();
        let widths = &mut self.style().border_widths;
        (widths.top, widths.right, widths.bottom, widths.left) =
            (Some(width), Some(width), Some(width), Some(width));
        self
    }
}

impl<T: Styled> TextSized for T {}

pub fn icon(name: &str, size: Pixels, colour: Hsla) -> gpui::Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(size)
        .flex_none()
        .text_color(colour)
}

pub fn with_alpha(colour: Hsla, alpha: f32) -> Hsla {
    Hsla {
        a: colour.a * alpha,
        ..colour
    }
}

pub fn halo(tone: Hsla, corner: Pixels) -> impl Iterator<Item = AnyElement> {
    const RINGS: usize = 4;
    const STEP: f32 = 1.5;
    (0..RINGS).map(move |at| {
        let out = px(-(at as f32 + 1.) * STEP);
        div()
            .absolute()
            .top(out)
            .bottom(out)
            .left(out)
            .right(out)
            .rounded(corner + px((at as f32 + 1.) * STEP))
            .border_width(px(STEP))
            .border_color(with_alpha(tone, 0.3 * 0.62f32.powi(at as i32)))
            .into_any_element()
    })
}

pub use super::button::{Btn, Chip};

/// `text` that may break between any two characters: a path has
/// few places a line may break otherwise. The zero-width spaces between them draw nothing.
pub fn breakable(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 4);
    for (at, ch) in text.chars().enumerate() {
        if at > 0 {
            out.push('\u{200b}');
        }
        out.push(ch);
    }
    out
}

pub fn notice(tone: Hsla) -> gpui::Div {
    div()
        .relative()
        .rounded(theme::GAP_HAIR)
        .bg(with_alpha(tone, 0.06))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.))
                .bg(tone),
        )
}

pub fn measure(
    window: &Window,
    theme: &Theme,
    text: &str,
    size: Pixels,
    weight: FontWeight,
) -> Pixels {
    measure_in(window, theme, theme.family.clone(), text, size, weight)
}

pub fn measure_in(
    window: &Window,
    theme: &Theme,
    family: SharedString,
    text: &str,
    size: Pixels,
    weight: FontWeight,
) -> Pixels {
    if text.is_empty() {
        return Pixels::ZERO;
    }
    let run = |len: usize, weight: FontWeight| gpui::TextRun {
        len,
        font: gpui::Font {
            weight,
            ..gpui::font(family.clone())
        },
        color: theme.colours.ink,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut at = 0;
    for upright in lacking(window, &family, text, size, weight) {
        if upright.start > at {
            runs.push(run(upright.start - at, weight));
        }
        runs.push(run(upright.len(), FontWeight::NORMAL));
        at = upright.end;
    }
    if at < text.len() {
        runs.push(run(text.len() - at, weight));
    }
    window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), size, &runs, None)
        .width
}

/// Where `text` has a symbol (U+2100 to U+2BFF: arrows, operators, shapes) the family lacks at
/// `weight`. cosmic-text takes a fallback face only at the very weight asked, and the fonts
/// holding symbols (DejaVu Sans) have no medium or semibold face, so such a symbol would come
/// from an unrelated font; drawn at the normal weight it comes from the face fontconfig
/// takes. Scripts keep their weight: their fallbacks (Noto Sans CJK) have one.
fn lacking(
    window: &Window,
    family: &SharedString,
    text: &str,
    size: Pixels,
    weight: FontWeight,
) -> Vec<std::ops::Range<usize>> {
    let symbol = |ch: char| ('\u{2100}'..='\u{2bff}').contains(&ch);
    if weight == FontWeight::NORMAL || !text.chars().any(symbol) {
        return Vec::new();
    }
    let system = window.text_system();
    let id = system.resolve_font(&gpui::Font {
        weight,
        ..gpui::font(family.clone())
    });
    text.char_indices()
        .filter(|(_, ch)| symbol(*ch) && system.advance(id, size, *ch).is_err())
        .map(|(at, ch)| at..at + ch.len_utf8())
        .collect()
}

pub fn weighted(
    window: &Window,
    theme: &Theme,
    text: SharedString,
    size: Pixels,
    weight: FontWeight,
) -> gpui::StyledText {
    let upright = lacking(window, &theme.family, &text, size, weight);
    let normal = gpui::HighlightStyle {
        font_weight: Some(FontWeight::NORMAL),
        ..Default::default()
    };
    gpui::StyledText::new(text).with_highlights(upright.into_iter().map(|r| (r, normal)))
}

pub fn elide_middle(
    window: &Window,
    theme: &Theme,
    text: &str,
    size: Pixels,
    room: Pixels,
) -> String {
    elide_middle_in(window, theme, theme.family.clone(), text, size, room)
}

pub fn elide_middle_in(
    window: &Window,
    theme: &Theme,
    family: SharedString,
    text: &str,
    size: Pixels,
    room: Pixels,
) -> String {
    let fits = |shown: &str| {
        measure_in(
            window,
            theme,
            family.clone(),
            shown,
            size,
            FontWeight::NORMAL,
        ) <= room
    };
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let shown = |kept: usize| {
        let (left, right) = (kept.div_ceil(2), kept / 2);
        let start: String = chars[..left].iter().collect();
        let end: String = chars[chars.len() - right..].iter().collect();
        format!("{start}…{end}")
    };
    let (mut low, mut high) = (0, chars.len().saturating_sub(1));
    while low < high {
        let middle = (low + high).div_ceil(2);
        match fits(&shown(middle)) {
            true => low = middle,
            false => high = middle - 1,
        }
    }
    shown(low)
}

pub fn elide_start_in(
    window: &Window,
    theme: &Theme,
    family: SharedString,
    text: &str,
    size: Pixels,
    room: Pixels,
) -> String {
    let fits = |shown: &str| {
        measure_in(
            window,
            theme,
            family.clone(),
            shown,
            size,
            FontWeight::NORMAL,
        ) <= room
    };
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let shown = |kept: usize| {
        let end: String = chars[chars.len() - kept..].iter().collect();
        format!("…{end}")
    };
    let (mut low, mut high) = (0, chars.len().saturating_sub(1));
    while low < high {
        let middle = (low + high).div_ceil(2);
        match fits(&shown(middle)) {
            true => low = middle,
            false => high = middle - 1,
        }
    }
    shown(low)
}

/// A caption in capitals a pixel apart, as `font.capitalization: Font.AllUppercase` with
/// `font.letterSpacing: 1` drew it; GPUI has no spacing of its own, so each letter is placed.
pub fn caps(theme: &Theme, text: &str) -> gpui::Div {
    let shown = text.to_uppercase();
    let letters = shown.chars().map(|letter| div().child(letter.to_string()));
    watched(
        div()
            .flex()
            .flex_none()
            .overflow_hidden()
            .whitespace_nowrap()
            .sized_text(theme, theme.caption())
            .font_weight(theme.semibold)
            .text_color(theme.colours.ink3),
        SharedString::from(shown.clone()),
        div().flex().gap(theme::GAP_LINE / 2.).children(letters),
        false,
    )
}

pub fn empty_state(
    theme: &Theme,
    width: Pixels,
    icon_name: &str,
    title: String,
    hint: String,
) -> gpui::Div {
    let c = &theme.colours;
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(theme::GAP_SMALL)
        .w(width)
        .when(!icon_name.is_empty(), |d| {
            d.child(icon(icon_name, theme.sized(28.), c.ink3))
        })
        .child(
            div()
                .w_full()
                .text_center()
                .sized_text(theme, theme.title())
                .font_weight(theme.semibold)
                .text_color(c.ink)
                .child(title),
        )
        .when(!hint.is_empty(), |d| {
            d.child(
                div()
                    .w_full()
                    .text_center()
                    .spaced_text(theme, theme.body(), 1.3)
                    .text_color(c.ink2)
                    .child(hint),
            )
        })
}

pub fn chip_width(window: &Window, theme: &Theme, text: &str, icon: bool) -> Pixels {
    let shown = if icon {
        theme.sized(11.) + theme::GAP_TINY
    } else {
        Pixels::ZERO
    };
    shown + measure(window, theme, text, theme.caption(), theme.medium) + theme::GAP_CARD
}

pub fn btn_width(window: &Window, theme: &Theme, text: &str, icon: bool, menu: bool) -> Pixels {
    btn_width_of(window, theme, text, icon, menu, false)
}

pub fn btn_width_of(
    window: &Window,
    theme: &Theme,
    text: &str,
    icon: bool,
    menu: bool,
    large: bool,
) -> Pixels {
    let (size, weight) = match large {
        true => (theme.body(), theme.semibold),
        false => (theme.small(), FontWeight::NORMAL),
    };
    let parts = [
        icon.then(|| theme.icon()),
        (!text.is_empty()).then(|| measure(window, theme, text, size, weight)),
        menu.then(|| theme.sized(12.)),
    ];
    let shown: Vec<Pixels> = parts.into_iter().flatten().collect();
    let gaps = theme::GAP_SMALL * shown.len().saturating_sub(1) as f32;
    let room = match (large, text.is_empty() && !menu) {
        (true, _) => theme.sized(36.),
        (false, true) => theme.sized(14.),
        (false, false) => theme.sized(20.),
    };
    shown.into_iter().fold(gaps + room, |all, part| all + part)
}

pub fn label(
    theme: &Theme,
    size: Pixels,
    colour: Hsla,
    text: impl Into<SharedString>,
) -> gpui::Div {
    let text_div = div()
        .flex_none()
        .whitespace_nowrap()
        .sized_text(theme, size)
        .text_color(colour);
    let text: SharedString = text.into();
    watched(text_div, text.clone(), text, false)
}

/// A text in its box; under the layout checks it also tells where it went and how wide it is
/// whole, `may_cut` when its box cuts it on purpose.
pub fn watched(
    text_box: gpui::Div,
    text: SharedString,
    shown: impl IntoElement,
    may_cut: bool,
) -> gpui::Div {
    #[cfg(test)]
    {
        let said = text.clone();
        let seen = gpui::canvas(
            move |bounds, window, _| watch::saw(&said, bounds, may_cut, window),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        text_box.relative().child(shown).child(seen)
    }
    #[cfg(not(test))]
    {
        let _ = (text, may_cut);
        text_box.child(shown)
    }
}

#[cfg(test)]
pub mod watch {
    use gpui::{Bounds, Pixels, SharedString, Window};
    use std::cell::RefCell;

    pub struct Seen {
        pub text: SharedString,
        pub bounds: Bounds<Pixels>,
        pub whole: Pixels,
        pub may_cut: bool,
    }

    thread_local! {
        static SEEN: RefCell<Option<Vec<Seen>>> = const { RefCell::new(None) };
    }

    pub fn start() {
        SEEN.with_borrow_mut(|seen| *seen = Some(Vec::new()));
    }

    pub fn take() -> Vec<Seen> {
        SEEN.with_borrow_mut(|seen| seen.take().unwrap_or_default())
    }

    /// Only the part shown counts: a row half under the list's edge is cut there.
    pub(super) fn saw(
        text: &SharedString,
        bounds: Bounds<Pixels>,
        may_cut: bool,
        window: &mut Window,
    ) {
        if SEEN.with_borrow(|seen| seen.is_none()) || text.is_empty() {
            return;
        }
        let shown = bounds.intersect(&window.content_mask().bounds);
        if shown.size.width <= Pixels::ZERO || shown.size.height <= Pixels::ZERO {
            return;
        }
        let style = window.text_style();
        let size = style.font_size.to_pixels(window.rem_size());
        let run = style.to_run(text.len());
        let whole = window
            .text_system()
            .shape_line(text.clone(), size, &[run], None)
            .width;
        let found = Seen {
            text: text.clone(),
            bounds: shown,
            whole,
            may_cut,
        };
        SEEN.with_borrow_mut(|seen| seen.as_mut().map(|seen| seen.push(found)));
    }
}
