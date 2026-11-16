//! What the other sections read of the load order: filled from the Mods tab each time it
//! changes, never while drawn, and told only when it changed. A section reads it while drawn, or
//! observes it, and needs no handle on the Mods tab.

use super::mods::ListState;
use crate::workspace::load_order::Overview;
use gpui::{App as Cx, AppContext, Entity, Global};
use rimstack_core::mods::library::ModId;
use std::rc::Rc;

#[derive(Default, Clone, PartialEq)]
pub struct Order {
    pub overview: Option<Rc<Overview>>,
    pub selected: Option<ModId>,
    pub active: Rc<Vec<ModId>>,
}

struct Held(Entity<Order>);

impl Global for Held {}

pub fn install(cx: &mut Cx) {
    let order = cx.new(|_| Order::default());
    cx.set_global(Held(order));
}

pub fn order(cx: &Cx) -> Entity<Order> {
    cx.global::<Held>().0.clone()
}

pub fn read(cx: &Cx) -> Order {
    order(cx).read(cx).clone()
}

pub fn from_list(list: &Entity<ListState>, cx: &mut Cx) {
    let list = list.read(cx);
    let fresh = Order {
        overview: list.overview(),
        selected: list.selected_id(),
        active: Rc::new(list.active_installed()),
    };
    order(cx).update(cx, |order, cx| {
        if *order != fresh {
            *order = fresh;
            cx.notify();
        }
    });
}
