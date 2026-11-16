//! `RimStack.conf`, where the appearance and the Mods screen's places are kept: `[Group]` lines,
//! then `key=value` lines. A value holding a comma or a quote is quoted, with `\"` and `\\`
//! inside. Only the keys set are written back; every other line stays as it was.

use crate::base::error::Error;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    lines: Vec<String>,
}

impl Prefs {
    /// A missing file is no preference yet.
    pub fn read(file: &Path) -> Self {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        Self {
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|e| Error::file(folder, e))?;
        }
        let mut text = self.lines.join("\n");
        text.push('\n');
        crate::base::atomic_file::write(file, text)
    }

    pub fn get(&self, group: &str, key: &str) -> Option<String> {
        let (_, found) = self.find(group, key);
        let line = &self.lines[found?];
        Some(unquote(line.split_once('=')?.1))
    }

    pub fn flag(&self, group: &str, key: &str) -> Option<bool> {
        match self.get(group, key)?.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    pub fn number(&self, group: &str, key: &str) -> Option<f64> {
        self.get(group, key)?.parse().ok()
    }

    pub fn set(&mut self, group: &str, key: &str, value: &str) {
        let line = format!("{key}={}", quote(value));
        match self.find(group, key) {
            (_, Some(at)) => self.lines[at] = line,
            (Some(end), None) => self.lines.insert(end, line),
            (None, None) => {
                if self
                    .lines
                    .last()
                    .is_some_and(|last| !last.trim().is_empty())
                {
                    self.lines.push(String::new());
                }
                self.lines.push(format!("[{group}]"));
                self.lines.push(line);
            }
        }
    }

    /// Where `group` ends (after its last key), and the line of `key` in it.
    fn find(&self, group: &str, key: &str) -> (Option<usize>, Option<usize>) {
        let heading = format!("[{group}]");
        let Some(start) = self.lines.iter().position(|line| line.trim() == heading) else {
            return (None, None);
        };
        let mut end = start + 1;
        for (at, line) in self.lines.iter().enumerate().skip(start + 1) {
            let line = line.trim();
            if line.starts_with('[') {
                break;
            }
            if line.is_empty() {
                continue;
            }
            end = at + 1;
            if line
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == key)
            {
                return (Some(end), Some(at));
            }
        }
        (Some(end), None)
    }
}

fn quote(value: &str) -> String {
    match value.contains([',', '"', ';', '=']) || value.starts_with(' ') || value.ends_with(' ') {
        true => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
        false => value.to_string(),
    }
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_string();
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.extend(chars.next()),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const QT_WROTE: &str = "[Appearance]\ntheme=light\ntextScale=1.25\nhintsSeen=true\ncategoryColours=\"2=Rose,5=Sky\"\n\n[Mods]\ninactive=479\n";

    #[test]
    fn what_qt_wrote_is_read_with_its_quotes_taken_off() {
        let file = tempfile::tempdir().unwrap();
        let path = file.path().join("RimStack.conf");
        std::fs::write(&path, QT_WROTE).unwrap();
        let prefs = Prefs::read(&path);
        assert_eq!(prefs.get("Appearance", "theme").as_deref(), Some("light"));
        assert_eq!(prefs.number("Appearance", "textScale"), Some(1.25));
        assert_eq!(prefs.flag("Appearance", "hintsSeen"), Some(true));
        assert_eq!(
            prefs.get("Appearance", "categoryColours").as_deref(),
            Some("2=Rose,5=Sky")
        );
        assert_eq!(prefs.number("Mods", "inactive"), Some(479.));
        assert_eq!(
            prefs.get("Mods", "theme"),
            None,
            "a key belongs to its group"
        );
    }

    #[test]
    fn a_key_set_is_written_in_its_group_and_every_other_line_stays() {
        let file = tempfile::tempdir().unwrap();
        let path = file.path().join("settings").join("RimStack.conf");
        let mut prefs = Prefs {
            lines: QT_WROTE.lines().map(str::to_string).collect(),
        };
        prefs.set("Appearance", "theme", "oled");
        prefs.set("Appearance", "reduceMotion", "true");
        prefs.set("Mods", "listName", "a, b");
        prefs.set("Window", "maximized", "false");
        prefs.write(&path).unwrap();
        let again = Prefs::read(&path);
        assert_eq!(again.get("Appearance", "theme").as_deref(), Some("oled"));
        assert_eq!(again.flag("Appearance", "reduceMotion"), Some(true));
        assert_eq!(again.get("Mods", "listName").as_deref(), Some("a, b"));
        assert_eq!(again.flag("Window", "maximized"), Some(false));
        assert_eq!(
            again.get("Appearance", "categoryColours").as_deref(),
            Some("2=Rose,5=Sky")
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("reduceMotion=true\n\n[Mods]"), "{text}");
    }
}
