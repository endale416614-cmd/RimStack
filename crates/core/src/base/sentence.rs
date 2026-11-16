//! `core` holds no language's words: whoever shows a sentence gives the language.
//! `Display` is English.

use crate::base::translate::Counting;
use serde::ser::SerializeMap;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sentence {
    /// Shown as it came, never looked up in a language.
    Plain(String),
    Number(i64),
    Said {
        english: Cow<'static, str>,
        parts: Vec<Sentence>,
    },
    Counted {
        n: usize,
        one: Cow<'static, str>,
        many: Cow<'static, str>,
        parts: Vec<Sentence>,
    },
    Joined {
        by: Cow<'static, str>,
        parts: Vec<Sentence>,
    },
}

pub trait Language {
    /// Its English where the language has no text.
    fn word<'a>(&'a self, english: &'a str) -> &'a str;
    fn counting(&self) -> &Counting;
    fn number(&self, n: i64) -> String;
}

pub struct English;

impl Language for English {
    fn word<'a>(&'a self, english: &'a str) -> &'a str {
        english
    }

    fn counting(&self) -> &Counting {
        static ENGLISH: std::sync::LazyLock<Counting> =
            std::sync::LazyLock::new(|| Counting::of("en"));
        &ENGLISH
    }

    fn number(&self, n: i64) -> String {
        n.to_string()
    }
}

pub struct Part(pub Sentence);

pub struct Inside<'a>(&'a Sentence);

pub struct Pick<'a, T>(pub &'a T);

pub trait Exact {
    fn part(&self) -> Part;
}

pub trait Shown {
    fn part(&self) -> Part;
}

macro_rules! numbers {
    ($($int:ty),*) => {$(
        impl Exact for Pick<'_, $int> {
            fn part(&self) -> Part {
                Part(Sentence::Number(i64::try_from(*self.0).unwrap_or(i64::MAX)))
            }
        }
    )*};
}

numbers!(usize, u8, u16, u32, u64, isize, i8, i16, i32, i64);

impl<T: Display> Shown for &Pick<'_, T> {
    fn part(&self) -> Part {
        Part(Sentence::Plain(self.0.to_string()))
    }
}

impl Exact for Pick<'_, Inside<'_>> {
    fn part(&self) -> Part {
        Part(self.0.0.clone())
    }
}

fn fill(form: &str, parts: &[String]) -> String {
    let mut pieces = form.split("{}");
    let mut out = pieces.next().unwrap_or_default().to_string();
    for (piece, part) in pieces.zip(parts) {
        out.push_str(part);
        out.push_str(piece);
    }
    out
}

impl Sentence {
    pub fn said(english: &'static str, parts: Vec<Part>) -> Self {
        Sentence::Said {
            english: Cow::Borrowed(english),
            parts: parts.into_iter().map(|p| p.0).collect(),
        }
    }

    pub fn counted(n: usize, one: &'static str, many: &'static str, parts: Vec<Part>) -> Self {
        Sentence::Counted {
            n,
            one: Cow::Borrowed(one),
            many: Cow::Borrowed(many),
            parts: parts.into_iter().map(|p| p.0).collect(),
        }
    }

    pub fn named(english: &'static str) -> Self {
        Sentence::said(english, Vec::new())
    }

    pub fn listed(all: Vec<Sentence>) -> Self {
        Sentence::joined(", ", all)
    }

    pub fn joined(by: &'static str, all: Vec<Sentence>) -> Self {
        Sentence::Joined {
            by: Cow::Borrowed(by),
            parts: all,
        }
    }

    pub fn inside(&self) -> Inside<'_> {
        Inside(self)
    }

    pub fn put(&self, language: &dyn Language) -> String {
        let all =
            |parts: &[Sentence]| -> Vec<String> { parts.iter().map(|p| p.put(language)).collect() };
        match self {
            Sentence::Plain(text) => text.clone(),
            Sentence::Number(n) => language.number(*n),
            Sentence::Said { english, parts } => fill(language.word(english), &all(parts)),
            Sentence::Counted {
                n,
                one,
                many,
                parts,
            } => {
                let (one, many) = (language.word(one), language.word(many));
                let form = language.counting().pick(*n, one, many);
                let mut with_count = vec![language.number(i64::try_from(*n).unwrap_or(i64::MAX))];
                with_count.extend(all(parts));
                fill(form, &with_count)
            }
            Sentence::Joined { by, parts } => {
                let shown: Vec<String> = all(parts).into_iter().filter(|p| !p.is_empty()).collect();
                shown.join(by)
            }
        }
    }

    /// Format: `["Updated {}: {}", "Harmony", "35a8963"]` is a text and its parts; a
    /// part is a string or a sentence itself. `["#", n, one, many, part…]` is a count,
    /// `[",", …]` a list, `["+", by, …]` parts with another separator.
    /// Read back by `from_kept` in the language chosen then.
    pub fn to_kept(&self) -> Value {
        let all = |parts: &[Sentence]| parts.iter().map(Sentence::to_kept).collect::<Vec<_>>();
        match self {
            Sentence::Plain(text) => Value::from(text.as_str()),
            Sentence::Number(n) => Value::from(*n),
            Sentence::Said { english, parts } => std::iter::once(Value::from(english.as_ref()))
                .chain(all(parts))
                .collect(),
            Sentence::Counted {
                n,
                one,
                many,
                parts,
            } => [
                Value::from("#"),
                Value::from(*n),
                Value::from(one.as_ref()),
                Value::from(many.as_ref()),
            ]
            .into_iter()
            .chain(all(parts))
            .collect(),
            Sentence::Joined { by, parts } if by == ", " => std::iter::once(Value::from(","))
                .chain(all(parts))
                .collect(),
            Sentence::Joined { by, parts } => [Value::from("+"), Value::from(by.as_ref())]
                .into_iter()
                .chain(all(parts))
                .collect(),
        }
    }

    /// An older record's plain English is a text to look up.
    pub fn from_kept(record: &str) -> Self {
        match serde_json::from_str::<Value>(record) {
            Ok(value @ Value::Array(_)) => Sentence::from_kept_value(&value),
            _ => Sentence::Said {
                english: Cow::Owned(record.to_string()),
                parts: Vec::new(),
            },
        }
    }

    pub fn from_kept_value(value: &Value) -> Self {
        let Value::Array(all) = value else {
            if let Some(n) = value.as_i64() {
                return Sentence::Number(n);
            }
            return Sentence::Plain(match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            });
        };
        let text = |at: usize| match all.get(at) {
            Some(Value::String(text)) => text.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        let parts = |from: usize| {
            all.iter()
                .skip(from)
                .map(Sentence::from_kept_value)
                .collect()
        };
        match text(0).as_str() {
            "," => Sentence::Joined {
                by: Cow::Borrowed(", "),
                parts: parts(1),
            },
            "+" => Sentence::Joined {
                by: Cow::Owned(text(1)),
                parts: parts(2),
            },
            "#" => Sentence::Counted {
                n: text(1).parse().unwrap_or(0),
                one: Cow::Owned(text(2)),
                many: Cow::Owned(text(3)),
                parts: parts(4),
            },
            english => Sentence::Said {
                english: Cow::Owned(english.to_string()),
                parts: parts(1),
            },
        }
    }
}

impl Display for Sentence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.put(&English))
    }
}

/// The key a serialized sentence is marked with, for the app to put in words.
pub const MARK: &str = "@sentence";

impl serde::Serialize for Sentence {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        let mut map = to.serialize_map(Some(1))?;
        map.serialize_entry(MARK, &self.to_kept())?;
        map.end()
    }
}

impl Default for Sentence {
    fn default() -> Self {
        Sentence::listed(Vec::new())
    }
}

impl Sentence {
    pub fn is_empty(&self) -> bool {
        match self {
            Sentence::Plain(text) => text.is_empty(),
            Sentence::Joined { parts, .. } => parts.iter().all(Sentence::is_empty),
            _ => false,
        }
    }
}

impl From<String> for Sentence {
    fn from(text: String) -> Self {
        Sentence::Plain(text)
    }
}

impl From<&str> for Sentence {
    fn from(text: &str) -> Self {
        Sentence::Plain(text.to_string())
    }
}

/// Pass a sentence inside as `other.inside()`, an error as `problem.within()`.
#[macro_export]
macro_rules! say {
    ($text:literal $(, $part:expr)* $(,)?) => {
        $crate::base::sentence::Sentence::said(
            $text,
            vec![$({
                #[allow(unused_imports)]
                use $crate::base::sentence::{Exact, Shown};
                (&$crate::base::sentence::Pick(&$part)).part()
            }),*],
        )
    };
}

#[macro_export]
macro_rules! said {
    ($($all:tt)*) => { $crate::say!($($all)*) };
}

#[macro_export]
macro_rules! say_n {
    ($n:expr, $one:literal, $many:literal $(, $part:expr)* $(,)?) => {
        $crate::base::sentence::Sentence::counted(
            $n,
            $one,
            $many,
            vec![$({
                #[allow(unused_imports)]
                use $crate::base::sentence::{Exact, Shown};
                (&$crate::base::sentence::Pick(&$part)).part()
            }),*],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct French(HashMap<&'static str, &'static str>, Counting);

    impl Language for French {
        fn word<'a>(&'a self, english: &'a str) -> &'a str {
            self.0.get(english).copied().unwrap_or(english)
        }

        fn counting(&self) -> &Counting {
            &self.1
        }

        fn number(&self, n: i64) -> String {
            n.to_string()
        }
    }

    fn french() -> French {
        French(
            HashMap::from([
                ("Needs {}, which is {}.", "Requiert {}, qui est {}."),
                ("switched off", "désactivé"),
                ("{} mod", "{} mod"),
                ("{} mods", "{} mods"),
            ]),
            Counting::of("fr"),
        )
    }

    #[test]
    fn a_sentence_is_english_unless_a_language_is_given() {
        let off = crate::say!("switched off");
        let needs = crate::say!("Needs {}, which is {}.", "Lib", off.inside());
        assert_eq!(needs.to_string(), "Needs Lib, which is switched off.");
        assert_eq!(needs.put(&french()), "Requiert Lib, qui est désactivé.");
        let none = crate::say_n!(0, "{} mod", "{} mods");
        assert_eq!(
            (none.to_string(), none.put(&french())),
            ("0 mods".into(), "0 mod".into())
        );
        let both = Sentence::listed(vec!["a".into(), crate::say_n!(2, "{} mod", "{} mods")]);
        assert_eq!(both.to_string(), "a, 2 mods");
        assert_eq!(Sentence::from("bad {} value").to_string(), "bad {} value");
    }

    #[test]
    fn a_sentence_kept_is_read_back_whole_and_an_older_record_is_looked_up() {
        let off = crate::say!("switched off");
        let all = Sentence::listed(vec![
            crate::say!("Needs {}, which is {}.", "Lib", off.inside()),
            crate::say_n!(3, "{} mod", "{} mods"),
        ]);
        let record = all.to_kept().to_string();
        assert_eq!(Sentence::from_kept(&record), all);
        let spaced = Sentence::joined("   ", vec!["a".into(), Sentence::default(), "b".into()]);
        assert_eq!(spaced.to_string(), "a   b");
        assert_eq!(Sentence::from_kept(&spaced.to_kept().to_string()), spaced);
        assert_eq!(
            Sentence::from_kept(
                "[\"Saved the load order: {}\",[\",\",[\"#\",\"2\",\"{} moved\",\"{} moved\"]]]"
            )
            .to_string(),
            "Saved the load order: 2 moved"
        );
        assert_eq!(
            Sentence::from_kept("switched off").put(&french()),
            "désactivé"
        );
        assert_eq!(Sentence::from_kept("[broken").to_string(), "[broken");
    }

    #[test]
    fn a_sentence_is_marked_where_it_is_handed_on() {
        let said = serde_json::to_value(crate::say!("Needs {}, which is {}.", 1, 2)).unwrap();
        assert_eq!(said[MARK][0], "Needs {}, which is {}.");
        assert_eq!(said[MARK][1], 1);
        let named = serde_json::to_value(crate::say!("Needs {}", "1")).unwrap();
        assert_eq!(named[MARK][1], "1");
    }
}
