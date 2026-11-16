use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Held;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::launches::{self, LastStart, LogRead, Shown, SinceClean};
use rimstack_core::mods::blame::{Blamed, Pointed};
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::logs::Level;
use rimstack_core::rimworld::{id, logs, paths::Source};
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const SHOWN_LINES: usize = 30_000;

/// Starts later than this after a recorded launch are not that launch.
const LAUNCH_SLACK_SECS: u64 = 120;

const LONGEST_KIND: usize = 90;

#[derive(Serialize)]
pub struct LastStartSaid {
    #[serde(flatten)]
    pub start: LastStart,
    pub line: Sentence,
}

pub fn moment(seconds: u64) -> String {
    use chrono::TimeZone;
    let at = i64::try_from(seconds)
        .ok()
        .and_then(|s| chrono::Local.timestamp_opt(s, 0).single());
    at.map_or(String::new(), |at| at.format("%Y-%m-%d %H:%M").to_string())
}

pub fn day(seconds: u64) -> String {
    local(seconds, "%Y-%m-%d")
}

pub fn clock(seconds: u64) -> String {
    local(seconds, "%H:%M")
}

fn local(seconds: u64, shape: &str) -> String {
    use chrono::TimeZone;
    let at = i64::try_from(seconds)
        .ok()
        .and_then(|s| chrono::Local.timestamp_opt(s, 0).single());
    at.map_or(String::new(), |at| at.format(shape).to_string())
}

pub struct LogsReport {
    pub previous: bool,
    pub has_other: bool,
    pub path: String,
    pub written: u64,
    pub lines: usize,
    pub version: Option<String>,
    pub text: Vec<String>,
    pub levels: Vec<Option<Level>>,
    pub errors: Vec<Pointed>,
    pub culprits: Vec<Blamed>,
    pub launches: Vec<Shown>,
    pub since_clean: Option<SinceClean>,
    pub outside: bool,
}

pub fn outcome(errors: Option<usize>) -> Sentence {
    match errors {
        None => say!("Not read"),
        Some(0) => say!("Clean"),
        Some(n) => say_n!(n, "{} error", "{} errors"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Head<'a> {
    pub previous: bool,
    pub has_other: bool,
    pub path: &'a str,
    pub written: u64,
    pub lines: usize,
    pub about: Sentence,
    pub cut: Sentence,
    pub outcome: Sentence,
    pub errors: usize,
    pub outside: bool,
}

pub fn head(report: &LogsReport) -> Head<'_> {
    let lines = say_n!(report.lines, "{} line", "{} lines");
    let version = report
        .version
        .as_ref()
        .map(|v| Sentence::Plain(format!("RimWorld {v}")));
    let about = Sentence::listed(version.into_iter().chain([lines]).collect());
    let shown = report.text.len();
    let cut = match report.lines > shown {
        true => say_n!(
            report.lines,
            "{} line, the first {} shown",
            "{} lines, the first {} shown",
            shown
        ),
        false => Sentence::default(),
    };
    Head {
        previous: report.previous,
        has_other: report.has_other,
        path: &report.path,
        written: report.written,
        lines: report.lines,
        about,
        cut,
        outcome: outcome(Some(report.errors.len())),
        errors: report.errors.len(),
        outside: report.outside,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchShown<'a> {
    #[serde(flatten)]
    pub launch: &'a Shown,
    pub outcome: Sentence,
}

pub fn launches(report: &LogsReport) -> Vec<LaunchShown<'_>> {
    report
        .launches
        .iter()
        .map(|launch| LaunchShown {
            launch,
            outcome: outcome(launch.errors),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Since<'a> {
    pub time: u64,
    pub ids: &'a [String],
    pub added: String,
    pub removed: String,
    pub reordered: bool,
    pub same: bool,
}

pub fn since(report: &LogsReport) -> Option<Since<'_>> {
    report.since_clean.as_ref().map(|since| Since {
        time: since.time,
        ids: &since.ids,
        added: since.added.join(", "),
        removed: since.removed.join(", "),
        reordered: since.reordered,
        same: since.added.is_empty() && since.removed.is_empty() && !since.reordered,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorShown<'a> {
    #[serde(flatten)]
    pub error: &'a Pointed,
    pub kind: &'a str,
    pub detail: &'a str,
    /// 0 for none.
    pub more: usize,
    pub opened: bool,
    pub trace_text: String,
}

fn kind_of(head: &str) -> (&str, &str) {
    match head.find(':') {
        Some(at) if (1..=LONGEST_KIND).contains(&head[..at].chars().count()) => {
            (&head[..at], head[at + 1..].trim())
        }
        _ => (head, ""),
    }
}

fn breakable(trace: &[String]) -> String {
    let lines: Vec<String> = trace
        .iter()
        .map(|line| {
            let mut out = String::with_capacity(line.len());
            for c in line.trim_end().chars() {
                out.push(c);
                if matches!(c, '.' | '/' | '(' | ',') {
                    out.push('\u{200b}');
                }
            }
            out
        })
        .collect();
    lines.join("\n")
}

pub fn listed<'a>(
    errors: &'a [Pointed],
    filter: &str,
    open_runs: &HashSet<usize>,
) -> Vec<ErrorShown<'a>> {
    errors
        .iter()
        .filter(|error| {
            logs::listed(
                filter,
                &error.head,
                error.first.as_deref(),
                error.run == error.at,
                open_runs.contains(&error.run),
            )
        })
        .map(|error| {
            let (kind, detail) = kind_of(&error.head);
            let leads = error.alike > 1 && error.run == error.at && filter.is_empty();
            ErrorShown {
                error,
                kind,
                detail,
                more: if leads { error.alike - 1 } else { 0 },
                opened: open_runs.contains(&error.run),
                trace_text: breakable(&error.trace),
            }
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line<'a> {
    pub number: usize,
    pub text: &'a str,
    pub error: bool,
    pub warning: bool,
}

pub fn lines<'a>(report: &'a LogsReport, filter: &str) -> Vec<Line<'a>> {
    report
        .text
        .iter()
        .enumerate()
        .filter(|(_, text)| filter.is_empty() || text.to_lowercase().contains(filter))
        .map(|(at, text)| Line {
            number: at + 1,
            text,
            error: report.levels[at] == Some(Level::Error),
            warning: report.levels[at] == Some(Level::Warning),
        })
        .collect()
}

/// The empty name is for errors that point at none.
pub fn errors_of<'a>(report: &'a LogsReport, name: &str) -> Vec<&'a Pointed> {
    report
        .errors
        .iter()
        .filter(|e| e.first.as_deref().unwrap_or_default() == name)
        .collect()
}

/// Always in English.
pub fn help_text(report: &LogsReport, error: &Pointed) -> String {
    let from = error
        .first
        .as_ref()
        .map_or(String::new(), |m| format!("Likely mod: {m}\n"));
    let game = report
        .version
        .as_ref()
        .map_or(String::new(), |v| format!("RimWorld {v}\n"));
    format!(
        "{}\n{}\n\n{game}{from}Logged {} times, first at line {} of the log.",
        error.head,
        error.trace.join("\n"),
        error.count,
        error.line
    )
}

impl App {
    pub fn logs_kept_limit(&self) -> usize {
        self.settings().logs_kept().unwrap_or(10)
    }

    /// `keep_logs` off the caller's thread, one copy at a time.
    pub fn keep_logs_soon(self: &std::sync::Arc<Self>) {
        let app = self.clone();
        self.log_keeping
            .ask("keep logs", (), move |()| app.keep_logs());
    }

    pub fn keep_logs(&self) {
        let paths = self.paths();
        let Some(config) = paths.config.as_ref() else {
            return;
        };
        for log in logs::files(config).iter().take(2).rev() {
            if let Err(problem) = logs::keep(log, &self.kept_logs_dir(), self.logs_kept_limit()) {
                rimstack_core::problem!("log not kept: {problem}");
            }
        }
    }

    pub fn last_start_said(&self) -> Option<LastStartSaid> {
        self.last_start().map(|start| {
            let errors = start.errors.map(|n| match n {
                0 => say!("no errors"),
                n => say_n!(n, "{} different error", "{} different errors"),
            });
            let started = say!("Last started {}", moment(start.time));
            LastStartSaid {
                start,
                line: Sentence::listed(std::iter::once(started).chain(errors).collect()),
            }
        })
    }

    pub fn last_start(&self) -> Option<LastStart> {
        let recorded = launches::read(&self.launches_file()).pop();
        let read = *self.log_read.held();
        launches::last_start(recorded.as_ref(), read, self.log_written() / 1000)
    }

    pub fn logs_report(
        &self,
        previous: bool,
        kept: Option<&Path>,
    ) -> Result<(LogsReport, bool), Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let paths = self.paths();
        let config = paths
            .config
            .as_ref()
            .ok_or_else(|| Error::refused(said!("RimWorld's settings folder was not found.")))?;
        let found = logs::files(config);
        let kept = kept.map(|kept| self.kept_log(kept)).transpose()?;
        let on_last = !previous && kept.is_none();
        let path = kept
            .or_else(|| found.get(usize::from(previous)).cloned())
            .ok_or_else(|| Error::refused(said!("The game has not written a log yet.")))?;
        let text = logs::read(&path)?;
        let written = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let list = self.session().current().to_vec();
        let active = library.arrange(&list).active;
        let mut assemblies: HashMap<String, ModId> = HashMap::new();
        for i in active.iter().filter_map(|m| m.installed) {
            for name in library.get(i).into_iter().flat_map(|m| m.assemblies()) {
                assemblies.entry(name).or_insert(i);
            }
        }
        let ids: Vec<(String, ModId)> = library
            .ids()
            .filter(|(_, m)| m.source != Source::Official)
            .filter_map(|(i, m)| Some((m.about.as_ref().ok()?.package_id.clone(), i)))
            .collect();
        let groups = logs::errors(&text);
        let clues = rimstack_core::mods::blame::Clues {
            assemblies: &assemblies,
            ids: &ids,
        };
        let name_of_mod = |i: ModId| {
            let named = library.get(i).map(|m| m.display_name());
            named.unwrap_or_default().to_string()
        };
        let (errors, culprits) = rimstack_core::mods::blame::of(&groups, &clues, &name_of_mod);
        let file = self.launches_file();
        let mut record = launches::read(&file);
        let mut changed = false;
        if !previous && launches::note_outcome(&mut record, written, groups.len()) {
            changed = true;
            if let Err(problem) = launches::write(&file, &record) {
                rimstack_core::problem!("{problem}");
            }
        }
        if on_last {
            let read = Some(LogRead {
                written,
                previous: false,
                errors: groups.len(),
            });
            let mut last = self.log_read.held();
            changed |= *last != read;
            *last = read;
        }
        let name = |written: &str| {
            let key = id::split_steam(written).0;
            library.name_of(&key).unwrap_or(key)
        };
        let since_clean = launches::since_clean(&record, &list, &name);
        let shown = launches::shown(&record, &list, &logs::kept(&self.kept_logs_dir()));
        let last_launch = shown.first().map_or(0, |launch| launch.time);
        let report = LogsReport {
            previous,
            has_other: found.len() > 1,
            path: path.to_string_lossy().into_owned(),
            written,
            lines: text.lines().count(),
            version: logs::game_version(&text),
            text: text.lines().take(SHOWN_LINES).map(String::from).collect(),
            levels: logs::levels(&text).into_iter().take(SHOWN_LINES).collect(),
            errors,
            culprits,
            launches: shown,
            since_clean,
            outside: on_last && written > last_launch + LAUNCH_SLACK_SECS,
        };
        Ok((report, changed))
    }

    /// Only a path inside the kept-logs folder.
    fn kept_log(&self, path: &Path) -> Result<PathBuf, Error> {
        let dir = self.kept_logs_dir();
        match path.parent() == Some(dir.as_path()) && path.is_file() {
            true => Ok(path.to_path_buf()),
            false => Err(Error::refused(said!(
                "{} is not a kept log",
                path.display()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    fn with_log(tmp: &tempfile::TempDir) {
        let config = tmp.path().join("config");
        std::fs::write(
            config.join("Config/ModsConfig.xml"),
            "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>ann.roads</li></activeMods></ModsConfigData>",
        )
        .unwrap();
        std::fs::write(
            config.join("Player.log"),
            "RimWorld 1.6.4871 rev600\nCould not load texture for Ann.Roads\nCould not load texture for Ann.Roads\nall fine\n",
        )
        .unwrap();
    }

    #[test]
    fn an_error_naming_an_active_mod_is_counted_against_it_and_the_launch_noted() {
        let (tmp, app) = installed();
        with_log(&tmp);
        launches::record(&app.launches_file(), &["ludeon.rimworld".to_string()], 1).unwrap();
        let (report, changed) = app.logs_report(false, None).unwrap();
        assert!(changed);
        let errors = crate::words::as_shown(&listed(&report.errors, "", &HashSet::new()));
        assert_eq!(errors[0]["count"], 2);
        assert_eq!(errors[0]["mod"], "Ann.Roads");
        assert_eq!(errors[0]["kind"], "Could not load texture for Ann.Roads");
        assert_eq!(crate::words::as_shown(&report.culprits)[0]["times"], 2);
        let shown = crate::words::as_shown(&launches(&report));
        assert_eq!(shown[0]["errors"], 1);
        assert_eq!(shown[0]["added"], 1);
        assert_eq!(shown[0]["outcome"], "1 error");
        let said = crate::words::as_shown(&head(&report));
        assert_eq!(said["about"], "RimWorld 1.6.4871 rev600, 4 lines");
        assert_eq!(said["outside"], true);
        assert_eq!(said["cut"], "");
        assert_eq!(launches::read(&app.launches_file())[0].errors, Some(1));
        let found = lines(&report, "texture");
        assert_eq!(found.len(), 2);
        assert!(found[0].error);
        assert!(!found[0].warning);
        assert_eq!(errors_of(&report, "Ann.Roads").len(), 1);
        assert!(help_text(&report, &report.errors[0]).contains("Logged 2 times, first at line 2"));
        assert_eq!(app.last_start().unwrap().errors, Some(1));
        let line = app.last_start_said().unwrap().line.to_string();
        assert!(line.starts_with("Last started 20"), "{line}");
        assert!(line.ends_with(", 1 different error"), "{line}");
        let (_, again) = app.logs_report(false, None).unwrap();
        assert!(!again, "a second read changes nothing");
    }

    #[test]
    fn without_a_log_the_report_says_so_and_only_kept_logs_are_read() {
        let (tmp, app) = installed();
        let problem = app.logs_report(false, None).err().unwrap();
        assert_eq!(problem.kind(), rimstack_core::base::error::Kind::Refused);
        with_log(&tmp);
        let elsewhere = tmp.path().join("config/Player.log");
        let refused = app.logs_report(false, Some(&elsewhere)).err().unwrap();
        assert_eq!(refused.kind(), rimstack_core::base::error::Kind::Refused);
    }

    #[test]
    fn a_kind_is_the_first_line_before_a_short_colon() {
        assert_eq!(kind_of("Exception: boom"), ("Exception", "boom"));
        assert_eq!(kind_of(": none"), (": none", ""));
        let long = format!("{}: tail", "x".repeat(91));
        assert_eq!(kind_of(&long), (long.as_str(), ""));
    }

    #[test]
    fn a_trace_may_break_after_its_dots_brackets_and_slashes() {
        assert_eq!(
            breakable(&["a.b(c) ".into(), "d/e".into()]),
            "a.\u{200b}b(\u{200b}c)\nd/\u{200b}e"
        );
    }
}
