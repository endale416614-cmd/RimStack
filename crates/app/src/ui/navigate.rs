//! Where a view asks the window to go: another section, a mod in the load order, a list used.
//! Told to one router the shell hears, so a view needs no handle on the others and is never
//! updated by them while it updates.

use super::section::Section;
use gpui::{App as Cx, AppContext, Entity, EventEmitter, Global};
use rimstack_core::mods::library::ModId;

#[derive(Clone, Debug, PartialEq)]
pub enum Go {
    Section(Section),
    ShowMod(ModId),
    UseList(Vec<String>),
    /// The load order becomes these mods, unsaved, with the window left where it is.
    TakeList(Vec<String>),
    StartHints,
    Distrust(ModId),
    SearchedInMods,
    FindBroken,
    Browse(String),
    Subscribe(String),
    Redownload(ModId),
    Unsubscribe(ModId),
    UpdateFromGit(ModId),
    Problem(
        super::problem::Act,
        Box<crate::workspace::load_order::ProblemSaid>,
    ),
    /// The download list downloaded, subscribed to, or SteamCMD installed for it, each asked
    /// first.
    DownloadListed,
    SubscribeListed,
    InstallSteamcmd,
    FetchNotes(ModId),
}

pub struct Router;

impl EventEmitter<Go> for Router {}

struct Held(Entity<Router>);

impl Global for Held {}

pub fn install(cx: &mut Cx) {
    let router = cx.new(|_| Router);
    cx.set_global(Held(router));
}

pub fn router(cx: &Cx) -> Entity<Router> {
    cx.global::<Held>().0.clone()
}

/// Heard once the update asking it is done, as an event is.
pub fn go(to: Go, cx: &mut Cx) {
    router(cx).update(cx, |_, cx| cx.emit(to));
}
