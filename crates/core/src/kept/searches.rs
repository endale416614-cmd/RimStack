use crate::base::error::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Searches kept, newest first.
pub const MOST_KEPT: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Search {
    pub text: String,
    pub case: bool,
    pub regex: bool,
    pub whole_word: bool,
    pub place: String,
    pub types: String,
    pub languages: bool,
    pub source: bool,
    pub textures: bool,
    pub git: bool,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            text: String::new(),
            case: false,
            regex: false,
            whole_word: false,
            place: String::new(),
            types: "xml".into(),
            languages: true,
            source: true,
            textures: true,
            git: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Searches(Vec<Search>);

impl Searches {
    pub fn read(file: &Path) -> Self {
        let kept = crate::base::atomic_file::read_json(file).map(serde_json::from_value);
        let mut kept: Self = kept.and_then(Result::ok).unwrap_or_default();
        kept.0.retain(|search| !search.text.is_empty());
        kept.0.truncate(MOST_KEPT);
        kept
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        crate::base::atomic_file::write_json(file, self)
    }

    pub fn all(&self) -> &[Search] {
        &self.0
    }

    pub fn with(mut self, search: Search) -> Self {
        self.0.retain(|kept| *kept != search);
        self.0.insert(0, search);
        self.0.truncate(MOST_KEPT);
        self
    }

    pub fn revised(mut self, search: Search) -> Self {
        if self
            .0
            .first()
            .is_some_and(|first| first.text == search.text)
        {
            self.0.remove(0);
        }
        self.with(search)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(text: &str) -> Search {
        Search {
            text: text.into(),
            ..Search::default()
        }
    }

    #[test]
    fn searches_are_kept_newest_first_without_repeats_and_only_ten() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kept/searches.json");
        assert_eq!(Searches::read(&file), Searches::default());
        let mut kept = Searches::default();
        for n in 0..12 {
            kept = kept.with(asked(&n.to_string()));
        }
        kept = kept.with(asked("5"));
        kept.write(&file).unwrap();
        let read = Searches::read(&file);
        let texts: Vec<&str> = read.all().iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["5", "11", "10", "9", "8", "7", "6", "4", "3", "2"]);
    }

    #[test]
    fn a_search_revised_replaces_the_newest_of_the_same_text_only() {
        let kept = Searches::default().with(asked("Steel")).with(asked("Rock"));
        let cased = Search {
            case: true,
            ..asked("Rock")
        };
        let kept = kept.revised(cased.clone());
        assert_eq!(kept.all(), [cased, asked("Steel")]);
        let kept = kept.revised(asked("Wood"));
        let texts: Vec<&str> = kept.all().iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["Wood", "Rock", "Steel"]);
    }

    #[test]
    fn an_older_record_missing_fields_reads_with_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("searches.json");
        std::fs::write(&file, r#"[{"text":"Steel","regex":true}]"#).unwrap();
        let read = Searches::read(&file);
        assert!(read.all()[0].regex && read.all()[0].languages && !read.all()[0].case);
        std::fs::write(&file, "damaged").unwrap();
        assert_eq!(Searches::read(&file), Searches::default());
    }
}
