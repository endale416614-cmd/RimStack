use crate::base::error::Error;
use crate::base::path_text::has_extension;
use crate::said;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileTypes {
    Any,
    Only(Vec<String>),
}

impl Default for FileTypes {
    fn default() -> Self {
        FileTypes::Only(vec!["xml".into()])
    }
}

fn is_extension(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

impl FileTypes {
    pub fn parse(text: &str) -> Result<FileTypes, Error> {
        let mut types: Vec<String> = Vec::new();
        for token in text
            .split([',', ';', '|', ' ', '\t'])
            .filter(|t| !t.is_empty())
        {
            if matches!(token, "." | "*" | "*.*") {
                return Ok(FileTypes::Any);
            }
            let extension = token.trim_start_matches('*').trim_start_matches('.');
            if !is_extension(extension) {
                return Err(Error::refused(said!(
                    "\"{}\" is not a file type; write extensions like xml, cs or json, or . for every text file",
                    token
                )));
            }
            let extension = extension.to_lowercase();
            if !types.contains(&extension) {
                types.push(extension);
            }
        }
        match types.is_empty() {
            true => Err(Error::refused(said!(
                "name at least one file type, like xml, or . for every text file"
            ))),
            false => Ok(FileTypes::Only(types)),
        }
    }

    pub fn reads_any_text(&self) -> bool {
        matches!(self, FileTypes::Any)
    }

    pub fn takes(&self, file: &Path) -> bool {
        match self {
            FileTypes::Any => true,
            FileTypes::Only(types) => types.iter().any(|t| has_extension(file, t)),
        }
    }
}

const SEPARATORS: [char; 5] = [',', ';', '|', ' ', '\t'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub ext: String,
    pub count: usize,
}

fn bare(token: &str) -> String {
    token.trim_start_matches(['*', '.']).to_lowercase()
}

fn typing(text: &str) -> (&str, &str) {
    text.split_at(text.rfind(SEPARATORS).map_or(0, |at| at + 1))
}

pub fn suggest(counts: &[(String, usize)], text: &str) -> Vec<Offer> {
    let (head, token) = typing(text);
    let prefix = bare(token);
    let listed: Vec<String> = head
        .split(SEPARATORS)
        .filter(|t| !t.is_empty())
        .map(|t| match matches!(t, "." | "*" | "*.*") {
            true => ".".to_string(),
            false => bare(t),
        })
        .collect();
    if listed.iter().any(|t| t == ".") {
        return Vec::new();
    }
    let mut offers: Vec<Offer> = counts
        .iter()
        .filter(|(ext, _)| ext.starts_with(&prefix) && *ext != prefix && !listed.contains(ext))
        .map(|(ext, count)| Offer {
            ext: ext.clone(),
            count: *count,
        })
        .collect();
    if prefix.is_empty() && token != "." {
        offers.push(Offer {
            ext: ".".into(),
            count: 0,
        });
    }
    offers
}

pub fn complete(text: &str, ext: &str) -> String {
    let (head, _) = typing(text);
    let space = if head.is_empty() || head.ends_with(char::is_whitespace) {
        ""
    } else {
        " "
    };
    let after = if ext == "." { "" } else { ", " };
    format!("{head}{space}{ext}{after}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_split_by_any_of_the_separators_and_kept_once_in_lowercase() {
        let parsed = |text| FileTypes::parse(text).unwrap();
        let only = |types: &[&str]| FileTypes::Only(types.iter().map(|t| t.to_string()).collect());
        assert_eq!(parsed("xml, cs; json"), only(&["xml", "cs", "json"]));
        assert_eq!(parsed("*.XML|.xml  txt"), only(&["xml", "txt"]));
        assert_eq!(parsed("xml"), FileTypes::default());
        assert_eq!(parsed("."), FileTypes::Any);
        assert_eq!(parsed("xml, ."), FileTypes::Any);
    }

    #[test]
    fn a_bad_entry_or_none_is_refused_with_a_sentence() {
        let said = |text| FileTypes::parse(text).unwrap_err().sentence().to_string();
        assert!(said("xml, a/b").starts_with("\"a/b\" is not a file type"));
        assert!(said(" , ;").starts_with("name at least one file type"));
        assert!(said("").starts_with("name at least one file type"));
    }

    #[test]
    fn a_file_is_taken_by_its_extension_in_any_case() {
        let types = FileTypes::parse("xml, cs").unwrap();
        assert!(types.takes(Path::new("a/B.XML")) && types.takes(Path::new("c.cs")));
        assert!(!types.takes(Path::new("c.json")) && !types.takes(Path::new("noext")));
        assert!(FileTypes::Any.takes(Path::new("noext")));
    }

    #[test]
    fn suggestions_follow_the_token_typed_and_skip_what_is_listed() {
        let counts: Vec<(String, usize)> = [("xml", 9), ("xsl", 4), ("cs", 3), ("json", 1)]
            .map(|(e, n)| (e.to_string(), n))
            .into();
        let exts = |text| {
            suggest(&counts, text)
                .into_iter()
                .map(|o| o.ext)
                .collect::<Vec<_>>()
        };
        assert_eq!(exts(""), ["xml", "xsl", "cs", "json", "."]);
        assert_eq!(exts("x"), ["xml", "xsl"]);
        assert_eq!(exts("*.X"), ["xml", "xsl"]);
        assert_eq!(exts("xml, c"), ["cs"]);
        assert_eq!(exts("xml; "), ["xsl", "cs", "json", "."]);
        assert_eq!(exts("xml"), Vec::<String>::new());
        assert_eq!(exts("."), ["xml", "xsl", "cs", "json"]);
        assert!(exts(". x").is_empty());
        assert_eq!(suggest(&counts, "c")[0].count, 3);
    }

    #[test]
    fn deleting_a_token_that_offered_nothing_offers_everything_again() {
        let counts: Vec<(String, usize)> = [("xml", 9), ("md", 2)]
            .map(|(e, n)| (e.to_string(), n))
            .into();
        assert!(suggest(&counts, "md").is_empty());
        assert_eq!(suggest(&counts, "").len(), 3);
    }

    #[test]
    fn completion_replaces_the_token_and_adds_a_separator() {
        assert_eq!(complete("x", "xml"), "xml, ");
        assert_eq!(complete("xml,c", "cs"), "xml, cs, ");
        assert_eq!(complete("xml; c", "cs"), "xml; cs, ");
        assert_eq!(complete("", "."), ".");
    }
}
