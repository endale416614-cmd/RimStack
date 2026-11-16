use crate::base::error::Error;
use crate::base::sentence::Sentence;
use crate::said;
use regex::{Regex, RegexBuilder};

const SIZE_LIMIT: usize = 8 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Switches {
    pub case: bool,
    pub regex: bool,
    pub whole_word: bool,
}

#[derive(Debug, Clone)]
pub struct Pattern(Regex);

impl Pattern {
    pub fn new(text: &str, switches: Switches) -> Result<Self, Error> {
        if text.is_empty() {
            return Err(Error::refused(said!("there is nothing to search for")));
        }
        let body = match switches.regex {
            true => text.to_string(),
            false => regex::escape(text),
        };
        let source = match switches.whole_word {
            true => format!(r"\b(?:{body})\b"),
            false => body,
        };
        RegexBuilder::new(&source)
            .case_insensitive(!switches.case)
            .size_limit(SIZE_LIMIT)
            .build()
            .map(Self)
            .map_err(|problem| {
                let said: Sentence = said!("not a valid pattern: {}", problem.to_string());
                Error::format(said)
            })
    }

    pub fn find_in<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Iterator<Item = std::ops::Range<usize>> + 'a {
        self.0
            .find_iter(text)
            .map(|found| found.range())
            .filter(|range| !range.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str, switches: Switches, in_text: &str) -> Vec<String> {
        let pattern = Pattern::new(text, switches).unwrap();
        pattern
            .find_in(in_text)
            .map(|r| in_text[r].to_string())
            .collect()
    }

    #[test]
    fn plain_text_ignores_case_and_is_not_a_regex() {
        let none = Switches::default();
        assert_eq!(
            found("steel", none, "Steel steel STEEL"),
            ["Steel", "steel", "STEEL"]
        );
        assert_eq!(found("a.c", none, "abc a.c"), ["a.c"]);
    }

    #[test]
    fn case_regex_and_whole_word_switches() {
        let case = Switches {
            case: true,
            ..Default::default()
        };
        assert_eq!(found("Steel", case, "Steel steel"), ["Steel"]);
        let regex = Switches {
            regex: true,
            ..Default::default()
        };
        assert_eq!(found(r"st\w+l", regex, "steel stoop"), ["steel"]);
        let word = Switches {
            whole_word: true,
            ..Default::default()
        };
        assert_eq!(
            found("steel", word, "steel steelplate a_steel steel."),
            ["steel", "steel"]
        );
        assert_eq!(found("été", word, "été étés"), ["été"]);
    }

    #[test]
    fn a_bad_regex_or_an_empty_text_is_an_error() {
        let regex = Switches {
            regex: true,
            ..Default::default()
        };
        assert!(Pattern::new("(", regex).is_err());
        assert!(Pattern::new("", regex).is_err());
        assert!(Pattern::new("(", Switches::default()).is_ok());
    }

    #[test]
    fn a_regex_too_large_is_refused() {
        let regex = Switches {
            regex: true,
            ..Default::default()
        };
        assert!(Pattern::new(r"\w{1000}{1000}", regex).is_err());
    }
}
