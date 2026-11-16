use super::lists::{FileLists, FolderSettings};
use crate::base::locks::Held;
use crate::base::pattern::Pattern;
use rayon::prelude::*;
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

const MOST_BYTES: u64 = 16 << 20;
/// A file with a NUL in its first bytes is binary.
const SNIFF: usize = 8192;
const CONTEXT: usize = 3;
const LINE_CUT: usize = 300;
const BATCH_HITS: usize = 500;

pub enum Target<K> {
    Mod {
        key: K,
        folder: PathBuf,
    },
    Settings {
        folder: PathBuf,
        key_of: Box<dyn Fn(&str) -> K + Send + Sync>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// From 1.
    pub line: usize,
    pub before: Vec<String>,
    pub text: String,
    /// Bytes of `text`, clipped to it when the match runs past the line.
    pub marked: Range<usize>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileHits<K> {
    pub key: K,
    pub file: PathBuf,
    pub hits: Vec<Hit>,
    pub unkept: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Batch<K> {
    pub found: Vec<FileHits<K>>,
    pub progress: Progress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub targets: usize,
    pub files: usize,
    pub matches: usize,
    pub too_large: usize,
    pub binary: usize,
    pub cancelled: bool,
}

#[derive(Default)]
struct Counts {
    done: AtomicUsize,
    files: AtomicUsize,
    matches: AtomicUsize,
    too_large: AtomicUsize,
    binary: AtomicUsize,
}

pub fn search<K: Clone + Send + Sync>(
    targets: &[Target<K>],
    pattern: &Pattern,
    settings: &FolderSettings,
    lists: &FileLists,
    cancel: &AtomicBool,
    most_hits: usize,
    on_batch: impl Fn(Batch<K>) + Sync,
) -> Summary {
    let counts = Counts::default();
    let room = AtomicUsize::new(most_hits);
    let progress = || Progress {
        done: counts.done.load(Relaxed),
        total: targets.len(),
    };
    let emit = |found: Vec<FileHits<K>>| {
        if !found.is_empty() {
            on_batch(Batch {
                found,
                progress: progress(),
            });
        }
    };
    targets.par_iter().for_each(|target| {
        if cancel.load(Relaxed) {
            return;
        }
        let jobs = jobs_of(target, settings, lists);
        let held = Mutex::new((Vec::new(), 0usize));
        jobs.par_iter().for_each(|(key, file)| {
            if cancel.load(Relaxed) {
                return;
            }
            let Some((hits, unkept)) = search_file(
                file,
                pattern,
                settings.types.reads_any_text(),
                &counts,
                &room,
            ) else {
                return;
            };
            let mut held = held.held();
            held.1 += hits.len();
            held.0.push(FileHits {
                key: key.clone(),
                file: file.clone(),
                hits,
                unkept,
            });
            if held.1 >= BATCH_HITS {
                held.1 = 0;
                emit(std::mem::take(&mut held.0));
            }
        });
        emit(held.into_inner().unwrap().0);
        if !cancel.load(Relaxed) {
            counts.done.fetch_add(1, Relaxed);
        }
    });
    Summary {
        targets: targets.len(),
        files: counts.files.load(Relaxed),
        matches: counts.matches.load(Relaxed),
        too_large: counts.too_large.load(Relaxed),
        binary: counts.binary.load(Relaxed),
        cancelled: cancel.load(Relaxed),
    }
}

fn jobs_of<K: Clone>(
    target: &Target<K>,
    settings: &FolderSettings,
    lists: &FileLists,
) -> Vec<(K, PathBuf)> {
    match target {
        Target::Mod { key, folder } => lists
            .get(folder, settings)
            .iter()
            .map(|file| (key.clone(), file.clone()))
            .collect(),
        Target::Settings { folder, key_of } => {
            let entries = std::fs::read_dir(folder).into_iter().flatten().flatten();
            let mut found: Vec<(K, PathBuf)> = entries
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let lower = name.to_lowercase();
                    let is_settings = lower.starts_with("mod_") && lower.ends_with(".xml");
                    is_settings.then(|| (key_of(&name), entry.path()))
                })
                .collect();
            found.sort_by(|a, b| a.1.cmp(&b.1));
            found
        }
    }
}

fn search_file(
    file: &Path,
    pattern: &Pattern,
    all_text: bool,
    counts: &Counts,
    room: &AtomicUsize,
) -> Option<(Vec<Hit>, usize)> {
    let mut open = std::fs::File::open(file).ok()?;
    if open.metadata().is_ok_and(|m| m.len() > MOST_BYTES) {
        counts.too_large.fetch_add(1, Relaxed);
        return None;
    }
    let bytes = read_text(&mut open, all_text, counts)?;
    counts.files.fetch_add(1, Relaxed);
    let text = String::from_utf8_lossy(&bytes);
    let (hits, matches) = hits_within(&text, pattern, room);
    counts.matches.fetch_add(matches, Relaxed);
    (matches > 0).then(|| {
        let unkept = matches - hits.len();
        (hits, unkept)
    })
}

fn read_text(open: &mut std::fs::File, sniff: bool, counts: &Counts) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    if sniff {
        (&mut *open)
            .take(SNIFF as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.contains(&0) {
            counts.binary.fetch_add(1, Relaxed);
            return None;
        }
    }
    open.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn take_room(room: &AtomicUsize) -> bool {
    room.try_update(Relaxed, Relaxed, |left| left.checked_sub(1))
        .is_ok()
}

fn hits_within(text: &str, pattern: &Pattern, room: &AtomicUsize) -> (Vec<Hit>, usize) {
    let mut found = pattern.find_in(text).peekable();
    if found.peek().is_none() {
        return (Vec::new(), 0);
    }
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let lines = starts.len() - usize::from(text.ends_with('\n'));
    let line_of = |at: usize| starts.partition_point(|&start| start <= at) - 1;
    let line_text = |index: usize| {
        let end = starts.get(index + 1).map_or(text.len(), |&next| next - 1);
        text[starts[index]..end].trim_end_matches('\r')
    };
    let mut matches = 0;
    let hits = found
        .filter_map(|range| {
            matches += 1;
            if !take_room(room) {
                return None;
            }
            let index = line_of(range.start);
            let line = line_text(index);
            let from = range.start - starts[index];
            let to = (range.end - starts[index])
                .min(line.len())
                .min(from + LINE_CUT);
            let (shown, marked) = cut_around(line, from..to);
            let context = |indexes: Range<usize>| {
                indexes
                    .map(|i| cut_around(line_text(i), 0..0).0.to_string())
                    .collect()
            };
            Some(Hit {
                line: index + 1,
                before: context(index.saturating_sub(CONTEXT)..index),
                text: shown.to_string(),
                marked,
                after: context(index + 1..(index + 1 + CONTEXT).min(lines)),
            })
        })
        .collect();
    (hits, matches)
}

fn cut_around(line: &str, marked: Range<usize>) -> (&str, Range<usize>) {
    if line.len() <= LINE_CUT {
        return (line, marked);
    }
    let room = LINE_CUT.saturating_sub(marked.len()) / 3;
    let mut start = marked.start.saturating_sub(room);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + LINE_CUT.max(marked.end - start)).min(line.len());
    while !line.is_char_boundary(end) {
        end += 1;
    }
    (&line[start..end], marked.start - start..marked.end - start)
}
