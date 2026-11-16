//! What changed in the `App`, told to whoever listens. A listener is called on the thread that
//! published, so a view's hands the news to the window's thread (`ui::hear`).

use rimstack_core::base::locks::Held;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changes(u64);

impl Changes {
    pub const NONE: Self = Self(0);
    /// Every `ModId` from before is void.
    pub const LIBRARY: Self = Self(1);
    pub const LOAD_ORDER: Self = Self(1 << 1);
    pub const PROBLEMS: Self = Self(1 << 2);
    pub const NOTES: Self = Self(1 << 3);
    pub const RULES: Self = Self(1 << 4);
    pub const SETTINGS: Self = Self(1 << 5);
    pub const LANGUAGE: Self = Self(1 << 6);
    pub const THEME: Self = Self(1 << 7);
    pub const COMMUNITY: Self = Self(1 << 8);
    pub const UPDATES: Self = Self(1 << 9);
    pub const TEXTURES: Self = Self(1 << 10);
    pub const FILES_READ: Self = Self(1 << 11);
    pub const LOGS: Self = Self(1 << 12);
    pub const SAVES: Self = Self(1 << 13);
    pub const LISTS: Self = Self(1 << 14);
    pub const LAUNCHES: Self = Self(1 << 16);
    pub const FOLDERS: Self = Self(1 << 17);
    pub const FIND: Self = Self(1 << 19);
    pub const FACTS: Self = Self(1 << 20);
    pub const DOWNLOAD_LIST: Self = Self(1 << 21);
    pub const KEY: Self = Self(1 << 22);
    pub const SOURCE: Self = Self(1 << 23);
    pub const ALL: Self = Self(u64::MAX);

    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn and(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn contains(self, any_of: Self) -> bool {
        self.0 & any_of.0 != 0
    }

    pub fn within(self, set: Self) -> bool {
        self.0 & !set.0 == 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Changes {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOrAssign for Changes {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

type Listener = Arc<dyn Fn(Changes) -> bool + Send + Sync>;

#[derive(Default)]
pub struct Bus {
    listeners: Mutex<Vec<Listener>>,
}

impl Bus {
    pub fn listen(&self, listener: impl Fn(Changes) + Send + Sync + 'static) {
        self.listen_while(move |changes| {
            listener(changes);
            true
        });
    }

    /// Told until it answers false, as one whose view has closed does.
    pub fn listen_while(&self, listener: impl Fn(Changes) -> bool + Send + Sync + 'static) {
        self.listeners.held().push(Arc::new(listener));
    }

    #[cfg(test)]
    pub fn listening(&self) -> usize {
        self.listeners.held().len()
    }

    /// Listeners are called outside the lock: one may listen, or publish, in turn.
    pub fn publish(&self, changes: Changes) {
        if changes.is_empty() {
            return;
        }
        let listeners = self.listeners.held().clone();
        let gone: Vec<Listener> = listeners
            .into_iter()
            .filter(|listener| !listener(changes))
            .collect();
        if !gone.is_empty() {
            self.listeners
                .held()
                .retain(|listener| !gone.iter().any(|left| Arc::ptr_eq(left, listener)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn every_listener_hears_each_change_and_nothing_is_not_told() {
        let bus = Bus::default();
        let heard = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..2 {
            let heard = heard.clone();
            bus.listen(move |changes| heard.held().push(changes));
        }
        bus.publish(Changes::NONE);
        bus.publish(Changes::NOTES | Changes::PROBLEMS);
        let heard = heard.held();
        assert_eq!(heard.len(), 2);
        assert!(heard[0].contains(Changes::PROBLEMS) && !heard[0].contains(Changes::LIBRARY));
        assert!(heard[0].within(Changes::NOTES | Changes::PROBLEMS | Changes::LOGS));
        assert!(!heard[0].within(Changes::NOTES));
    }

    #[test]
    fn a_listener_that_is_done_is_told_no_more_and_one_may_publish_in_turn() {
        let bus = Arc::new(Bus::default());
        let told = Arc::new(Mutex::new(0));
        let counted = told.clone();
        bus.listen_while(move |changes| {
            *counted.held() += usize::from(changes.contains(Changes::NOTES));
            false
        });
        let again = bus.clone();
        bus.listen(move |changes| {
            if changes.contains(Changes::NOTES) {
                again.publish(Changes::RULES);
            }
        });
        bus.publish(Changes::NOTES);
        bus.publish(Changes::NOTES);
        assert_eq!(*told.held(), 1);
    }
}
