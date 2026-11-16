use super::theme::{self, Theme};
use super::widgets::{TextSized, with_alpha};
use gpui::{
    AnyElement, App, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveElement,
    InteractiveText, IntoElement, ParentElement, Pixels, StrikethroughStyle, Styled, StyledText,
    UnderlineStyle, Window, div, px,
};
use rimstack_core::base::markup::{Block, Kind, Run};
use std::ops::Range;
use std::rc::Rc;

pub type Link = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// Where a press started and where the pointer is now, each a block and a byte in its words.
#[derive(Default)]
struct Picked {
    from: (usize, usize),
    to: (usize, usize),
    held: bool,
    /// Each block's words as last drawn: a selection of other words is let go.
    texts: Vec<String>,
}

impl Picked {
    fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        (self.from.min(self.to), self.from.max(self.to))
    }

    fn within(&self, at: usize, len: usize) -> Option<Range<usize>> {
        let (start, end) = self.ordered();
        if start == end || at < start.0 || at > end.0 {
            return None;
        }
        let from = if at == start.0 { start.1 } else { 0 };
        let to = if at == end.0 { end.1 } else { len };
        (from < to).then_some(from..to.min(len))
    }

    fn said(&self) -> String {
        let (start, end) = self.ordered();
        (start.0..=end.0)
            .filter_map(|at| {
                let text = self.texts.get(at)?;
                self.within(at, text.len()).and_then(|part| text.get(part))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A description's words selected with the mouse and copied with Ctrl + C, as `CopyText` let
/// them be; `view` is drawn again as the selection moves.
#[derive(Clone)]
pub struct Selection {
    picked: Rc<std::cell::RefCell<Picked>>,
    focus: gpui::FocusHandle,
    view: gpui::EntityId,
}

impl Selection {
    pub fn new<V: 'static>(cx: &mut gpui::Context<V>) -> Self {
        Self {
            picked: Rc::default(),
            focus: cx.focus_handle(),
            view: cx.entity_id(),
        }
    }

    pub fn said(&self) -> String {
        self.picked.borrow().said()
    }

    pub fn holding(&self, blocks: gpui::Div) -> gpui::Stateful<gpui::Div> {
        let (copied, released) = (self.picked.clone(), self.picked.clone());
        blocks
            .id("selectable")
            .track_focus(&self.focus)
            .on_key_down(move |event: &gpui::KeyDownEvent, _, cx| {
                let keys = &event.keystroke;
                if keys.key == "c" && keys.modifiers.secondary(cx) && !keys.modifiers.alt {
                    let said = copied.borrow().said();
                    if !said.is_empty() {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(said));
                        cx.stop_propagation();
                    }
                }
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_, _, _| {
                released.borrow_mut().held = false
            })
    }
}

fn indent(theme: &Theme) -> Pixels {
    theme.sized(40.)
}

fn styled(theme: &Theme, run: &Run) -> HighlightStyle {
    let c = &theme.colours;
    let line = |colour| UnderlineStyle {
        thickness: px(1.),
        color: Some(colour),
        wavy: false,
    };
    HighlightStyle {
        color: run.link.as_ref().map(|_| c.focus),
        font_weight: run.bold.then_some(FontWeight::BOLD),
        font_style: run.italic.then_some(FontStyle::Italic),
        underline: match (run.link.is_some(), run.underline) {
            (true, _) => Some(line(c.focus)),
            (false, true) => Some(line(c.ink)),
            (false, false) => None,
        },
        strikethrough: run.strike.then_some(StrikethroughStyle {
            thickness: px(1.),
            color: Some(c.ink),
        }),
        ..HighlightStyle::default()
    }
}

fn words(
    id: ElementId,
    theme: &Theme,
    (at, block): (usize, &Block),
    link: &Link,
    selection: Option<&Selection>,
) -> AnyElement {
    let mut text = String::new();
    let mut marks = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    for run in &block.runs {
        let start = text.len();
        text += &run.text;
        let range = start..text.len();
        let style = styled(theme, run);
        if style != HighlightStyle::default() {
            marks.push((range.clone(), style));
        }
        if let Some(url) = &run.link {
            links.push((range, url.clone()));
        }
    }
    let chosen = selection.and_then(|s| s.picked.borrow().within(at, text.len()));
    let marked = HighlightStyle {
        background_color: Some(with_alpha(theme.colours.accent, 0.45)),
        ..HighlightStyle::default()
    };
    let marks: Vec<_> =
        gpui::combine_highlights(marks, chosen.map(|part| (part, marked))).collect();
    let shown = StyledText::new(text).with_highlights(marks);
    let laid = shown.layout().clone();
    let words = match links.is_empty() {
        true => shown.into_any_element(),
        false => {
            let (ranges, urls): (Vec<_>, Vec<_>) = links.into_iter().unzip();
            let link = link.clone();
            InteractiveText::new(id, shown)
                .on_click(ranges, move |at, window, cx| link(&urls[at], window, cx))
                .into_any_element()
        }
    };
    let Some(selection) = selection.cloned() else {
        return words;
    };
    let (pressed, moved) = (selection.clone(), selection);
    let (pressed_at, moved_at) = (laid.clone(), laid);
    div()
        .child(words)
        .on_mouse_down(gpui::MouseButton::Left, move |event, window, cx| {
            let byte = pressed_at
                .index_for_position(event.position)
                .unwrap_or_else(|nearest| nearest);
            let mut picked = pressed.picked.borrow_mut();
            (picked.from, picked.to, picked.held) = ((at, byte), (at, byte), true);
            window.focus(&pressed.focus, cx);
            cx.notify(pressed.view);
        })
        .on_mouse_move(move |event, _, cx| {
            let mut picked = moved.picked.borrow_mut();
            if !picked.held || !event.dragging() {
                return;
            }
            let byte = moved_at
                .index_for_position(event.position)
                .unwrap_or_else(|nearest| nearest);
            if picked.to != (at, byte) {
                picked.to = (at, byte);
                cx.notify(moved.view);
            }
        })
        .into_any_element()
}

pub fn blocks(
    id: &'static str,
    theme: &Theme,
    blocks: &[Block],
    link: &Link,
    selection: Option<&Selection>,
) -> Vec<AnyElement> {
    let c = &theme.colours;
    if let Some(selection) = selection {
        let texts: Vec<String> = blocks
            .iter()
            .map(|block| block.runs.iter().map(|run| run.text.as_str()).collect())
            .collect();
        let mut picked = selection.picked.borrow_mut();
        if picked.texts != texts {
            *picked = Picked {
                texts,
                ..Picked::default()
            };
        }
    }
    blocks
        .iter()
        .enumerate()
        .map(|(at, block)| {
            let shown = words((id, at).into(), theme, (at, block), link, selection);
            let text = div()
                .w_full()
                .sized_text(theme, theme.body())
                .text_color(c.ink);
            match block.kind {
                Kind::Paragraph => text.child(shown),
                Kind::Heading(level) => {
                    let size = match level {
                        1 => theme.body() * 1.2,
                        _ => theme.body(),
                    };
                    text.sized_text(theme, size)
                        .font_weight(FontWeight::BOLD)
                        .pt(theme::GAP_SMALL)
                        .child(shown)
                }
                Kind::Item { depth, number } => {
                    let mark = match (number, depth) {
                        (Some(n), _) => format!("{n}."),
                        (None, 1) => "•".into(),
                        (None, 2) => "◦".into(),
                        (None, _) => "▪".into(),
                    };
                    text.flex()
                        .pl(indent(theme) * (depth - 1) as f32)
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .justify_end()
                                .w(indent(theme))
                                .pr(theme::GAP)
                                .child(mark),
                        )
                        .child(div().flex_1().min_w_0().child(shown))
                }
                Kind::Quote => text.pl(indent(theme)).pr(indent(theme)).child(shown),
                Kind::Code => text
                    .font_family(theme.mono.clone())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(shown),
                Kind::Rule => div()
                    .w_full()
                    .py(theme::GAP_SMALL)
                    .child(div().h(px(1.)).bg(with_alpha(c.ink3, 0.6))),
            }
            .into_any_element()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Picked;

    #[test]
    fn a_selection_across_blocks_says_each_part_on_its_own_line() {
        let picked = Picked {
            from: (2, 3),
            to: (0, 6),
            held: false,
            texts: vec!["First block".into(), "Middle".into(), "Last one".into()],
        };
        assert_eq!(picked.said(), "block\nMiddle\nLas");
        assert_eq!(picked.within(1, 6), Some(0..6));
        let none = Picked {
            from: (1, 2),
            to: (1, 2),
            ..picked
        };
        assert_eq!(none.said(), "", "a press alone selects nothing");
    }
}
