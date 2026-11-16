//! Sentences are put in words here, on the window's thread, as they are handed to a view.

use icu_decimal::DecimalFormatter;
use icu_decimal::input::Decimal;
use icu_decimal::options::DecimalFormatterOptions;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::{Language, MARK, Sentence};
use rimstack_core::base::translate::{Counting, LOCALE};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Words {
    table: HashMap<String, String>,
    locale: String,
    counting: Counting,
    digits: DecimalFormatter,
}

fn digits_for(locale: &str) -> DecimalFormatter {
    let options = DecimalFormatterOptions::default();
    let parsed = icu_locale_core::Locale::try_from_str(locale.trim());
    let english = icu_locale_core::locale!("en");
    parsed
        .ok()
        .and_then(|locale| DecimalFormatter::try_new((&locale).into(), options).ok())
        .or_else(|| DecimalFormatter::try_new((&english).into(), options).ok())
        .expect("English digits are compiled in")
}

impl Default for Words {
    fn default() -> Self {
        Self::of(&Map::new())
    }
}

impl Words {
    pub fn of(table: &Map<String, Value>) -> Self {
        let said = |(english, said): (&String, &Value)| {
            let said = said.as_str().filter(|said| !said.is_empty())?;
            Some((english.clone(), said.to_string()))
        };
        let table: HashMap<String, String> = table.iter().filter_map(said).collect();
        let locale = table.get(LOCALE).cloned().unwrap_or_default();
        Self {
            counting: Counting::of(&locale),
            digits: digits_for(&locale),
            locale,
            table,
        }
    }
}

impl Language for Words {
    fn word<'a>(&'a self, english: &'a str) -> &'a str {
        self.table.get(english).map_or(english, String::as_str)
    }

    fn counting(&self) -> &Counting {
        &self.counting
    }

    fn number(&self, n: i64) -> String {
        self.digits.format(&Decimal::from(n)).to_string()
    }
}

thread_local! {
    /// Set on the window's thread only.
    static ON_SCREEN: RefCell<Rc<Words>> = RefCell::new(Rc::new(Words::default()));
}

pub fn show_in(words: Words) {
    ON_SCREEN.with(|on_screen| *on_screen.borrow_mut() = Rc::new(words));
}

/// How many texts the language in use words otherwise than English, as `Theme.words` counted.
pub fn translated() -> usize {
    ON_SCREEN.with(|on_screen| {
        on_screen
            .borrow()
            .table
            .keys()
            .filter(|key| key.as_str() != LOCALE)
            .count()
    })
}

pub fn put(sentence: &Sentence) -> String {
    let words = ON_SCREEN.with(|on_screen| on_screen.borrow().clone());
    sentence.put(words.as_ref())
}

/// The locale numbers are written in: `LC_NUMERIC` before the language.
fn number_locale() -> String {
    let from_env = ["LC_ALL", "LC_NUMERIC", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX");
    let named = from_env.or_else(sys_locale::get_locale).unwrap_or_default();
    named
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
}

thread_local! {
    static NUMBERS: DecimalFormatter = digits_for(&number_locale());
}

/// A count as `Theme.count` wrote it, grouped the system's way: "2 000" in French.
pub fn count(n: usize) -> String {
    NUMBERS.with(|digits| digits.format(&Decimal::from(n as u64)).to_string())
}

pub fn counted(n: usize, one: &str, many: &str, locale: &str) -> String {
    let words = ON_SCREEN.with(|on_screen| on_screen.borrow().clone());
    if words.locale == locale {
        return words.counting.pick(n, one, many).to_string();
    }
    Counting::of(locale).pick(n, one, many).to_string()
}

pub fn said(sentence: &Sentence) -> String {
    put(sentence)
}

pub fn said_of(problem: &Error) -> String {
    said(problem.sentence())
}

/// Empty when it was done.
pub fn why_not<T>(done: &Result<T, Error>) -> String {
    done.as_ref().err().map_or_else(String::new, said_of)
}

pub fn as_shown(value: &impl serde::Serialize) -> Value {
    fn shown(value: Value) -> Value {
        match value {
            Value::Object(fields) if fields.len() == 1 && fields.contains_key(MARK) => {
                Value::from(put(&Sentence::from_kept_value(&fields[MARK])))
            }
            Value::Object(fields) => {
                Value::Object(fields.into_iter().map(|(k, v)| (k, shown(v))).collect())
            }
            Value::Array(items) => Value::Array(items.into_iter().map(shown).collect()),
            other => other,
        }
    }
    match serde_json::to_value(value) {
        Ok(value) => shown(value),
        Err(problem) => {
            rimstack_core::problem!("not given to the window: {problem}");
            Value::Null
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstack_core::say_n;

    #[test]
    fn a_sentence_is_shown_in_the_words_set_and_english_before_any() {
        let counted = say_n!(0, "{} mod", "{} mods");
        assert_eq!(put(&counted), "0 mods");
        let table = serde_json::json!({ "@locale": "fr", "{} mod": "{} mod", "{} mods": "{} mods", "Play": "" });
        let words = Words::of(table.as_object().unwrap());
        assert_eq!(
            words.word("Play"),
            "Play",
            "an empty wording leaves the English"
        );
        show_in(words);
        assert_eq!(put(&counted), "0 mod");
        show_in(Words::default());
    }

    #[test]
    fn numbers_are_grouped_as_the_language_does_and_the_count_still_picks_the_form() {
        let expected = [
            ("en", "1,932", "20,139"),
            ("fr", "1\u{202f}932", "20\u{202f}139"),
            ("de", "1.932", "20.139"),
            ("ru", "1\u{a0}932", "20\u{a0}139"),
            ("pl", "1932", "20\u{a0}139"),
        ];
        for (locale, small, large) in expected {
            let table = serde_json::json!({ "@locale": locale });
            let words = Words::of(table.as_object().unwrap());
            for (n, shown) in [(1932, small), (20139, large), (999, "999")] {
                let sentence = say_n!(n, "{} mod", "{} mods");
                assert_eq!(sentence.put(&words), format!("{shown} mods"), "{locale}");
                let plain = rimstack_core::say!("{} inactive", n);
                assert_eq!(plain.put(&words), format!("{shown} inactive"), "{locale}");
            }
        }
        assert_eq!(
            rimstack_core::say!("{} inactive", 1932).to_string(),
            "1932 inactive"
        );
    }
}
