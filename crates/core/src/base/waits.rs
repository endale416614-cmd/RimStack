//! Work that may hold its caller a long while: reading the mod folders, a database or a list
//! being read by another thread. Told to whoever watches, which the app uses to keep it off the
//! window's thread.

use std::sync::OnceLock;

static WATCH: OnceLock<fn(&str)> = OnceLock::new();

/// `told` hears of each wait from now on, on the thread that waits; set once.
pub fn watch(told: fn(&str)) {
    let _ = WATCH.set(told);
}

pub fn slow(what: &str) {
    if let Some(told) = WATCH.get() {
        told(what);
    }
}
