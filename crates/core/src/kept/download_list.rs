//! Kept in `downloadList.json`, a JSON array in the order added, one object per mod:
//! `{"id", "title", "size": bytes|null, "why": "added"|"missing"|"needed:<Workshop id>", "failed": <Sentence::to_kept>|null}`.

use crate::base::error::Error;
use crate::base::sentence::Sentence;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    Added,
    NeededBy(String),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub size: Option<u64>,
    pub why: Why,
    pub failed: Option<Sentence>,
}

impl Row {
    pub fn new(id: impl Into<String>, title: impl Into<String>, why: Why) -> Self {
        Row {
            id: id.into(),
            title: title.into(),
            size: None,
            why,
            failed: None,
        }
    }

    fn to_value(&self) -> Value {
        let why = match &self.why {
            Why::Added => "added".to_string(),
            Why::Missing => "missing".to_string(),
            Why::NeededBy(id) => format!("needed:{id}"),
        };
        json!({
            "id": self.id,
            "title": self.title,
            "size": self.size,
            "why": why,
            "failed": self.failed.as_ref().map(Sentence::to_kept),
        })
    }

    fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key)?.as_str().map(String::from);
        let id = text("id").filter(|id| !id.is_empty())?;
        let why = match text("why").as_deref() {
            Some("missing") => Why::Missing,
            Some(other) => other
                .strip_prefix("needed:")
                .map_or(Why::Added, |by| Why::NeededBy(by.to_string())),
            None => Why::Added,
        };
        Some(Row {
            title: text("title").unwrap_or_else(|| id.clone()),
            id,
            size: value.get("size").and_then(Value::as_u64),
            why,
            failed: value
                .get("failed")
                .filter(|failed| !failed.is_null())
                .map(Sentence::from_kept_value),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadList {
    rows: Vec<Row>,
}

pub fn file_in(data_dir: &Path) -> PathBuf {
    data_dir.join("downloadList.json")
}

impl DownloadList {
    pub fn read(file: &Path) -> Self {
        let rows = crate::base::atomic_file::read_json(file)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let mut list = DownloadList::default();
        for row in rows.iter().filter_map(Row::from_value) {
            list.add(row);
        }
        list
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        let rows: Vec<Value> = self.rows.iter().map(Row::to_value).collect();
        crate::base::atomic_file::write_json(file, &rows)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn ids(&self) -> Vec<String> {
        self.rows.iter().map(|row| row.id.clone()).collect()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.rows.iter().any(|row| row.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// `false`, and the row is dropped, when its mod is already on the list.
    pub fn add(&mut self, row: Row) -> bool {
        let new = !self.contains(&row.id);
        if new {
            self.rows.push(row);
        }
        new
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.rows.len();
        self.rows.retain(|row| row.id != id);
        self.rows.len() != before
    }

    pub fn mark_failed(&mut self, id: &str, reason: Sentence) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == id) {
            row.failed = Some(reason);
        }
    }

    pub fn clear_failures(&mut self) {
        self.rows.iter_mut().for_each(|row| row.failed = None);
    }

    pub fn set_size(&mut self, id: &str, size: u64) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == id) {
            row.size = Some(size);
        }
    }

    pub fn remove_downloaded(&mut self, downloaded: &[String]) {
        self.rows.retain(|row| !downloaded.contains(&row.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::said;

    #[test]
    fn a_list_is_kept_and_read_back_with_its_reasons_and_failures() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path());
        assert!(DownloadList::read(&file).is_empty());
        let mut list = DownloadList::default();
        list.add(Row::new("100", "Alpha", Why::Added));
        list.add(Row::new("200", "Beta", Why::NeededBy("100".into())));
        list.add(Row::new("300", "Gamma", Why::Missing));
        list.set_size("200", 4096);
        list.mark_failed("300", said!("the download failed: {}", "offline"));
        list.write(&file).unwrap();
        assert_eq!(DownloadList::read(&file), list);
        assert_eq!(list.ids(), ["100", "200", "300"]);
    }

    #[test]
    fn adding_one_already_there_is_refused() {
        let mut list = DownloadList::default();
        assert!(list.add(Row::new("100", "Alpha", Why::Added)));
        assert!(!list.add(Row::new("100", "Other", Why::Missing)));
        assert_eq!(list.rows().len(), 1);
        assert_eq!(list.rows()[0].title, "Alpha");
    }

    #[test]
    fn downloaded_rows_leave_and_a_failed_one_stays() {
        let mut list = DownloadList::default();
        for id in ["1", "2", "3"] {
            list.add(Row::new(id, id, Why::Added));
        }
        list.mark_failed("2", said!("it failed"));
        list.remove_downloaded(&["1".into(), "3".into()]);
        assert_eq!(list.ids(), ["2"]);
        assert!(list.rows()[0].failed.is_some());
        list.clear_failures();
        assert!(list.rows()[0].failed.is_none());
        assert!(list.remove("2"));
        assert!(!list.remove("2"));
    }

    #[test]
    fn a_damaged_file_or_row_never_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path());
        std::fs::write(&file, "{ not json").unwrap();
        assert!(DownloadList::read(&file).is_empty());
        std::fs::write(&file, r#"[{"title":"no id"},{"id":"7"},5]"#).unwrap();
        let list = DownloadList::read(&file);
        assert_eq!(list.ids(), ["7"]);
        assert_eq!(list.rows()[0].title, "7");
        assert_eq!(list.rows()[0].why, Why::Added);
    }
}
