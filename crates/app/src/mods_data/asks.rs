use super::{id, ids, qobject};
use crate::qml::values::{from_qml, to_qml};
use crate::workspace::load_order::{LoadOrder, Side};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use rimstack_core::mods::columns;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Row;

/// -1 for none.
fn held(found: Option<usize>) -> i32 {
    found.map_or(-1, |at| at as i32)
}

fn held_id(found: Option<ModId>) -> i32 {
    found.map_or(-1, ModId::to_qml)
}

impl qobject::ModsData {
    pub(super) fn needed_ids(&self, keys: &[String]) -> Vec<ModId> {
        let app = self.app();
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let key_of = |id: ModId| library.get(id)?.key();
        self.order.list(Side::Inactive).with_keys(keys, &key_of)
    }

    pub(super) fn version_warned(&self, given: &QVariant) -> QVariant {
        let list = self.order.list(Side::Active);
        let warned = |id: &ModId| {
            let own = list
                .position_of(*id)
                .and_then(|at| self.order.troubles().get(at));
            own.is_some_and(|own| rimstack_core::mods::describe::warned_for_version(own))
        };
        let Some(given) = ids(given) else {
            return to_qml(&Vec::<i32>::new());
        };
        let found: Vec<i32> = given
            .into_iter()
            .filter(warned)
            .map(ModId::to_qml)
            .collect();
        to_qml(&found)
    }

    pub(super) fn upkeep_of(&self, given: &QVariant) -> QVariant {
        let given = ids(given).unwrap_or_default();
        to_qml(&self.order.upkeep(&self.app(), &given))
    }

    pub(super) fn package_mods(&self, given: &QVariant) -> QVariant {
        let Some(keys) = from_qml::<Vec<String>>(given) else {
            return to_qml(&Vec::<i32>::new());
        };
        let app = self.app();
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return to_qml(&Vec::<i32>::new());
        };
        let key_of = |id: ModId| library.get(id)?.key();
        let found = self.order.list(Side::Active).with_keys(&keys, &key_of);
        to_qml(&found.into_iter().map(ModId::to_qml).collect::<Vec<_>>())
    }

    pub(super) fn package_mod(&self, active: bool, key: &QString) -> i32 {
        let key = key.to_string();
        let app = self.app();
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return -1;
        };
        let has = |id: &ModId| {
            library
                .get(*id)
                .and_then(|m| m.key())
                .is_some_and(|k| k == key)
        };
        let mods = self.order.list(Side::of(active)).mods().iter();
        held_id(mods.filter_map(|m| m.installed).find(has))
    }

    pub(super) fn find_mods(&self, text: &QString, most: i32) -> QVariant {
        let most = usize::try_from(most).unwrap_or(0);
        to_qml(&self.order.found(&self.app(), &text.to_string(), most))
    }

    pub(super) fn listing(&self) -> QString {
        let app = self.app();
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return QString::default();
        };
        let mods = self.order.list(Side::Active).mods().iter();
        let entries: Vec<rimstack_core::mods::share::Entry> = mods
            .filter_map(|m| library.get(m.installed?))
            .map(rimstack_core::mods::share::Entry::of)
            .collect();
        QString::from(&rimstack_core::mods::share::listing(
            &library.game_version,
            &entries,
        ))
    }

    pub(super) fn sort_moves(&self) -> QVariant {
        to_qml(&self.order.sort_moves())
    }

    pub(super) fn first_moved(&self) -> i32 {
        held(self.order.first_moved())
    }

    fn troubled_mods(order: &LoadOrder) -> Vec<bool> {
        let count = order.list(Side::Active).mods().len();
        (0..count)
            .map(|at| order.troubles().get(at).is_some_and(|t| !t.is_empty()))
            .collect()
    }

    pub(super) fn next_problem(&self, row: i32, forward: bool) -> i32 {
        let rows = self.order.list(Side::Active).rows();
        let troubled = Self::troubled_mods(&self.order);
        let has = |r: &usize| matches!(rows[*r], Row::Mod { index } if troubled[index]);
        let count = rows.len() as i32;
        if count == 0 {
            return -1;
        }
        let step = if forward { 1 } else { -1 };
        let from = if row < 0 && !forward { 0 } else { row };
        let mut others = (1..=count).map(|n| (from + step * n).rem_euclid(count) as usize);
        held(others.find(has))
    }

    pub(super) fn problem_place(&self, row: i32) -> i32 {
        let rows = self.order.list(Side::Active).rows();
        match usize::try_from(row).ok().and_then(|r| rows.get(r)) {
            Some(Row::Mod { index }) => {
                let troubled = Self::troubled_mods(&self.order);
                rimstack_core::mods::checks::place_among(&troubled, *index).0 as i32
            }
            _ => 0,
        }
    }

    pub(super) fn name_of(&self, mod_id: i32) -> QString {
        let app = self.app();
        let loaded = app.library();
        let found = loaded.as_ref().as_ref().ok().zip(id(mod_id));
        let name = found.and_then(|((library, _), id)| library.get(id).map(|m| m.display_name()));
        QString::from(name.unwrap_or_default())
    }

    pub(super) fn step_from(&self, active: bool, mod_id: i32, by: i32) -> i32 {
        let list = self.order.list(Side::of(active));
        held(list.step_from(id(mod_id), by.signum() as isize))
    }

    pub(super) fn beside_mod(&self, active: bool, mod_id: i32) -> i32 {
        let list = self.order.list(Side::of(active));
        held_id(id(mod_id).and_then(|id| list.beside_mod(id)))
    }

    pub(super) fn others_through(&self, active: bool, from: i32, to: i32) -> QVariant {
        let list = self.order.list(Side::of(active));
        let others = id(from)
            .zip(id(to))
            .and_then(|(a, b)| list.others_through(a, b));
        to_qml(&others.map(|ids| ids.into_iter().map(ModId::to_qml).collect::<Vec<_>>()))
    }

    pub(super) fn in_load_order(&self, a: i32, b: i32) -> QVariant {
        let list = self.order.list(Side::Active);
        let ordered = id(a).zip(id(b)).and_then(|(a, b)| list.in_load_order(a, b));
        to_qml(&ordered.map(|(earlier, later)| [earlier.to_qml(), later.to_qml()]))
    }

    pub(super) fn nudge_to(&self, active: bool, mod_id: i32, by: i32) -> i32 {
        let list = self.order.list(Side::of(active));
        held(
            id(mod_id)
                .and_then(|id| list.nudged(id, by as isize))
                .map(|(_, to)| to),
        )
    }

    pub(super) fn subs_beside(&self, mod_id: i32) -> QVariant {
        let shown = [Side::Active, Side::Inactive].map(|side| self.order.list(side));
        let listed = shown
            .iter()
            .flat_map(|list| list.mods().iter().map(move |m| (list, m)))
            .find(|(_, m)| m.installed.is_some() && m.installed == id(mod_id));
        let subs = listed
            .map(|(list, m)| list.subs(m.category))
            .unwrap_or_default();
        to_qml(&subs)
    }

    pub(super) fn details_of(mut self: Pin<&mut Self>, mod_id: i32) -> QVariant {
        let app = self.app();
        match id(mod_id) {
            Some(id) => to_qml(&self.as_mut().rust_mut().get_mut().order.details(&app, id)),
            None => QVariant::default(),
        }
    }

    pub(super) fn not_installed(&self, active: bool, index: i32) -> QVariant {
        usize::try_from(index)
            .ok()
            .and_then(|at| self.order.not_installed(&self.app(), Side::of(active), at))
            .map_or_else(QVariant::default, |said| to_qml(&said))
    }

    pub(super) fn files_of(&self, mod_id: i32) -> QVariant {
        match id(mod_id) {
            Some(id) => to_qml(&LoadOrder::files(&self.app(), id)),
            None => QVariant::default(),
        }
    }

    pub(super) fn columns_toggled(&self, kept: &QString, column: &QString) -> QString {
        QString::from(&columns::toggled(&kept.to_string(), &column.to_string()))
    }

    pub(super) fn columns_moved(&self, kept: &QString, column: &QString, to: i32) -> QString {
        QString::from(&columns::moved(&kept.to_string(), &column.to_string(), to))
    }

    pub(super) fn column_widths(&self, kept: &QString) -> QVariant {
        to_qml(&columns::widths(&kept.to_string()))
    }

    pub(super) fn column_sized(&self, kept: &QString, column: &QString, width: f64) -> QString {
        QString::from(&columns::sized(
            &kept.to_string(),
            &column.to_string(),
            width,
        ))
    }

    pub(super) fn column_pressed(
        &self,
        by: &QString,
        descending: bool,
        asked: &QString,
    ) -> QVariant {
        to_qml(&columns::pressed(
            &by.to_string(),
            descending,
            &asked.to_string(),
        ))
    }

    pub(super) fn mark_ready(&self) {
        if let Some(started) = crate::STARTED.get() {
            rimstack_core::note!("startup_ms={}", started.elapsed().as_millis());
        }
        let check = crate::hub::value("--check");
        let scripted = matches!(check.as_deref(), Some("" | "restore"));
        if (check.is_some() && !scripted) || crate::hub::given("--selftest") {
            crate::jobs::watch_stalls(self.qt_thread());
        }
    }
}
