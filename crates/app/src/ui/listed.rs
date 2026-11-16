//! What the other sections show of Downloads: the download list and the change notes fetched.
//! Filled by Downloads each time it changes, never while drawn, and told only when it changed; a
//! view reads it while drawn, and asks Downloads for anything through `navigate::go`.

use crate::workspace::changed::ChangeNotes;
use crate::workspace::download_list::ListShown;
use gpui::{App as Cx, AppContext, Entity, Global};
use std::rc::Rc;

#[derive(Default, Clone, PartialEq)]
pub struct Listed {
    pub list: Rc<ListShown>,
    pub local_mods: String,
    pub steamcmd: bool,
    /// By Workshop id.
    pub notes: Rc<ChangeNotes>,
    pub notes_busy: bool,
    pub notes_label: String,
}

struct Held(Entity<Listed>);

impl Global for Held {}

pub fn install(cx: &mut Cx) {
    let listed = cx.new(|_| Listed::default());
    cx.set_global(Held(listed));
}

pub fn listed(cx: &Cx) -> Entity<Listed> {
    cx.global::<Held>().0.clone()
}

pub fn read(cx: &Cx) -> Listed {
    listed(cx).read(cx).clone()
}

pub fn change(cx: &mut Cx, fresh: Listed) {
    listed(cx).update(cx, |listed, cx| {
        if *listed != fresh {
            *listed = fresh;
            cx.notify();
        }
    });
}
