#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        /// `{ name, mods, modified }`.
        #[qproperty(QVariant, saved)]
        /// `{ name, title }`.
        #[qproperty(QVariant, snapshots)]
        /// Newest first; `{ time, said, restore, bringBack }`.
        #[qproperty(QVariant, history)]
        type ListsData = super::ListsDataRust;
    }

    impl cxx_qt::Threading for ListsData {}

    extern "RustQt" {
        #[qinvokable]
        fn keep(self: &ListsData, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "keepIds"]
        fn keep_ids(self: &ListsData, name: &QString, ids: &QVariant) -> QString;

        #[qinvokable]
        fn remove(self: &ListsData, name: &QString);

        /// `None` for no such list.
        #[qinvokable]
        fn named(self: &ListsData, name: &QString) -> QVariant;

        #[qinvokable]
        #[cxx_name = "snapshotNamed"]
        fn snapshot_named(self: &ListsData, name: &QString) -> QVariant;

        #[qinvokable]
        #[cxx_name = "idsInText"]
        fn ids_in_text(self: &ListsData, text: &QString) -> QVariant;

        /// `{ ids, problem }`.
        #[qinvokable]
        #[cxx_name = "idsInFile"]
        fn ids_in_file(self: &ListsData, file: &QUrl) -> QVariant;

        #[qinvokable]
        fn original(self: &ListsData) -> QVariant;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_lists_data() -> UniquePtr<ListsData>;
    }
}

use crate::App;
use crate::changes::Changes;
use crate::qml::values::{from_qml, to_qml};
use crate::workspace::lists::ListFile;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl, QVariant};
use rimstack_core::base::error::Error;
use rimstack_core::say;
use std::sync::Arc;

#[derive(Default)]
pub struct ListsDataRust {
    saved: QVariant,
    snapshots: QVariant,
    history: QVariant,
    app: crate::hub::Given,
}

fn said(done: Result<(), Error>) -> QString {
    crate::words::why_not(&done)
}

impl qobject::ListsData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>, demo: bool) {
        self.as_mut().rust_mut().get_mut().app = crate::hub::Given::of(app.clone());
        if demo {
            return;
        }
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        self.heard(Changes::LISTS);
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        if !changes.contains(Changes::LISTS | Changes::LANGUAGE) {
            return;
        }
        let app = self.app();
        self.as_mut().set_saved(to_qml(&app.saved_lists()));
        self.as_mut().set_snapshots(to_qml(&app.recent_snapshots()));
        self.set_history(to_qml(&app.history()));
    }

    fn keep(&self, name: &QString) -> QString {
        said(self.app().keep_list(&name.to_string(), None))
    }

    fn keep_ids(&self, name: &QString, ids: &QVariant) -> QString {
        match from_qml::<Vec<String>>(ids) {
            Some(ids) => said(self.app().keep_list(&name.to_string(), Some(ids))),
            None => crate::words::said(&say!("That is not a list of mods.")),
        }
    }

    fn remove(&self, name: &QString) {
        self.app().delete_list(&name.to_string());
    }

    fn named(&self, name: &QString) -> QVariant {
        to_qml(&self.app().list_named(&name.to_string()))
    }

    fn snapshot_named(&self, name: &QString) -> QVariant {
        to_qml(&self.app().snapshot_named(&name.to_string()))
    }

    fn ids_in_text(&self, text: &QString) -> QVariant {
        to_qml(&rimstack_core::mods::share::ids_in(&text.to_string()))
    }

    fn ids_in_file(&self, file: &QUrl) -> QVariant {
        let path = file.to_local_file().unwrap_or_default().to_string();
        to_qml(&ListFile::read(std::path::Path::new(&path)))
    }

    fn original(&self) -> QVariant {
        to_qml(&self.app().original_list())
    }
}
