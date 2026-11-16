//! `Display` is English; a failure shown on screen goes through `sentence()`
//! in the chosen language.

use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Format,
    Program,
    Network,
    Refused,
}

pub use crate::base::sentence::{Inside, Part, Sentence};

type Cause = Arc<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
pub struct Error {
    kind: Kind,
    said: Sentence,
    cause: Option<Cause>,
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.said == other.said
    }
}

impl Eq for Error {}

impl Error {
    pub fn new(kind: Kind, said: impl Into<Sentence>) -> Self {
        Self {
            kind,
            said: said.into(),
            cause: None,
        }
    }

    pub fn caused(
        kind: Kind,
        said: Sentence,
        cause: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        let said = crate::say!("{}: {}", said.inside(), cause);
        Self {
            kind,
            said,
            cause: Some(Arc::new(cause)),
        }
    }

    pub fn file(path: &Path, cause: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            kind: Kind::File,
            said: Sentence::Plain(format!("{}: {cause}", path.display())),
            cause: Some(Arc::new(cause)),
        }
    }

    pub fn format(said: impl Into<Sentence>) -> Self {
        Self::new(Kind::Format, said)
    }

    pub fn program(said: impl Into<Sentence>) -> Self {
        Self::new(Kind::Program, said)
    }

    pub fn network(said: impl Into<Sentence>) -> Self {
        Self::new(Kind::Network, said)
    }

    pub fn refused(said: impl Into<Sentence>) -> Self {
        Self::new(Kind::Refused, said)
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn of(self, what: impl Display) -> Self {
        let said = crate::say!("{}: {}", what, self.said.inside());
        Self { said, ..self }
    }

    pub fn within(&self) -> Inside<'_> {
        self.said.inside()
    }

    pub fn sentence(&self) -> &Sentence {
        &self.said
    }

    /// Not `source()`: a reporter walking the chain would say the sentence's cause twice.
    pub fn cause(&self) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
        self.cause.as_deref()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.said.fmt(f)
    }
}

impl std::error::Error for Error {}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, to: S) -> std::result::Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.said, to)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::said;

    #[test]
    fn a_failure_is_english_for_the_diagnostics_and_put_in_words_where_shown() {
        struct French;
        impl crate::base::sentence::Language for French {
            fn word<'a>(&'a self, english: &'a str) -> &'a str {
                match english {
                    "For the error test {}" => "Pour le test {}",
                    "Inside the error test" => "Dedans",
                    other => other,
                }
            }
            fn counting(&self) -> &crate::base::translate::Counting {
                crate::base::sentence::English.counting()
            }
            fn number(&self, n: i64) -> String {
                n.to_string()
            }
        }
        let inner = Error::refused(said!("Inside the error test"));
        let outer = Error::program(said!("For the error test {}", inner.within()));
        assert_eq!(outer.sentence().put(&French), "Pour le test Dedans");
        assert_eq!(
            outer.to_string(),
            "For the error test Inside the error test"
        );
        assert_eq!(
            Error::format("bad {} value".to_string()).to_string(),
            "bad {} value"
        );
    }

    #[test]
    fn a_failure_keeps_its_kind_and_its_cause_whatever_is_said_around_it() {
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let denied = Error::file(Path::new("/a/notes.json"), denied);
        assert_eq!(denied.to_string(), "/a/notes.json: denied");
        let said = denied.of("the notes were not kept");
        assert_eq!(said.kind(), Kind::File);
        assert_eq!(
            said.to_string(),
            "the notes were not kept: /a/notes.json: denied"
        );
        let cause = said
            .cause()
            .and_then(|c| c.downcast_ref::<std::io::Error>());
        assert_eq!(
            cause.map(|c| c.kind()),
            Some(std::io::ErrorKind::PermissionDenied)
        );
        let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let caused = Error::caused(Kind::File, said!("Not kept"), missing);
        assert_eq!(caused.to_string(), "Not kept: gone");
        assert!(caused.cause().is_some());
    }

    #[test]
    fn a_reporter_walking_the_chain_names_the_cause_once() {
        let gone = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let failed = Error::caused(Kind::File, said!("Not kept"), gone);
        let mut chain = vec![failed.to_string()];
        let mut at = std::error::Error::source(&failed);
        while let Some(cause) = at {
            chain.push(cause.to_string());
            at = cause.source();
        }
        assert_eq!(chain.join(": "), "Not kept: gone");
    }
}
