//! the mature-content choice changes. No record that reaches the window, a log or the history holds it.

use super::secret::Vault;
use super::{App, Written, written};
use crate::changes::Changes;
use rimstack_core::base::locks::Held;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::settings::Switch;
use rimstack_core::rimworld::steamapi::{ApiKey, Call};
use rimstack_core::rimworld::workshopindex;
use rimstack_core::say;
use rimstack_core::sources::browse::{Browse, Failure, KeyCheck, Steam, Transport, check_key};
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Watch {
    inner: Arc<dyn Transport>,
    refused: AtomicBool,
    last: Mutex<Option<Failure>>,
}

impl Watch {
    pub fn new(inner: Arc<dyn Transport>) -> Self {
        Watch {
            inner,
            refused: AtomicBool::new(false),
            last: Mutex::new(None),
        }
    }

    fn refused(&self) -> bool {
        self.refused.load(Ordering::Relaxed)
    }

    fn forgive(&self) {
        self.refused.store(false, Ordering::Relaxed);
        self.forget();
    }

    fn forget(&self) {
        *self.last.held() = None;
    }

    fn take(&self) -> Option<Failure> {
        self.last.held().take()
    }
}

impl Transport for Watch {
    fn call(&self, call: Call, args: &Value, key: &ApiKey) -> Result<String, Failure> {
        let answer = self.inner.call(call, args, key);
        if let Err(failure) = &answer {
            if matches!(failure, Failure::Status(401 | 403)) {
                self.refused.store(true, Ordering::Relaxed);
            }
            *self.last.held() = Some(*failure);
        }
        answer
    }
}

type Stamp = (Written, bool);

/// What the last ask of the store gave: `None` is the cache.
#[derive(Default)]
pub struct Chosen(Option<(Stamp, Option<Arc<dyn Browse>>)>);

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyState {
    pub stored: bool,
    pub available: bool,
    pub refused: bool,
    pub note: Option<Sentence>,
}

pub fn refusal() -> Sentence {
    say!("The Steam key was refused; searching the downloaded database instead")
}

pub fn unreachable() -> Sentence {
    say!("Steam could not be reached; searching the downloaded database instead")
}

impl App {
    pub fn with_vault(self, vault: Arc<dyn Vault>) -> Self {
        Self { vault, ..self }
    }

    pub fn with_transport(self, transport: Arc<dyn Transport>) -> Self {
        Self {
            watch: Arc::new(Watch::new(transport.clone())),
            transport,
            ..self
        }
    }

    pub fn show_mature(&self) -> bool {
        self.settings().switch(Switch::ShowMature)
    }

    fn key_kept(&self) -> bool {
        self.settings().switch(Switch::SteamKeyKept)
    }

    fn mark_key(&self, kept: bool) {
        let value = if kept { "on" } else { "off" };
        if let Err(problem) = self.set_setting(Switch::SteamKeyKept.name(), value) {
            rimstack_core::problem!("steam key flag: {problem}");
        }
    }

    pub fn key_state(&self) -> KeyState {
        let refused = self.watch.refused();
        let (stored, available, note) = match self.key_kept().then(|| self.vault.get()) {
            None => (false, true, None),
            Some(Ok(held)) => {
                if held.is_none() {
                    self.mark_key(false);
                }
                (held.is_some(), true, None)
            }
            Some(Err(why)) => (false, false, Some(why)),
        };
        KeyState {
            stored,
            available,
            refused: refused && stored,
            note: match (&note, refused && stored) {
                (None, true) => Some(refusal()),
                _ => note,
            },
        }
    }

    pub fn check_and_keep(&self, typed: &str) -> Result<(), Sentence> {
        let Some(key) = ApiKey::new(typed) else {
            return Err(say!("Type the key first"));
        };
        match check_key(&*self.transport, &key) {
            KeyCheck::Valid => {}
            KeyCheck::Invalid => return Err(say!("Steam refused this key")),
            KeyCheck::Unreachable => {
                return Err(say!(
                    "Steam could not be reached to check the key, so it was not kept"
                ));
            }
        }
        self.vault.set(key.reveal())?;
        self.mark_key(true);
        self.key_changed();
        Ok(())
    }

    pub fn forget_key(&self) -> Result<(), Sentence> {
        self.vault.delete()?;
        self.mark_key(false);
        self.key_changed();
        Ok(())
    }

    fn key_changed(&self) {
        self.watch.forgive();
        self.keyed.held().0 = None;
        *self.browse_details.held() = Default::default();
        self.publish(Changes::KEY | Changes::SOURCE);
    }

    fn index_stamp(&self) -> Written {
        written(workshopindex::file_in(&self.data_dir))
    }

    pub(super) fn keyed_source(&self) -> Option<Arc<dyn Browse>> {
        if self.watch.refused() || !self.key_kept() {
            return None;
        }
        let stamp = (self.index_stamp(), self.show_mature());
        if let Some((was, source)) = &self.keyed.held().0
            && *was == stamp
        {
            return source.clone();
        }
        let source = self.steam_source(stamp.1);
        self.keyed.held().0 = Some((stamp, source.clone()));
        source
    }

    fn steam_source(&self, show_mature: bool) -> Option<Arc<dyn Browse>> {
        let held = match self.vault.get().ok()? {
            Some(held) => held,
            None => {
                self.mark_key(false);
                return None;
            }
        };
        let key = ApiKey::new(&held)?;
        if self.fixture {
            return Some(Arc::new(Steam::fixture()));
        }
        let index = self.database_there().then(|| self.workshop_index());
        let transport: Arc<dyn Transport> = self.watch.clone();
        Some(Arc::new(Steam::new(key, transport, index, show_mature)))
    }

    pub fn key_mode(&self) -> bool {
        self.keyed_source().is_some()
    }

    pub(super) fn steam_failure(&self) -> Option<Failure> {
        self.watch.take()
    }

    pub(super) fn steam_unasked(&self) {
        self.watch.forget();
    }

    pub(super) fn steam_refusal_told(&self, failure: Failure) -> Sentence {
        match failure {
            Failure::Status(401 | 403) => {
                self.publish(Changes::KEY);
                refusal()
            }
            _ => unreachable(),
        }
    }
}
