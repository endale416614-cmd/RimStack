mod area;
mod input;

use super::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App as Cx, Bounds, ClipboardItem, Context, ElementInputHandler, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Pixels, Point, Render, ShapedLine, SharedString, StatefulInteractiveElement,
    Styled, TextRun, UTF16Selection, Window, actions, canvas, div, fill, point, px, size,
};
use std::ops::Range;

actions!(
    text_field,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Copy,
        Cut,
        Escape,
        Enter,
        Up,
        Down
    ]
);

const CONTEXT: &str = "TextField";
const AREA: &str = "area";

/// A key held with Command on macOS and Ctrl elsewhere, as GPUI names it.
#[macro_export]
macro_rules! command {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

pub fn bind_keys(cx: &mut Cx) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("left", Left, Some(CONTEXT)),
        KeyBinding::new("right", Right, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        KeyBinding::new(crate::command!("a"), SelectAll, Some(CONTEXT)),
        KeyBinding::new("home", Home, Some(CONTEXT)),
        KeyBinding::new("end", End, Some(CONTEXT)),
        KeyBinding::new(crate::command!("v"), Paste, Some(CONTEXT)),
        KeyBinding::new(crate::command!("c"), Copy, Some(CONTEXT)),
        KeyBinding::new(crate::command!("x"), Cut, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("enter", Enter, Some(CONTEXT)),
        KeyBinding::new("up", Up, Some(AREA)),
        KeyBinding::new("down", Down, Some(AREA)),
    ]);
}

/// Told after each change of the text.
pub struct Edited;

pub struct Entered;

pub struct TextField {
    focus: FocusHandle,
    text: String,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    /// Where the text was last painted, written by the painter itself: written through the
    /// entity while drawing, gpui-fast takes the field as changed and draws again each view
    /// that read it, every frame.
    shaped: Painted<ShapedLine>,
    size: Pixels,
    /// Shown as bullets, as `TextInput.Password`: a key, which is not copied either.
    masked: bool,
    focused: std::cell::Cell<bool>,
    /// The rows a field of several lines shows at least; 0 for one line.
    area: usize,
    /// Esc leaves the text as it is, as in a field that holds what is kept rather than a filter.
    kept_on_escape: bool,
    wrapped: Painted<area::Wrapped>,
    rows_drawn: std::rc::Rc<std::cell::Cell<usize>>,
    /// Shown to be selected and copied, never typed in, as `CopyText` was.
    read_only: bool,
    weight: Option<gpui::FontWeight>,
    colour: Option<gpui::Hsla>,
    dragging: bool,
    spoken: gpui::SharedString,
}

type Painted<T> = std::rc::Rc<std::cell::RefCell<Option<(T, Bounds<Pixels>)>>>;

const BULLET: char = '\u{25cf}';

impl EventEmitter<Edited> for TextField {}
impl EventEmitter<Entered> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _: &Cx) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextField {
    pub fn new(size: Pixels, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            text: String::new(),
            selected: 0..0,
            reversed: false,
            marked: None,
            shaped: Default::default(),
            size,
            masked: false,
            focused: std::cell::Cell::new(false),
            area: 0,
            kept_on_escape: false,
            wrapped: Default::default(),
            rows_drawn: std::rc::Rc::new(std::cell::Cell::new(1)),
            read_only: false,
            weight: None,
            colour: None,
            dragging: false,
            spoken: gpui::SharedString::default(),
        }
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self.several_lines(1)
    }

    pub fn weight(mut self, weight: gpui::FontWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn colour(mut self, colour: gpui::Hsla) -> Self {
        self.colour = Some(colour);
        self
    }

    fn index_at(&self, at: Point<Pixels>) -> Option<usize> {
        if let Some((wrapped, bounds)) = &*self.wrapped.borrow() {
            return Some(wrapped.index_at(at - bounds.origin));
        }
        let shaped = self.shaped.borrow();
        let (shaped, bounds) = shaped.as_ref()?;
        Some(self.text_at(shaped.closest_index_for_x(at.x - bounds.left())))
    }

    fn word_at(&self, at: usize) -> Range<usize> {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let start = self.text[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(at, |(i, _)| i);
        let end = self.text[at..]
            .char_indices()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(at, |(i, c)| at + i + c.len_utf8());
        start..end
    }

    fn pressed(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let Some(at) = self.index_at(event.position) else {
            return;
        };
        match (event.click_count, event.modifiers.shift) {
            (2, _) => {
                self.selected = self.word_at(at);
                self.reversed = false;
                cx.notify();
            }
            (_, true) => self.select_to(at, cx),
            _ => self.move_to(at, cx),
        }
        self.dragging = event.click_count < 2;
    }

    fn moved(&mut self, event: &gpui::MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging || event.pressed_button != Some(gpui::MouseButton::Left) {
            self.dragging = false;
            return;
        }
        if let Some(at) = self.index_at(event.position) {
            self.select_to(at, cx);
        }
    }

    /// A field of several lines, `rows` tall at least, where Enter starts a line.
    pub fn several_lines(mut self, rows: usize) -> Self {
        self.area = rows.max(1);
        self.kept_on_escape = true;
        self
    }

    pub fn kept_on_escape(mut self) -> Self {
        self.kept_on_escape = true;
        self
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
    }

    fn shown_at(&self, at: usize) -> usize {
        match self.masked {
            true => self.text[..at].chars().count() * BULLET.len_utf8(),
            false => at,
        }
    }

    fn text_at(&self, shown: usize) -> usize {
        match self.masked {
            true => {
                let chars = shown / BULLET.len_utf8();
                self.text
                    .char_indices()
                    .nth(chars)
                    .map_or(self.text.len(), |(at, _)| at)
            }
            false => shown,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text = text.to_string();
        self.selected = self.text.len()..self.text.len();
        self.marked = None;
        cx.emit(Edited);
        cx.notify();
    }

    /// The text held, changed without telling: what a view shows of its own state.
    pub fn set_text_quietly(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text = text.to_string();
        self.selected = self.text.len()..self.text.len();
        self.marked = None;
        cx.notify();
    }

    /// Whether the keys were here when last drawn, for a listener that has no window.
    pub fn is_focused_now(&self) -> bool {
        self.focused.get()
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn move_to(&mut self, at: usize, cx: &mut Context<Self>) {
        self.selected = at..at;
        self.reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, at: usize, cx: &mut Context<Self>) {
        match self.reversed {
            true => self.selected.start = at,
            false => self.selected.end = at,
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        cx.notify();
    }

    fn previous(&self, at: usize) -> usize {
        self.text[..at]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self, at: usize) -> usize {
        self.text[at..]
            .chars()
            .next()
            .map_or(at, |c| at + c.len_utf8())
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let at = if self.selected.is_empty() {
            self.previous(self.cursor())
        } else {
            self.selected.start
        };
        self.move_to(at, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let at = if self.selected.is_empty() {
            self.next(self.cursor())
        } else {
            self.selected.end
        };
        self.move_to(at, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous(self.cursor()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next(self.cursor()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = 0..self.text.len();
        self.reversed = false;
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let at = match self.area {
            0 => 0,
            _ => self.text[..self.cursor()]
                .rfind('\n')
                .map_or(0, |at| at + 1),
        };
        self.move_to(at, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let at = match self.area {
            0 => self.text.len(),
            _ => self.text[self.cursor()..]
                .find('\n')
                .map_or(self.text.len(), |at| self.cursor() + at),
        };
        self.move_to(at, cx);
    }

    fn by_row(&mut self, rows: f32, cx: &mut Context<Self>) {
        let line = cx.global::<Theme>().line(self.size);
        let Some((at, to)) = self.wrapped.borrow().as_ref().map(|(wrapped, _)| {
            let at = wrapped.position(self.cursor());
            (
                at,
                wrapped.index_at(point(at.x, at.y + line * rows + line / 2.)),
            )
        }) else {
            return;
        };
        let to = if at.y + line * rows < px(0.) { 0 } else { to };
        self.move_to(to, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.by_row(-1., cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.by_row(1., cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return cx.propagate();
        }
        if self.selected.is_empty() {
            self.select_to(self.previous(self.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return cx.propagate();
        }
        if self.selected.is_empty() {
            self.select_to(self.next(self.cursor()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = match self.area {
                0 => text.replace('\n', " "),
                _ => text,
            };
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selected.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(&Copy, window, cx);
        if self.read_only {
            return;
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return cx.propagate();
        }
        match self.area {
            0 => cx.emit(Entered),
            _ => self.replace_text_in_range(None, "\n", window, cx),
        }
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if !self.kept_on_escape {
            self.set_text("", cx);
        }
    }

    fn run(&self, theme: &Theme, len: usize) -> TextRun {
        TextRun {
            len,
            font: gpui::Font {
                weight: self.weight.unwrap_or_default(),
                ..gpui::font(theme.family.clone())
            },
            color: self.colour.unwrap_or(theme.colours.ink),
            background_color: None,
            underline: self.marked.as_ref().map(|_| gpui::UnderlineStyle {
                thickness: px(1.),
                color: Some(theme.colours.ink),
                wavy: false,
            }),
            strikethrough: None,
        }
    }

    fn shape(&self, window: &Window, theme: &Theme) -> ShapedLine {
        let shown: String = match self.masked {
            true => self.text.chars().map(|_| BULLET).collect(),
            false => self.text.clone(),
        };
        let run = self.run(theme, shown.len());
        window
            .text_system()
            .shape_line(SharedString::from(shown), self.size, &[run], None)
    }
}

impl TextField {
    pub fn speak_as(&mut self, spoken: &str) {
        self.spoken = spoken.to_string().into();
    }

    pub fn spoken(&self) -> &str {
        &self.spoken
    }

    fn keyed(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("text-field")
            .role(match self.read_only {
                true => gpui::Role::Label,
                false => gpui::Role::TextInput,
            })
            .when(!self.spoken.is_empty(), |d| {
                d.aria_label(self.spoken.clone())
            })
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .cursor_text()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::pressed))
            .on_mouse_move(cx.listener(Self::moved))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|field, _, _, _| field.dragging = false),
            )
    }

    fn paint_area(&mut self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Canvas<()> {
        let (field, focus) = (cx.entity(), self.focus.clone());
        let (text, text_size, line, run) = (
            self.text.clone(),
            self.size,
            theme.line(self.size),
            self.run(theme, self.text.len()),
        );
        let (selected, cursor, focused) = (
            self.selected.clone(),
            self.cursor(),
            self.focused.get() && !self.read_only,
        );
        let (ink, accent) = (theme.colours.ink, theme.colours.accent);
        let (kept, rows_drawn) = (self.wrapped.clone(), self.rows_drawn.clone());
        canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                let wrapped =
                    area::Wrapped::shape(window, &text, text_size, line, run, bounds.size.width);
                let covered = match selected.is_empty() {
                    true => Vec::new(),
                    false => wrapped.cover(selected.start, selected.end, bounds.size.width),
                };
                for cover in covered {
                    window.paint_quad(fill(
                        Bounds::new(bounds.origin + cover.origin, cover.size),
                        accent,
                    ));
                }
                wrapped.paint(bounds.origin, window, cx);
                if focused && selected.is_empty() {
                    let at = bounds.origin + wrapped.position(cursor);
                    window.paint_quad(fill(Bounds::new(at, size(px(1.), line)), ink));
                }
                let rows = wrapped.rows();
                // Against what the last paint found: what the render saw goes stale while
                // gpui-fast keeps the field's render, and a refresh each frame would follow.
                if rows != rows_drawn.get() {
                    rows_drawn.set(rows);
                    crate::ui::redraw_after(field.entity_id(), cx);
                }
                *kept.borrow_mut() = Some((wrapped, bounds));
                window.handle_input(&focus, ElementInputHandler::new(bounds, field.clone()), cx);
            },
        )
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        if self.area > 0 {
            self.focused.set(self.focus.is_focused(window));
            let tall = theme.line(self.size) * self.area.max(self.rows_drawn.get()) as f32;
            let painted = self.paint_area(&theme, cx);
            return self
                .keyed(cx)
                .key_context(gpui::KeyContext::parse("TextField area").unwrap_or_default())
                .w_full()
                .h(tall)
                .child(painted.size_full());
        }
        let (field, kept) = (cx.entity(), self.shaped.clone());
        let focus = self.focus.clone();
        let shaped = self.shape(window, &theme);
        let (selected, cursor, focused) = (
            self.shown_at(self.selected.start)..self.shown_at(self.selected.end),
            self.shown_at(self.cursor()),
            focus.is_focused(window),
        );
        self.focused.set(focused);
        let (ink, accent) = (theme.colours.ink, theme.colours.accent);
        let line = theme.line(self.size);
        self.keyed(cx).key_context(CONTEXT).w_full().h(line).child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    let top = bounds.top() + (bounds.size.height - line) / 2.;
                    if !selected.is_empty() {
                        let from = bounds.left() + shaped.x_for_index(selected.start);
                        let to = bounds.left() + shaped.x_for_index(selected.end);
                        window.paint_quad(fill(
                            Bounds::from_corners(point(from, top), point(to, top + line)),
                            accent,
                        ));
                    }
                    let _ = shaped.paint(
                        point(bounds.left(), top),
                        line,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                    if focused && selected.is_empty() {
                        let x = bounds.left() + shaped.x_for_index(cursor);
                        window
                            .paint_quad(fill(Bounds::new(point(x, top), size(px(1.), line)), ink));
                    }
                    *kept.borrow_mut() = Some((shaped, bounds));
                    window.handle_input(
                        &focus,
                        ElementInputHandler::new(bounds, field.clone()),
                        cx,
                    );
                },
            )
            .size_full(),
        )
    }
}
