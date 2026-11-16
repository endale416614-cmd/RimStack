use rayon::prelude::*;
use rimstack_core::base::stamp::FolderStamp;
use rimstack_core::kept::mod_facts::ModFacts;
use rimstack_core::mods::facts::Facts;
use rimstack_core::rimworld::loadfolders;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn left_out(name: &OsStr) -> bool {
    ["textures", "sounds", ".git"]
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
}

/// `RIMSTACK_WORKSHOP=…/steamapps/workshop/content/294100`, read only; the record goes to a scratch folder.
#[test]
#[ignore = "needs a Workshop folder in RIMSTACK_WORKSHOP"]
fn reading_a_whole_workshop_folder_and_again_with_the_record() {
    for whole in [false, true] {
        println!(
            "--- {}",
            if whole {
                "whole facts (every mod active)"
            } else {
                "names only (every mod inactive)"
            }
        );
        measured(whole);
    }
}

fn measured(whole: bool) {
    let root = PathBuf::from(std::env::var("RIMSTACK_WORKSHOP").unwrap());
    let mods: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    let game = HashSet::new();
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("mod-facts.json");

    let started = Instant::now();
    let mut record = ModFacts::<Facts>::empty("1.6");
    let read: Vec<(PathBuf, FolderStamp, Arc<Facts>)> = mods
        .par_iter()
        .map(|dir| {
            let stamp = FolderStamp::of(dir, left_out);
            let folders = loadfolders::every_folder(dir, "1.6");
            (
                dir.clone(),
                stamp,
                Arc::new(match whole {
                    true => Facts::read(folders, &game),
                    false => Facts::read_names(folders),
                }),
            )
        })
        .collect();
    for (dir, stamp, facts) in &read {
        record.put(dir, *stamp, facts.clone());
    }
    println!("first read of {} mods: {:?}", mods.len(), started.elapsed());
    let written = Instant::now();
    record.write(&file).unwrap();
    println!(
        "record written in {:?}, {} bytes",
        written.elapsed(),
        std::fs::metadata(&file).unwrap().len()
    );

    let second = Instant::now();
    let record = ModFacts::<Facts>::read(&file, "1.6");
    let reread = std::sync::atomic::AtomicUsize::new(0);
    mods.par_iter().for_each(|dir| {
        let stamp = FolderStamp::of(dir, left_out);
        let folders = loadfolders::every_folder(dir, "1.6");
        let kept = record.get(dir, stamp).filter(|f| f.folders == folders);
        if kept.is_none() {
            reread.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });
    println!(
        "second start: {:?}, {} of {} mods would be read again",
        second.elapsed(),
        reread.load(std::sync::atomic::Ordering::Relaxed),
        mods.len()
    );
}
