use super::theme::{self, Theme};
use super::widgets::{Btn, TextSized};
use crate::words::put;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, Global, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::say;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How long a toast stays: longer when it offers something.
const SHORT: Duration = Duration::from_secs(5);
const LONG: Duration = Duration::from_secs(8);
/// At most this many at once; an offer replaces the one before.
const SHOWN: usize = 4;
const KEPT: usize = 40;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Plain,
    Ok,
    Err,
}

type Run = Rc<dyn Fn(&mut Window, &mut App)>;

struct Toast {
    key: usize,
    text: SharedString,
    tone: Tone,
    offer: Option<(SharedString, Run)>,
    /// When it goes, unless the pointer is on it then.
    until: Instant,
    held: bool,
}

pub struct Earlier {
    pub text: SharedString,
    pub tone: Tone,
    pub time: u64,
}

pub struct Notices {
    shown: Vec<Toast>,
    pub earlier: Vec<Earlier>,
    counted: usize,
    ticking: bool,
}

struct Told(Entity<Notices>);

impl Global for Told {}

/// This run's notices, newest first: their words, whether they told a failure, and when.
pub fn earlier(cx: &App) -> Vec<(SharedString, bool, u64)> {
    cx.try_global::<Told>().map_or_else(Vec::new, |told| {
        told.0
            .read(cx)
            .earlier
            .iter()
            .map(|e| (e.text.clone(), matches!(e.tone, Tone::Err), e.time))
            .collect()
    })
}

pub fn install(cx: &mut App) -> Entity<Notices> {
    let notices = cx.new(|_| Notices {
        shown: Vec::new(),
        earlier: Vec::new(),
        counted: 0,
        ticking: false,
    });
    cx.set_global(Told(notices.clone()));
    notices
}

fn notices(cx: &App) -> Option<Entity<Notices>> {
    cx.try_global::<Told>().map(|told| told.0.clone())
}

pub fn say(text: impl Into<SharedString>, tone: Tone, cx: &mut App) {
    if let Some(notices) = notices(cx) {
        notices.update(cx, |notices, cx| notices.add(text.into(), tone, None, cx));
    }
}

pub fn offer(
    text: impl Into<SharedString>,
    label: impl Into<SharedString>,
    run: impl Fn(&mut Window, &mut App) + 'static,
    cx: &mut App,
) {
    if let Some(notices) = notices(cx) {
        let offer = Some((label.into(), Rc::new(run) as Run));
        notices.update(cx, |notices, cx| {
            notices.add(text.into(), Tone::Plain, offer, cx)
        });
    }
}

impl Notices {
    fn add(
        &mut self,
        text: SharedString,
        tone: Tone,
        offer: Option<(SharedString, Run)>,
        cx: &mut Context<Self>,
    ) {
        self.counted += 1;
        if offer.is_some() {
            self.shown.retain(|toast| toast.offer.is_none());
        }
        let stays = if offer.is_some() { LONG } else { SHORT };
        self.shown.push(Toast {
            key: self.counted,
            text: text.clone(),
            tone,
            offer,
            until: cx.background_executor().now() + stays,
            held: false,
        });
        let over = self.shown.len().saturating_sub(SHOWN);
        self.shown.drain(..over);
        self.earlier.insert(
            0,
            Earlier {
                text,
                tone,
                time: crate::workspace::now_secs(),
            },
        );
        self.earlier.truncate(KEPT);
        self.tick(cx);
        cx.notify();
    }

    fn dismiss(&mut self, key: usize, cx: &mut Context<Self>) {
        self.shown.retain(|toast| toast.key != key);
        cx.notify();
    }

    /// While toasts show, a look every quarter second for those whose time is up.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |notices, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let going = notices.update(cx, |notices, cx| {
                    let now = cx.background_executor().now();
                    let before = notices.shown.len();
                    notices
                        .shown
                        .retain(|toast| toast.held || toast.until > now);
                    if notices.shown.len() != before {
                        cx.notify();
                    }
                    notices.ticking = !notices.shown.is_empty();
                    notices.ticking
                });
                if !matches!(going, Ok(true)) {
                    return;
                }
            }
        })
        .detach();
    }

    fn toast(&self, at: usize, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let toast = &self.shown[at];
        let key = toast.key;
        let tone: Hsla = match toast.tone {
            Tone::Err => c.err,
            Tone::Ok => c.ok,
            Tone::Plain => c.moved,
        };
        let stays = if toast.offer.is_some() { LONG } else { SHORT };
        div()
            .id(("toast", key))
            .role(Role::Alert)
            .aria_label(toast.text.clone())
            .relative()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .max_w(theme.sized(420.))
            .min_h(theme.sized(40.))
            .py(theme::GAP_MEDIUM)
            .pl(theme::GAP_CARD)
            .pr(theme::GAP)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .child(
                div()
                    .absolute()
                    .left(px(1.))
                    .top(theme::GAP)
                    .bottom(theme::GAP)
                    .w(px(3.))
                    .rounded(px(1.5))
                    .bg(tone),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .max_w(theme.sized(300.))
                    .sized_text(theme, theme.body())
                    .text_color(c.ink)
                    .child(toast.text.clone()),
            )
            .when_some(toast.offer.clone(), |d, (label, run)| {
                d.child(Btn::new(("toast-offer", key), label).on_click(cx.listener(
                    move |notices, _, window, cx| {
                        run(window, cx);
                        notices.dismiss(key, cx);
                    },
                )))
            })
            .child(
                Btn::new(("toast-dismiss", key), "")
                    .icon("x")
                    .tip(put(&say!("Dismiss")))
                    .on_click(cx.listener(move |notices, _, _, cx| notices.dismiss(key, cx))),
            )
            .on_hover(cx.listener(move |notices, hovered: &bool, _, cx| {
                let now = cx.background_executor().now();
                if let Some(toast) = notices.shown.iter_mut().find(|toast| toast.key == key) {
                    toast.held = *hovered;
                    toast.until = now + stays;
                }
            }))
            .into_any_element()
    }
}

impl Render for Notices {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let toasts: Vec<AnyElement> = (0..self.shown.len())
            .map(|at| self.toast(at, &theme, cx))
            .collect();
        div()
            .absolute()
            .left(theme::GAP_CARD)
            .bottom(theme.strip() + theme::GAP_LARGE)
            .flex()
            .flex_col()
            .items_start()
            .gap(theme::GAP_SMALL)
            .children(toasts)
    }
}

#[cfg(test)]
impl Notices {
    pub(crate) fn shown(&self) -> Vec<SharedString> {
        self.shown.iter().map(|toast| toast.text.clone()).collect()
    }
}
