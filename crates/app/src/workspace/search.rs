use super::App;
use rimstack_core::mods::bisect::{self, Bisect};
use rimstack_core::rimworld::saves;
use rimstack_core::say;
use std::path::PathBuf;

const SAVES_BEFORE_SEARCH: usize = 3;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Hunt {
    pub active: bool,
    pub asking: bool,
    pub found: String,
    pub found_id: String,
    pub stuck: Vec<String>,
    pub rounds: u32,
    pub suspects: usize,
    pub list: Vec<String>,
    pub everything: usize,
}

impl App {
    fn search_file(&self) -> PathBuf {
        self.data_dir().join("bisect.json")
    }

    fn shown(&self, search: Option<&Bisect>) -> Hunt {
        let Some(search) = search else {
            return Hunt::default();
        };
        let loaded = self.library();
        let library = loaded.as_ref().as_ref().ok().map(|(library, _)| library);
        let name = |id: &str| {
            let key = rimstack_core::rimworld::id::split_steam(id).0;
            library
                .and_then(|l| l.name_of(&key))
                .unwrap_or_else(|| id.to_string())
        };
        let stuck: Vec<String> = match search.is_stuck() {
            true => search
                .suspects
                .iter()
                .map(|at| name(&search.original[*at]))
                .collect(),
            false => Vec::new(),
        };
        let found = search.found();
        Hunt {
            active: true,
            asking: found.is_none() && stuck.is_empty(),
            found: found.map(name).unwrap_or_default(),
            found_id: found.unwrap_or_default().to_string(),
            stuck,
            rounds: search.rounds_left(),
            suspects: search.suspects.len(),
            list: search.list(),
            everything: search.original.len(),
        }
    }

    fn keep_search(&self, search: &Bisect) {
        if let Err(problem) = search.write(&self.search_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn hunt(&self) -> Hunt {
        self.shown(Bisect::read(&self.search_file()).as_ref())
    }

    pub fn start_hunt(&self, records: bool) -> Hunt {
        let list = self.session().current().to_vec();
        if records {
            let before = self.snapshot("@hunt", &list);
            self.note_change(say!("Began finding the broken mod"), before.as_deref());
        }
        let newest = self
            .saves_dir()
            .and_then(|dir| saves::list(&dir).into_iter().next());
        if let Some(save) = newest {
            let kept = self.data_dir().join("save-before-search");
            let copied = saves::back_up(&save.path, &kept, super::now_secs(), SAVES_BEFORE_SEARCH);
            if let Err(problem) = copied {
                rimstack_core::problem!("{problem}");
            }
        }
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Hunt::default();
        };
        let search = bisect::start(library, &library.arrange(&list).active);
        self.keep_search(&search);
        self.shown(Some(&search))
    }

    pub fn answer_hunt(&self, broken: bool) -> Hunt {
        let Some(mut search) = Bisect::read(&self.search_file()) else {
            return Hunt::default();
        };
        search.answer(broken);
        self.keep_search(&search);
        self.shown(Some(&search))
    }

    /// Less the mod found if `without_found`.
    pub fn end_hunt(&self, without_found: bool) -> Vec<String> {
        let search = Bisect::read(&self.search_file());
        let _ = std::fs::remove_file(self.search_file());
        let Some(search) = search else {
            return Vec::new();
        };
        let found = search.found().filter(|_| without_found).map(String::from);
        search
            .original
            .into_iter()
            .filter(|id| Some(id) != found.as_ref())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::kept::journal;

    #[test]
    fn a_search_narrows_to_one_mod_keeps_its_state_and_gives_the_list_back() {
        let (tmp, app) = installed();
        for at in 0..6 {
            let about = tmp.path().join(format!("game/Mods/m{at}/About"));
            std::fs::create_dir_all(&about).unwrap();
            std::fs::write(
                about.join("About.xml"),
                format!(
                    "<ModMetaData><packageId>a.m{at}</packageId><name>M{at}</name></ModMetaData>"
                ),
            )
            .unwrap();
        }
        app.rescan();
        let mut list = vec!["ludeon.rimworld".to_string()];
        list.extend((0..6).map(|at| format!("a.m{at}")));
        app.session().record(list.clone());

        let first = app.start_hunt(true);
        assert!(first.active && first.asking);
        assert!(first.list.len() < first.everything);
        assert_eq!(first.everything, 7);
        app.settle_writes();
        assert_eq!(journal::read(&app.journal_file()).len(), 1);
        let mut now = first;
        let mut rounds = 0;
        while now.asking && rounds < 20 {
            now = app.answer_hunt(rounds % 2 == 0);
            rounds += 1;
        }
        assert!(!now.found.is_empty() || !now.stuck.is_empty());
        assert_eq!(app.hunt(), now);
        let back = app.end_hunt(true);
        assert_eq!(
            back.len(),
            list.len() - usize::from(!now.found_id.is_empty())
        );
        assert!(!app.hunt().active);
        assert!(app.end_hunt(false).is_empty());
    }
}
