//! Counts one view finds and others show. A view that reads them while it is drawn is drawn
//! again when they change: gpui-fast follows the entity it read.

use gpui::{App as Cx, AppContext, Context, Entity, Global};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counts {
    pub behind: usize,
    pub changed_since: usize,
}

struct Held(Entity<Counts>);

impl Global for Held {}

pub fn install(cx: &mut Cx) {
    let counts = cx.new(|_| Counts::default());
    cx.set_global(Held(counts));
}

pub fn counts(cx: &Cx) -> Entity<Counts> {
    cx.global::<Held>().0.clone()
}

pub fn read(cx: &Cx) -> Counts {
    *counts(cx).read(cx)
}

/// Told only when it changes, so nothing is drawn again for the same counts.
pub fn change<V>(cx: &mut Context<V>, change: impl FnOnce(&mut Counts)) {
    counts(cx).update(cx, |counts, cx| {
        let was = *counts;
        change(counts);
        if *counts != was {
            cx.notify();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, ParentElement, Render, Styled, Window, div};
    use std::sync::Arc;

    struct Reader {
        reads: bool,
        drawn: usize,
    }

    impl Render for Reader {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.drawn += 1;
            let said = match self.reads {
                true => read(cx).behind.to_string(),
                false => String::new(),
            };
            div().size_full().child(said)
        }
    }

    struct Both {
        reading: Entity<Reader>,
        not: Entity<Reader>,
    }

    impl Render for Both {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(self.reading.clone())
                .child(self.not.clone())
        }
    }

    #[test]
    fn a_change_draws_again_the_views_that_read_the_counts_alone() {
        let mut cx = gpui::HeadlessAppContext::with_asset_source(
            crate::ui::made_up::text_system(),
            Arc::new(crate::ui::assets::Assets),
        );
        cx.update(install);
        let window = cx
            .open_window(gpui::size(gpui::px(200.), gpui::px(100.)), |_, cx| {
                let reading = cx.new(|_| Reader {
                    reads: true,
                    drawn: 0,
                });
                let not = cx.new(|_| Reader {
                    reads: false,
                    drawn: 0,
                });
                cx.new(|_| Both { reading, not })
            })
            .unwrap();
        let draw = |cx: &mut gpui::HeadlessAppContext| {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
        };
        draw(&mut cx);
        let drawn = |cx: &mut gpui::HeadlessAppContext| {
            cx.update(|cx| {
                let both = window.read(cx).unwrap();
                (both.reading.read(cx).drawn, both.not.read(cx).drawn)
            })
        };
        let before = drawn(&mut cx);
        cx.update(|cx| {
            let counts = counts(cx);
            counts.update(cx, |counts, cx| {
                counts.behind = 3;
                cx.notify();
            });
        });
        draw(&mut cx);
        assert_eq!(drawn(&mut cx), (before.0 + 1, before.1));
    }
}
