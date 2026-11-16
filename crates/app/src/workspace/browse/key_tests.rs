use super::tests::asked;
use super::*;
use crate::workspace::key::{refusal, unreachable};
use crate::workspace::secret::{Memory, Vault};
use crate::workspace::tests::installed;
use rimstack_core::kept::settings::Switch;
use rimstack_core::rimworld::steamapi::{ApiKey, Call};
use rimstack_core::rimworld::workshopsearch::Kind;
use rimstack_core::say;
use rimstack_core::sources::browse::COLLECTION;
use rimstack_core::sources::browse::HARMONY;
use rimstack_core::sources::browse::{Canned, Failure, Transport};
use serde_json::Value;
use std::sync::Mutex;

const SECRET: &str = "ABCDEF0123456789SECRETKEY";

struct Steamlike {
    fail: Mutex<Option<Failure>>,
    asked: Mutex<Vec<Value>>,
}

impl Steamlike {
    fn new() -> Arc<Self> {
        Arc::new(Steamlike {
            fail: Mutex::new(None),
            asked: Mutex::new(Vec::new()),
        })
    }

    fn failing(&self, with: Option<Failure>) {
        *self.fail.lock().unwrap() = with;
    }
}

impl Transport for Steamlike {
    fn call(&self, call: Call, args: &Value, key: &ApiKey) -> Result<String, Failure> {
        self.asked.lock().unwrap().push(args.clone());
        if let Some(failure) = *self.fail.lock().unwrap() {
            return Err(failure);
        }
        Canned.call(call, args, key)
    }
}

fn keyed() -> (tempfile::TempDir, App, Arc<Steamlike>) {
    let (tmp, app) = installed();
    let steam = Steamlike::new();
    let app = app.with_transport(steam.clone());
    (tmp, app, steam)
}

fn titles(app: &App, text: &str) -> Vec<String> {
    let found = app.browse_search(&asked(text, Kind::Mod, false), 0, 50);
    found.unwrap().rows.into_iter().map(|r| r.title).collect()
}

#[test]
fn a_key_steam_accepts_is_kept_and_searches_go_through_steam() {
    let (_tmp, app, steam) = keyed();
    assert!(!app.key_mode() && app.browse_defaults().sorts.is_empty());
    assert!(titles(&app, "harmony").is_empty(), "no database, no key");

    app.check_and_keep(&format!("  {SECRET}\n")).unwrap();
    let state = app.key_state();
    assert!(state.stored && state.available && !state.refused && state.note.is_none());
    assert!(app.key_mode());
    assert!(
        titles(&app, "harmony")
            .iter()
            .any(|t| t.contains("Harmony"))
    );
    let defaults = app.browse_defaults();
    assert!(defaults.key_mode && defaults.ready);
    assert_eq!(defaults.sorts.len(), 6);
    assert!(!steam.asked.lock().unwrap().is_empty());
}

#[test]
fn a_key_steam_refuses_or_cannot_check_is_not_kept() {
    let (_tmp, app, steam) = keyed();
    steam.failing(Some(Failure::Status(403)));
    assert_eq!(
        app.check_and_keep(SECRET).unwrap_err(),
        say!("Steam refused this key")
    );
    steam.failing(Some(Failure::Unreachable));
    assert!(app.check_and_keep(SECRET).is_err());
    assert!(app.check_and_keep("   ").is_err());
    assert!(!app.key_state().stored && !app.key_mode());
}

#[test]
fn forgetting_the_key_returns_to_the_cache() {
    let (_tmp, app, _) = keyed();
    app.check_and_keep(SECRET).unwrap();
    assert!(app.key_mode());
    app.forget_key().unwrap();
    assert!(!app.key_state().stored && !app.key_mode());
    app.forget_key().unwrap();
}

#[test]
fn a_store_that_does_not_work_makes_key_mode_unavailable_with_a_reason() {
    let (_tmp, app, _) = keyed();
    let why = say!("No credential store is available to keep the key in");
    app.keep_setting("steamKeyKept", "on").unwrap();
    let app = app.with_vault(Arc::new(Memory::broken(why.clone())));
    let state = app.key_state();
    assert!(!state.stored && !state.available);
    assert_eq!(state.note, Some(why.clone()));
    assert_eq!(app.check_and_keep(SECRET).unwrap_err(), why);
    assert!(!app.key_mode());
    assert!(
        app.browse_search(&asked("x", Kind::Mod, false), 0, 5)
            .is_ok()
    );
}

#[test]
fn the_fixture_with_a_key_kept_uses_the_made_up_steam() {
    let (_tmp, app) = installed();
    let app = app.browsing_fixture(true);
    assert!(!app.key_mode());
    assert!(app.browse_details(HARMONY).unwrap().votes_said.is_none());
    app.check_and_keep(SECRET).unwrap();
    assert!(app.key_mode());
    let sorted = Asked {
        sort: sort_of("subscribed"),
        ..asked("", Kind::Mod, false)
    };
    assert!(!app.browse_search(&sorted, 0, 10).unwrap().rows.is_empty());
    let with_votes = app.browse_details(HARMONY).unwrap();
    assert!(with_votes.votes_said.is_some());
}

#[test]
fn a_key_refused_while_browsing_falls_back_once_and_stays_on_the_cache() {
    let (_tmp, app, steam) = keyed();
    app.check_and_keep(SECRET).unwrap();
    steam.failing(Some(Failure::Status(401)));
    let found = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert_eq!(found.notice, Some(refusal()));
    assert!(!app.key_mode());
    let state = app.key_state();
    assert!(state.stored && state.refused);
    assert_eq!(state.note, Some(refusal()));

    let again = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert!(again.notice.is_none(), "said once");

    steam.failing(None);
    app.check_and_keep(SECRET).unwrap();
    assert!(app.key_mode() && !app.key_state().refused);
}

#[test]
fn a_key_refused_on_a_details_call_is_noticed_too() {
    let (_tmp, app, steam) = keyed();
    app.check_and_keep(SECRET).unwrap();
    steam.failing(Some(Failure::Status(403)));
    let _ = app.browse_details("3100000002");
    assert!(app.key_state().refused && !app.key_mode());
}

#[test]
fn adding_a_collection_adds_its_items_and_a_mod_only_itself() {
    let (_tmp, app) = installed();
    let app = app.browsing_fixture(true);
    app.check_and_keep(SECRET).unwrap();
    app.add_to_list(COLLECTION, false);
    let mut listed = app.listed_ids();
    listed.sort();
    assert_eq!(listed, ["3100000001", "3100000003"]);
    app.add_to_list(HARMONY, false);
    assert_eq!(app.listed_ids().len(), 3);
}

#[test]
fn steam_out_of_reach_sends_that_search_to_the_cache_only() {
    let (_tmp, app, steam) = keyed();
    app.check_and_keep(SECRET).unwrap();
    steam.failing(Some(Failure::Unreachable));
    let found = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert_eq!(found.notice, Some(unreachable()));
    assert!(app.key_mode() && !app.key_state().refused);
    steam.failing(None);
    let found = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert!(found.notice.is_none() && !found.rows.is_empty());
}

#[test]
fn showing_mature_mods_rebuilds_the_source_and_leaves_nothing_out() {
    let (_tmp, app, steam) = keyed();
    app.check_and_keep(SECRET).unwrap();
    let excluded = |steam: &Steamlike| {
        let asked = steam.asked.lock().unwrap();
        asked
            .last()
            .unwrap()
            .get("excluded_content_descriptors")
            .cloned()
    };
    assert!(!app.show_mature());
    titles(&app, "harmony");
    assert!(excluded(&steam).is_some());
    app.keep_setting("showMature", "on").unwrap();
    assert!(app.show_mature() && app.settings_report().show_mature);
    titles(&app, "harmony");
    assert!(excluded(&steam).is_none());
    app.keep_setting("showMature", "off").unwrap();
    titles(&app, "harmony");
    assert!(excluded(&steam).is_some());
}

#[test]
fn nothing_shown_holds_the_key() {
    let (_tmp, app, steam) = keyed();
    let app = app.browsing_fixture(false);
    app.check_and_keep(SECRET).unwrap();
    let mut shown = vec![
        format!("{:?}", app.key_state()),
        serde_json::to_string(&app.key_state()).unwrap(),
        format!("{:?}", app.browse_defaults()),
        serde_json::to_string(&app.settings_report()).unwrap(),
        serde_json::to_string(&app.list_shown()).unwrap(),
    ];
    let found = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    shown.push(serde_json::to_string(&found).unwrap());
    if let Some(row) = found.rows.first() {
        let details = app.browse_details(&row.id);
        shown.push(serde_json::to_string(&details).unwrap());
    }
    steam.failing(Some(Failure::Status(403)));
    let refused = app.browse_search(&asked("x", Kind::Mod, false), 0, 10);
    shown.push(format!("{refused:?}"));
    shown.push(format!("{:?}", app.check_and_keep(SECRET)));
    shown.push(format!("{:?}", app.key_state()));
    for text in shown {
        assert!(!text.contains("SECRETKEY"), "{text}");
    }
    let held: Option<String> = Memory::holding(SECRET).get().unwrap();
    assert_eq!(
        held.as_deref(),
        Some(SECRET),
        "the memory store does keep it"
    );
}

struct Counting {
    inner: Memory,
    asked: std::sync::atomic::AtomicUsize,
}

impl Counting {
    fn new(inner: Memory) -> Arc<Self> {
        Arc::new(Counting {
            inner,
            asked: Default::default(),
        })
    }

    fn asked(&self) -> usize {
        self.asked.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn count(&self) {
        self.asked
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Vault for Counting {
    fn get(&self) -> Result<Option<String>, rimstack_core::base::sentence::Sentence> {
        self.count();
        self.inner.get()
    }

    fn set(&self, secret: &str) -> Result<(), rimstack_core::base::sentence::Sentence> {
        self.count();
        self.inner.set(secret)
    }

    fn delete(&self) -> Result<(), rimstack_core::base::sentence::Sentence> {
        self.count();
        self.inner.delete()
    }
}

#[test]
fn the_credential_store_is_not_touched_before_a_key_was_kept() {
    let (_tmp, app, _) = keyed();
    let vault = Counting::new(Memory::holding(SECRET));
    let app = app.with_vault(vault.clone());
    let state = app.key_state();
    assert!(!state.stored && state.available && state.note.is_none());
    assert!(!app.key_mode());
    let defaults = app.browse_defaults();
    assert!(!defaults.key_mode);
    titles(&app, "harmony");
    assert_eq!(vault.asked(), 0);
}

#[test]
fn a_key_kept_is_read_again_and_a_key_deleted_outside_clears_the_flag() {
    let (_tmp, app, _) = keyed();
    let vault = Counting::new(Memory::default());
    let app = app.with_vault(vault.clone());
    app.check_and_keep(SECRET).unwrap();
    assert!(app.settings().switch(Switch::SteamKeyKept));
    assert!(app.key_state().stored && app.key_mode());
    vault.inner.delete().unwrap();
    assert!(!app.key_state().stored);
    assert!(!app.settings().switch(Switch::SteamKeyKept) && !app.key_mode());
    let before = vault.asked();
    assert!(!app.key_state().stored && !app.key_mode());
    assert_eq!(vault.asked(), before);

    app.check_and_keep(SECRET).unwrap();
    vault.inner.delete().unwrap();
    assert!(!app.key_state().stored);
    assert!(!app.settings().switch(Switch::SteamKeyKept));
    app.check_and_keep(SECRET).unwrap();
    app.forget_key().unwrap();
    assert!(!app.settings().switch(Switch::SteamKeyKept));
}
