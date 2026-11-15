use super::focus;
use super::theme::{self, Theme};
use super::tip::tipped;
use super::widgets::{TextSized, halo, icon, watched, weighted, with_alpha};
use gpui::{
    App as Cx, Bounds, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder,
};
use std::cell::Cell;
use std::rc::Rc;

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut Cx)>;

#[derive(IntoElement)]
pub struct Btn {
    id: ElementId,
    text: SharedString,
    icon: Option<&'static str>,
    icon_tone: Option<Hsla>,
    primary: bool,
    large: bool,
    menu: bool,
    enabled: bool,
    danger: bool,
    tone: Option<Hsla>,
    height: Option<Pixels>,
    tip: SharedString,
    spoken: Option<SharedString>,
    on_click: Option<Click>,
    placed: Option<Rc<Cell<Bounds<Pixels>>>>,
    fill: bool,
}

impl Btn {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            icon: None,
            icon_tone: None,
            primary: false,
            large: false,
            menu: false,
            enabled: true,
            danger: false,
            tone: None,
            height: None,
            tip: SharedString::default(),
            spoken: None,
            on_click: None,
            placed: None,
            fill: false,
        }
    }

    /// Shown when the pointer rests on it; an icon alone shows its words instead when it has none.
    pub fn tip(mut self, tip: impl Into<SharedString>) -> Self {
        self.tip = tip.into();
        self
    }

    pub fn spoken(mut self, spoken: impl Into<SharedString>) -> Self {
        self.spoken = Some(spoken.into());
        self
    }

    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    pub fn icon_tone(mut self, tone: Hsla) -> Self {
        self.icon_tone = Some(tone);
        self
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn primary_if(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    pub fn menu(mut self) -> Self {
        self.menu = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn tone(mut self, tone: Hsla) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    pub fn on_click(mut self, run: impl Fn(&ClickEvent, &mut Window, &mut Cx) + 'static) -> Self {
        self.on_click = Some(Box::new(run));
        self
    }

    /// A button that opens a menu: `run` is given where the button is, to open it under.
    pub fn opens(self, run: impl Fn(Bounds<Pixels>, &mut Window, &mut Cx) + 'static) -> Self {
        let mut shown = self.pops(run);
        shown.menu = true;
        shown
    }

    /// A button that opens a popup with no menu mark: `run` is given where the button is.
    pub fn pops(mut self, run: impl Fn(Bounds<Pixels>, &mut Window, &mut Cx) + 'static) -> Self {
        let at: Rc<Cell<Bounds<Pixels>>> = Rc::default();
        self.placed = Some(at.clone());
        self.on_click = Some(Box::new(move |_, window, cx| run(at.get(), window, cx)));
        self
    }
}

impl RenderOnce for Btn {
    fn render(self, window: &mut Window, cx: &mut Cx) -> impl IntoElement {
        let handle = focus::handle(self.id.clone(), window, cx);
        let (pointed, keep_pointed) = super::motion::hovered(&self.id, window, cx);
        let theme = cx.global::<Theme>();
        let c = &theme.colours;
        let toned = self.tone.filter(|_| !self.primary);
        let ink = match (self.primary, toned) {
            (true, _) => c.accent_ink,
            (false, Some(tone)) => tone,
            (false, None) if self.danger && self.enabled => c.err,
            (false, None) => c.ink,
        };
        let padding = match (self.large, self.text.is_empty() && !self.menu) {
            (true, _) => theme.sized(36.),
            (false, true) => theme.sized(14.),
            (false, false) => theme.sized(20.),
        };
        let (ground, hovered) = match (self.primary, toned) {
            (true, _) => (c.accent, c.accent_hi),
            (false, Some(tone)) => (with_alpha(tone, 0.18), with_alpha(tone, 0.3)),
            (false, None) => (c.ctl, c.ctl_hi),
        };
        let frame = if self.enabled { 1. } else { 0.4 };
        let size = if self.large {
            theme.body()
        } else {
            theme.small()
        };
        let weight = match (self.large, self.primary || toned.is_some()) {
            (true, _) => theme.semibold,
            (false, true) => theme.medium,
            (false, false) => FontWeight::NORMAL,
        };
        let height = self.height.unwrap_or(if self.large {
            theme.control_large()
        } else {
            theme.control()
        });
        let line = toned.unwrap_or(c.field_line);
        let spoken = self.spoken.clone().unwrap_or_else(|| {
            match (self.text.is_empty(), self.tip.is_empty()) {
                (false, _) => self.text.clone(),
                (true, false) => self.tip.clone(),
                (true, true) => self.icon.unwrap_or_default().into(),
            }
        });
        let bare_text = (self.text.is_empty() && self.tip.is_empty()).then(|| self.text.clone());
        let tip = bare_text.unwrap_or_else(|| self.tip.clone());
        let control = div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(spoken)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(theme::GAP_SMALL)
            .when(self.fill, |d| d.w_full())
            .h(height)
            .px(padding / 2.)
            .rounded(theme::CORNER)
            .bg(with_alpha(ground, frame))
            .when(self.enabled, |d| {
                d.on_hover(keep_pointed).child(super::motion::layer(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(theme::CORNER)
                        .bg(super::motion::over(ground, hovered)),
                    pointed,
                ))
            })
            .when(theme.appearance.high_contrast, |d| {
                d.border_1().border_color(with_alpha(line, frame))
            })
            .when(self.primary && self.enabled, |d| {
                d.children(halo(c.accent, theme::CORNER))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP_SMALL)
                    .when(!self.enabled, |d| d.opacity(0.5))
                    .when_some(self.icon, |d, name| {
                        d.child(icon(name, theme.icon(), self.icon_tone.unwrap_or(ink)))
                    })
                    .when(!self.text.is_empty(), |d| {
                        let shown = weighted(window, theme, self.text.clone(), size, weight);
                        d.child(watched(
                            div()
                                .whitespace_nowrap()
                                .sized_text(theme, size)
                                .font_weight(weight)
                                .text_color(ink),
                            self.text,
                            shown,
                            false,
                        ))
                    })
                    .when(self.menu, |d| {
                        d.child(icon("chevron-down", theme.sized(12.), with_alpha(ink, 0.8)))
                    }),
            )
            .when_some(self.on_click.filter(|_| self.enabled), |d, run| {
                d.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    run(event, window, cx);
                })
            });
        let control = control.when_some(self.placed, |d, at| {
            d.child(
                gpui::canvas(move |bounds, _, _| at.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
        });
        let control = match self.enabled {
            true => focus::reachable(control, &handle, theme::CORNER, false, window, theme),
            false => control,
        };
        tipped(control, tip)
    }
}

#[derive(IntoElement)]
pub struct Chip {
    id: Option<ElementId>,
    tip: SharedString,
    on_click: Option<Click>,
    text: SharedString,
    tone: Option<Hsla>,
    icon: Option<&'static str>,
    on_accent: bool,
    pressable: bool,
    max_width: Option<Pixels>,
    held: bool,
}

impl Chip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            tip: SharedString::default(),
            on_click: None,
            text: text.into(),
            tone: None,
            icon: None,
            on_accent: false,
            pressable: false,
            max_width: None,
            held: false,
        }
    }

    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    pub fn on_accent(mut self, on: bool) -> Self {
        self.on_accent = on;
        self
    }

    pub fn pressable(mut self) -> Self {
        self.pressable = true;
        self
    }

    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn tone(mut self, tone: Hsla) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn tip(mut self, tip: impl Into<SharedString>) -> Self {
        self.tip = tip.into();
        self
    }

    pub fn on_click(
        mut self,
        id: impl Into<ElementId>,
        run: impl Fn(&ClickEvent, &mut Window, &mut Cx) + 'static,
    ) -> Self {
        self.id = Some(id.into());
        self.pressable = true;
        self.on_click = Some(Box::new(run));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, window: &mut Window, cx: &mut Cx) -> impl IntoElement {
        let id = self
            .id
            .clone()
            .unwrap_or_else(|| ElementId::Name(self.text.clone()));
        let handle = self
            .on_click
            .as_ref()
            .map(|_| focus::handle(id.clone(), window, cx));
        let pointed = self
            .on_click
            .as_ref()
            .map(|_| super::motion::hovered(&id, window, cx));
        let theme = cx.global::<Theme>();
        let tone = self.tone.unwrap_or(theme.colours.ink2);
        let ink = if self.on_accent {
            theme.colours.on_selected
        } else {
            tone
        };
        let (ground, hovered) = if self.on_accent {
            (theme::hex("#38000000"), theme::hex("#38000000"))
        } else {
            (with_alpha(tone, 0.16), with_alpha(tone, 0.28))
        };
        let height = if self.pressable {
            theme.control()
        } else {
            theme.tag()
        };
        let spoken = if self.text.is_empty() {
            self.tip.clone()
        } else {
            self.text.clone()
        };
        let pressed = self.on_click.is_some();
        let changed = super::motion::colour_change(&id, ground, window);
        let control = div()
            .id(id)
            .role(if pressed { Role::Button } else { Role::Label })
            .aria_label(spoken)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::GAP_TINY)
            .h(height)
            .px(theme::GAP_CARD / 2.)
            .rounded(height / 2.)
            .bg(ground)
            .when_some(changed, |d, (before, turn)| {
                d.child(super::motion::colour_fading(
                    div().absolute().inset_0().rounded(height / 2.).bg(before),
                    turn,
                ))
            })
            .when_some(pointed, |d, (pointed, keep_pointed)| {
                d.cursor_pointer()
                    .on_hover(keep_pointed)
                    .child(super::motion::layer(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(height / 2.)
                            .bg(super::motion::over(ground, hovered)),
                        pointed,
                    ))
            })
            .when(self.held, |d| d.border_2().border_color(tone))
            .when_some(self.max_width, |d, most| d.max_w(most))
            .when_some(self.icon, |d, name| {
                d.child(icon(name, theme.sized(11.), ink))
            })
            .when(!self.text.is_empty(), |d| {
                let shown = weighted(
                    window,
                    theme,
                    self.text.clone(),
                    theme.caption(),
                    theme.medium,
                );
                d.child(watched(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.caption())
                        .font_weight(theme.medium)
                        .text_color(ink),
                    self.text,
                    shown,
                    true,
                ))
            })
            .when_some(self.on_click, |d, run| {
                d.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    run(event, window, cx);
                })
            });
        let control = match handle {
            Some(handle) => {
                focus::reachable(control, &handle, height / 2., self.on_accent, window, theme)
            }
            None => control,
        };
        tipped(control, self.tip)
    }
}
