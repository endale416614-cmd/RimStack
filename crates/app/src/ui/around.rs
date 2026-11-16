//! What the window around the sections shows of them: what stands in Play's way, the list's name
//! and whether it is saved, and the work under way. Filled from the sections each time one of
//! them changes, never while drawn; the bars read it while drawn, so they are drawn again only
//! when what they show changed.

use super::mods::ListState;
use gpui::{App as Cx, AppContext, Entity, Global};

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Around {
    /// What stands in Play's way, worded, in the order the popup lists it.
    pub in_the_way: Vec<String>,
    pub errors: bool,
    pub sort_pending: bool,
    pub dirty: bool,
    pub unsaved: Option<String>,
    /// The kept list the load order came from, empty when none: read at the start, then told
    /// by `Lists` as it keeps it.
    pub list_name: String,
    pub mods_work: Vec<String>,
    pub downloads_work: Vec<String>,
    pub community_busy: bool,
    pub textures_work: Option<String>,
}

struct Held(Entity<Around>);

impl Global for Held {}

pub fn install(cx: &mut Cx) {
    let list_name = super::app(cx)
        .prefs()
        .get("Mods", "listName")
        .unwrap_or_default();
    let around = cx.new(|_| Around {
        list_name,
        ..Around::default()
    });
    cx.set_global(Held(around));
}

pub fn around(cx: &Cx) -> Entity<Around> {
    cx.global::<Held>().0.clone()
}

pub fn read(cx: &Cx) -> Around {
    around(cx).read(cx).clone()
}

/// Told only when it changes, so nothing is drawn again for the same.
pub fn change(cx: &mut Cx, change: impl FnOnce(&mut Around)) {
    around(cx).update(cx, |around, cx| {
        let was = around.clone();
        change(around);
        if *around != was {
            cx.notify();
        }
    });
}

pub fn from_list(list: &Entity<ListState>, cx: &mut Cx) {
    let list = list.read(cx);
    let (errors, sort_pending, dirty, unsaved) = list.standing();
    let in_the_way = list.in_the_way_said(errors, sort_pending, dirty);
    let work = list.work_said();
    change(cx, |around| {
        around.in_the_way = in_the_way;
        around.errors = errors;
        around.sort_pending = sort_pending;
        around.dirty = dirty;
        around.unsaved = unsaved;
        around.mods_work = work;
    });
}
