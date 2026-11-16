use super::Mods;
use crate::workspace::load_order::Side;
use gpui::{Context, px};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftOff {
    #[default]
    Waiting,
    Restored,
    Off,
}

impl Mods {
    pub fn leave_off(&mut self) {
        self.left_off = LeftOff::Off;
    }

    /// The filter, the mod and its tab, and the scroll as they were left; "is:changed" is left
    /// out, as nothing is unsaved at the start.
    pub(super) fn restore_left_off(&mut self, cx: &mut Context<Self>) {
        if self.left_off != LeftOff::Waiting
            || self.demo
            || self.list.read(cx).count(Side::Active) == 0
        {
            return;
        }
        let view = self.view.clone();
        let kept: Vec<&str> = view
            .last_filter
            .split(' ')
            .filter(|term| !term.is_empty() && *term != "is:changed")
            .collect();
        if !kept.is_empty() {
            self.filter_by(&kept.join(" "), cx);
        }
        let app = super::super::app(cx);
        let found = (!view.last_mod.is_empty())
            .then(|| {
                [Side::Active, Side::Inactive].into_iter().find_map(|side| {
                    self.list
                        .read(cx)
                        .order
                        .with_package(&app, side, &view.last_mod)
                })
            })
            .flatten();
        if let Some(id) = found {
            self.show_mod(id, cx);
            let tab = view.last_tab.min(2);
            self.details_view.update(cx, |details, cx| {
                details.tab = tab;
                cx.notify();
            });
        }
        if found.is_none() {
            let top = px(view.last_scroll);
            self.list(Side::Active).update(cx, |list, cx| {
                list.scroll().borrow_mut().set_top(top);
                cx.notify();
            });
        }
        self.left_off = LeftOff::Restored;
    }

    /// The mod chosen alone, by package id; "" for none or several.
    pub(super) fn keep_left_mod(&mut self, cx: &mut Context<Self>) {
        if self.left_off != LeftOff::Restored {
            return;
        }
        let app = super::super::app(cx);
        let list = self.list.read(cx);
        let alone = match list.selection.as_slice() {
            [one] => Some(*one),
            _ => None,
        };
        let key = alone
            .and_then(|id| list.order.package_of(&app, id))
            .unwrap_or_default();
        self.keep_view(|view| view.last_mod = key, cx);
    }

    pub(super) fn keep_left_filter(&mut self, text: String, cx: &mut Context<Self>) {
        if self.left_off == LeftOff::Restored {
            self.keep_view(|view| view.last_filter = text, cx);
        }
    }

    pub(super) fn keep_left_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        if self.left_off == LeftOff::Restored {
            self.keep_view(|view| view.last_tab = tab, cx);
        }
    }

    pub fn keep_left_scroll(&mut self, cx: &mut Context<Self>) {
        if self.left_off != LeftOff::Restored {
            return;
        }
        let top = f32::from(self.list(Side::Active).read(cx).scroll().borrow().top());
        self.keep_view(|view| view.last_scroll = top, cx);
    }
}
