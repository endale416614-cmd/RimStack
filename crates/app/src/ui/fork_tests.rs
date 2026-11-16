//! What the views count on of gpui-fast 0.1.14 beyond upstream GPUI. A release that changes one
//! of these fails here before it shows as a view drawn too often, too seldom or never.

use gpui::{
    AppContext, Context, Entity, HeadlessAppContext, IntoElement, ParentElement, Render, Styled,
    Window, WindowHandle, div, px, size,
};
use std::sync::Arc;

#[derive(Default)]
struct Leaf {
    drawn: usize,
    /// Asks a redraw of itself while drawn, `window.refresh()` or `redraw_after`, this many times.
    refreshes: usize,
    after: usize,
}

impl Render for Leaf {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn += 1;
        if self.refreshes > 0 {
            self.refreshes -= 1;
            window.refresh();
        }
        if self.after > 0 {
            self.after -= 1;
            super::redraw_after(cx.entity_id(), cx);
        }
        div().size_full().child(self.drawn.to_string())
    }
}

struct Around {
    drawn: usize,
    leaf: Entity<Leaf>,
    cached: bool,
}

impl Render for Around {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.drawn += 1;
        let place = div().size_full();
        match self.cached {
            true => place.child(super::cached(self.leaf.clone())),
            false => place.child(self.leaf.clone()),
        }
    }
}

fn opened(cached: bool, leaf: Leaf) -> (HeadlessAppContext, WindowHandle<Around>) {
    let mut cx = HeadlessAppContext::with_asset_source(
        super::made_up::text_system(),
        Arc::new(super::assets::Assets),
    );
    let window = cx
        .open_window(size(px(200.), px(100.)), |_, cx| {
            let leaf = cx.new(|_| leaf);
            cx.new(|_| Around {
                drawn: 0,
                leaf,
                cached,
            })
        })
        .unwrap();
    draw(&mut cx, window);
    (cx, window)
}

fn draw(cx: &mut HeadlessAppContext, window: WindowHandle<Around>) {
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

fn drawn(cx: &mut HeadlessAppContext, window: WindowHandle<Around>) -> (usize, usize) {
    cx.update(|cx| {
        let around = window.read(cx).unwrap();
        (around.drawn, around.leaf.read(cx).drawn)
    })
}

fn leaf(cx: &mut HeadlessAppContext, window: WindowHandle<Around>) -> Entity<Leaf> {
    cx.update(|cx| window.read(cx).unwrap().leaf.clone())
}

#[test]
fn a_view_not_notified_is_not_drawn_again_with_the_view_around_it() {
    let (mut cx, window) = opened(false, Leaf::default());
    let before = drawn(&mut cx, window);
    cx.update(|cx| window.update(cx, |_, _, cx| cx.notify()).unwrap());
    draw(&mut cx, window);
    assert_eq!(drawn(&mut cx, window), (before.0 + 1, before.1));
}

#[test]
fn a_cached_view_updated_without_notice_is_not_drawn_again_with_the_view_around_it() {
    let (mut cx, window) = opened(true, Leaf::default());
    let leaf = leaf(&mut cx, window);
    let before = drawn(&mut cx, window);
    cx.update(|cx| {
        leaf.update(cx, |_, _| {});
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
    });
    draw(&mut cx, window);
    assert_eq!(drawn(&mut cx, window), (before.0 + 1, before.1));
}

#[test]
fn a_notified_view_is_drawn_again_alone() {
    let (mut cx, window) = opened(true, Leaf::default());
    let leaf = leaf(&mut cx, window);
    let before = drawn(&mut cx, window);
    cx.update(|cx| leaf.update(cx, |_, cx| cx.notify()));
    draw(&mut cx, window);
    assert_eq!(drawn(&mut cx, window), (before.0, before.1 + 1));
}

#[test]
fn a_refresh_asked_while_drawn_is_dropped_and_redraw_after_is_not() {
    let drawn_asking = |leaf: Leaf| {
        let (mut cx, window) = opened(true, leaf);
        draw(&mut cx, window);
        drawn(&mut cx, window).1
    };
    let plain = drawn_asking(Leaf::default());
    let refreshing = drawn_asking(Leaf {
        refreshes: 1,
        ..Leaf::default()
    });
    let after = drawn_asking(Leaf {
        after: 1,
        ..Leaf::default()
    });
    assert_eq!(refreshing, plain, "refresh() while drawn");
    assert_eq!(after, plain + 1, "redraw_after");
}

#[test]
fn a_task_dropped_never_finishes() {
    let mut cx = HeadlessAppContext::with_asset_source(
        super::made_up::text_system(),
        Arc::new(super::assets::Assets),
    );
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let told = finished.clone();
    cx.update(|cx| {
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(10));
        let task = cx.spawn(async move |_| {
            timer.await;
            told.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        drop(task);
    });
    cx.advance_clock(std::time::Duration::from_millis(20));
    cx.run_until_parked();
    assert!(!finished.load(std::sync::atomic::Ordering::SeqCst));
}
