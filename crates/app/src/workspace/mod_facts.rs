use crate::App;
use rimstack_core::base::locks::{Held, Shared};
use rimstack_core::base::stamp::FolderStamp;
use rimstack_core::kept::mod_facts::ModFacts;
use rimstack_core::mods::facts::Facts;
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::loadfolders;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BATCH: usize = 48;
const THREADS: usize = 2;
const BETWEEN_BATCHES: Duration = Duration::from_millis(20);

pub type FactsBy = HashMap<ModId, Arc<Facts>>;

#[derive(Default)]
pub struct Read {
    library: Option<Arc<crate::workspace::Loaded>>,
    by: Arc<FactsBy>,
}

pub struct Job {
    pub root: PathBuf,
    pub loaded: Vec<PathBuf>,
    pub whole: bool,
}

/// The stamp leaves out the folders that hold the most files and whose contents no fact needs
/// beyond that they exist, which the folder above them already stamps.
fn left_out(name: &OsStr) -> bool {
    ["textures", "sounds", ".git"]
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
}

/// Returns the facts in the order of `jobs` and how many mods' files were read.
pub fn gather(
    record: &mut ModFacts<Facts>,
    jobs: &[Job],
    game: &HashSet<String>,
) -> (Vec<Arc<Facts>>, usize) {
    use rayon::prelude::*;
    let stamps: Vec<FolderStamp> = jobs
        .par_iter()
        .map(|job| FolderStamp::of(&job.root, left_out))
        .collect();
    let mut found: Vec<Option<Arc<Facts>>> = jobs
        .iter()
        .zip(&stamps)
        .map(|(job, stamp)| {
            record
                .get(&job.root, *stamp)
                .filter(|f| f.folders == job.loaded)
        })
        .collect();
    let missing: Vec<usize> = (0..jobs.len()).filter(|&at| found[at].is_none()).collect();
    let read: Vec<Arc<Facts>> = missing
        .par_iter()
        .map(|&at| {
            let loaded = jobs[at].loaded.clone();
            Arc::new(match jobs[at].whole {
                true => Facts::read(loaded, game),
                false => Facts::read_names(loaded),
            })
        })
        .collect();
    for (&at, facts) in missing.iter().zip(read) {
        record.put(&jobs[at].root, stamps[at], facts.clone());
        found[at] = Some(facts);
    }
    let facts = found.into_iter().flatten().collect();
    (facts, missing.len())
}

impl App {
    pub fn facts_file(&self) -> PathBuf {
        self.data_dir().join("mod-facts.json")
    }

    pub fn mod_facts(&self, library: &Arc<crate::workspace::Loaded>) -> Arc<FactsBy> {
        let read = self.facts().reading();
        match read
            .library
            .as_ref()
            .is_some_and(|l| Arc::ptr_eq(l, library))
        {
            true => read.by.clone(),
            false => Arc::default(),
        }
    }

    pub fn mods_read(&self) -> usize {
        self.facts_counted().load(Ordering::Relaxed)
    }

    pub(super) fn count_mods_read(&self, read: usize) {
        self.facts_counted().fetch_add(read, Ordering::Relaxed);
    }

    pub(super) fn begin_facts_run(&self) -> u64 {
        self.facts_runs().fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The files read under way, if any, stops at its next batch: a newer reading was asked for.
    pub fn outdate_files_read(&self) {
        self.begin_facts_run();
    }

    fn facts_run_current(&self, run: u64) -> bool {
        self.facts_runs().load(Ordering::SeqCst) == run
    }

    pub(super) fn keep_facts(
        &self,
        run: u64,
        library: &Arc<crate::workspace::Loaded>,
        found: impl IntoIterator<Item = (ModId, Arc<Facts>)>,
    ) -> bool {
        let mut slot = self.facts().writing();
        if !self.facts_run_current(run) || !Arc::ptr_eq(library, &self.library()) {
            return false;
        }
        let same = slot
            .library
            .as_ref()
            .is_some_and(|l| Arc::ptr_eq(l, library));
        let mut by = if same {
            (*slot.by).clone()
        } else {
            FactsBy::new()
        };
        by.extend(found);
        *slot = Read {
            library: Some(library.clone()),
            by: Arc::new(by),
        };
        true
    }

    pub(super) fn keep_record(
        &self,
        run: u64,
        record: &mut ModFacts<Facts>,
        installed: &HashSet<String>,
    ) {
        static WRITING: Mutex<()> = Mutex::new(());
        let _writing = WRITING.held();
        if !self.facts_run_current(run) {
            return;
        }
        record.keep_only(installed);
        if let Err(problem) = record.write(&self.facts_file()) {
            rimstack_core::problem!("what the mods' files hold was not kept: {problem}");
        }
    }

    /// `members` is ascending.
    pub(super) fn read_inactive_facts(
        &self,
        run: u64,
        loaded: &Arc<crate::workspace::Loaded>,
        record: &mut ModFacts<Facts>,
        members: &[ModId],
    ) {
        let Ok((library, _)) = &**loaded else {
            return;
        };
        let installed: HashSet<String> = library
            .ids()
            .map(|(_, m)| m.path.to_string_lossy().into_owned())
            .collect();
        let known = self.mod_facts(loaded);
        let every = |m: &rimstack_core::mods::library::Installed| {
            loadfolders::every_folder(&m.path, &library.game_version)
        };
        let mut waiting: Vec<(ModId, Job)> = library
            .ids()
            .filter(|(id, _)| members.binary_search(id).is_err())
            .map(|(id, m)| {
                (
                    id,
                    Job {
                        root: m.path.clone(),
                        loaded: every(m),
                        whole: false,
                    },
                )
            })
            .filter(|(id, job)| {
                known
                    .get(id)
                    .is_none_or(|facts| facts.folders != job.loaded)
            })
            .collect();
        let Ok(pool) = rayon::ThreadPoolBuilder::new()
            .num_threads(THREADS)
            .thread_name(|n| format!("mod facts {n}"))
            .build()
        else {
            return;
        };
        let mut unwritten = false;
        while !waiting.is_empty() {
            let rest = waiting.split_off(BATCH.min(waiting.len()));
            let (ids, jobs): (Vec<ModId>, Vec<Job>) =
                std::mem::replace(&mut waiting, rest).into_iter().unzip();
            let (facts, read) = pool.install(|| gather(record, &jobs, &library.game_assemblies));
            self.count_mods_read(read);
            unwritten |= read > 0;
            if !self.keep_facts(run, loaded, ids.into_iter().zip(facts)) {
                break;
            }
            self.publish(crate::changes::Changes::FACTS);
            std::thread::sleep(BETWEEN_BATCHES);
        }
        if unwritten {
            self.keep_record(run, record, &installed);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::load_order::{LoadOrder, Side};
    use crate::workspace::tests::installed;
    use std::time::{Duration, SystemTime};

    fn start(app: &crate::App) -> LoadOrder {
        app.rescan();
        let mut order = LoadOrder::read(app, true, None);
        let members = order.files_to_read(app).unwrap();
        app.read_active_files(members);
        order.facts_read(app);
        order
    }

    fn roads_defs(tmp: &tempfile::TempDir) -> std::path::PathBuf {
        let defs = tmp.path().join("game/Mods/roads/Defs");
        std::fs::create_dir_all(&defs).unwrap();
        std::fs::write(
            defs.join("a.xml"),
            "<Defs><ThingDef><defName>RoadDef</defName></ThingDef></Defs>",
        )
        .unwrap();
        defs
    }

    fn later(dir: &std::path::Path) {
        let file = std::fs::File::open(dir).unwrap();
        let at = SystemTime::now() + Duration::from_secs(900);
        file.set_modified(at).unwrap();
    }

    #[test]
    fn an_inactive_mod_is_read_and_the_filter_says_how_many_were_not_before() {
        let (tmp, app) = installed();
        roads_defs(&tmp);
        let mut order = LoadOrder::read(&app, true, None);
        order.set_filter("defines:roaddef");
        order.facts_read(&app);
        assert_eq!(
            order.filter_summary().to_string(),
            "2 mods not read yet: 0 active, 0 inactive"
        );
        let order = {
            let mut order = start(&app);
            order.set_filter("defines:roaddef");
            order
        };
        assert_eq!(order.filter_summary().to_string(), "0 active, 1 inactive");
        assert_eq!(order.list(Side::Inactive).matching(), 1);
        assert_eq!(order.list(Side::Active).matching(), 0);
        assert_eq!(app.mods_read(), 2);
    }

    #[test]
    fn a_start_with_nothing_changed_reads_no_mod_and_a_changed_folder_reads_that_mod_again() {
        let (tmp, app) = installed();
        let defs = roads_defs(&tmp);
        start(&app);
        assert_eq!(app.mods_read(), 2);
        start(&app);
        assert_eq!(app.mods_read(), 2);
        later(&defs);
        let mut order = start(&app);
        assert_eq!(app.mods_read(), 3);
        order.set_filter("has:defs");
        assert_eq!(order.list(Side::Inactive).matching(), 1);
    }

    #[test]
    fn a_record_of_an_older_format_or_another_game_is_read_again() {
        let (tmp, app) = installed();
        roads_defs(&tmp);
        start(&app);
        let file = app.facts_file();
        let text = std::fs::read_to_string(&file).unwrap();
        let format = format!("\"format\":{}", rimstack_core::kept::mod_facts::FORMAT);
        std::fs::write(&file, text.replacen(&format, "\"format\":0", 1)).unwrap();
        start(&app);
        assert_eq!(app.mods_read(), 4);
    }

    #[test]
    fn an_active_mod_read_with_fewer_folders_is_read_again_in_full_when_it_is_not_active() {
        let (tmp, app) = installed();
        let roads = tmp.path().join("game/Mods/roads");
        std::fs::write(
            roads.join("LoadFolders.xml"),
            r#"<loadFolders><v1.6><li>Plain</li><li IfModActive="other.mod">Extra</li></v1.6></loadFolders>"#,
        )
        .unwrap();
        for folder in ["Plain", "Extra"] {
            let defs = roads.join(folder).join("Defs");
            std::fs::create_dir_all(&defs).unwrap();
            let def = format!("<Defs><ThingDef><defName>{folder}Def</defName></ThingDef></Defs>");
            std::fs::write(defs.join("a.xml"), def).unwrap();
        }
        let mut order = start(&app);
        order.set_filter("defines:extradef");
        assert_eq!(order.list(Side::Inactive).matching(), 1);
    }
}
