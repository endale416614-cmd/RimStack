#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Paragraph,
    /// 1 for `[h1]`, 2 for `[h2]` and `[h3]`.
    Heading(u8),
    /// `depth` from 1; `number` in an ordered list.
    Item {
        depth: usize,
        number: Option<usize>,
    },
    Quote,
    Code,
    Rule,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    pub runs: Vec<Run>,
}

impl Block {
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

pub(super) enum Tag {
    Bold,
    Italic,
    Underline,
    Strike,
    Heading(u8),
    List { ordered: bool },
    Item,
    Quote,
    Code,
    Rule,
}

#[derive(Default)]
pub(super) struct Reader {
    done: Vec<Block>,
    runs: Vec<Run>,
    kind: Option<Kind>,
    bold: usize,
    italic: usize,
    underline: usize,
    strike: usize,
    link: Option<String>,
    /// Each list open, whether it counts, and how far it got.
    lists: Vec<(bool, usize)>,
    quotes: usize,
}

impl Reader {
    fn kind(&self) -> Kind {
        self.kind.unwrap_or(match self.quotes {
            0 => Kind::Paragraph,
            _ => Kind::Quote,
        })
    }

    pub(super) fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let run = Run {
            text: text.to_string(),
            bold: self.bold > 0,
            italic: self.italic > 0,
            underline: self.underline > 0,
            strike: self.strike > 0,
            link: self.link.clone(),
        };
        match self.runs.last_mut() {
            Some(last) if same_style(last, &run) => last.text += &run.text,
            _ => self.runs.push(run),
        }
    }

    /// The block read so far ended; the next is `next`, or what the lists and quotes around say.
    fn end(&mut self, next: Option<Kind>) {
        let kind = self.kind();
        let mut runs = std::mem::take(&mut self.runs);
        trim_edges(&mut runs);
        if !runs.is_empty() || kind == Kind::Rule {
            self.done.push(Block { kind, runs });
        }
        self.kind = next;
    }

    pub(super) fn link(&mut self, to: Option<String>) {
        self.link = to;
    }

    pub(super) fn open(&mut self, tag: Tag) {
        match tag {
            Tag::Bold => self.bold += 1,
            Tag::Italic => self.italic += 1,
            Tag::Underline => self.underline += 1,
            Tag::Strike => self.strike += 1,
            Tag::Heading(level) => self.end(Some(Kind::Heading(level))),
            Tag::Code => self.end(Some(Kind::Code)),
            Tag::Rule => {
                self.end(Some(Kind::Rule));
                self.end(None);
            }
            Tag::List { ordered } => {
                self.end(None);
                self.lists.push((ordered, 0));
            }
            Tag::Item => {
                let depth = self.lists.len().max(1);
                let number = self.lists.last_mut().and_then(|(ordered, at)| {
                    *at += 1;
                    ordered.then_some(*at)
                });
                self.end(Some(Kind::Item { depth, number }));
            }
            Tag::Quote => {
                self.end(None);
                self.quotes += 1;
            }
        }
    }

    pub(super) fn close(&mut self, tag: Tag) {
        let less = |n: &mut usize| *n = n.saturating_sub(1);
        match tag {
            Tag::Bold => less(&mut self.bold),
            Tag::Italic => less(&mut self.italic),
            Tag::Underline => less(&mut self.underline),
            Tag::Strike => less(&mut self.strike),
            Tag::Heading(_) | Tag::Code => self.end(None),
            Tag::List { .. } => {
                self.end(None);
                self.lists.pop();
            }
            Tag::Quote => {
                self.end(None);
                less(&mut self.quotes);
            }
            Tag::Item | Tag::Rule => {}
        }
    }

    pub(super) fn finish(mut self) -> Vec<Block> {
        self.end(None);
        self.done
    }
}

fn same_style(a: &Run, b: &Run) -> bool {
    (a.bold, a.italic, a.underline, a.strike, &a.link)
        == (b.bold, b.italic, b.underline, b.strike, &b.link)
}

fn trim_edges(runs: &mut Vec<Run>) {
    while let Some(first) = runs.first_mut() {
        first.text = first.text.trim_start_matches('\n').to_string();
        if !first.text.is_empty() {
            break;
        }
        runs.remove(0);
    }
    while let Some(last) = runs.last_mut() {
        last.text = last.text.trim_end().to_string();
        if !last.text.is_empty() {
            break;
        }
        runs.pop();
    }
    if runs.iter().all(|run| run.text.trim().is_empty()) {
        runs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::blocks;
    use super::*;

    fn kinds(text: &str) -> Vec<(Kind, String)> {
        blocks(text).iter().map(|b| (b.kind, b.text())).collect()
    }

    #[test]
    fn titles_lists_quotes_and_rules_become_blocks_of_their_own() {
        assert_eq!(
            kinds(
                "[h1]Title[/h1]\nIntro\nmore\n[list]\n[*]One\n[*]Two\n[/list]\n[hr][/hr]\n[quote]Said[/quote]\nEnd"
            ),
            [
                (Kind::Heading(1), "Title".into()),
                (Kind::Paragraph, "Intro\nmore".into()),
                (
                    Kind::Item {
                        depth: 1,
                        number: None
                    },
                    "One".into()
                ),
                (
                    Kind::Item {
                        depth: 1,
                        number: None
                    },
                    "Two".into()
                ),
                (Kind::Rule, String::new()),
                (Kind::Quote, "Said".into()),
                (Kind::Paragraph, "End".into()),
            ]
        );
    }

    #[test]
    fn an_ordered_list_counts_and_a_list_inside_another_goes_deeper() {
        assert_eq!(
            kinds("[olist][*]A[*]B[list][*]C[/list][/olist]"),
            [
                (
                    Kind::Item {
                        depth: 1,
                        number: Some(1)
                    },
                    "A".into()
                ),
                (
                    Kind::Item {
                        depth: 1,
                        number: Some(2)
                    },
                    "B".into()
                ),
                (
                    Kind::Item {
                        depth: 2,
                        number: None
                    },
                    "C".into()
                ),
            ]
        );
    }

    #[test]
    fn runs_keep_their_style_their_link_and_their_characters() {
        let read = blocks("a [b]bold [i]both[/i][/b] [url=https://e.org/?a=1&b=2]x < y[/url]");
        assert_eq!(read.len(), 1);
        let runs = &read[0].runs;
        let styled: Vec<(&str, bool, bool, Option<&str>)> = runs
            .iter()
            .map(|r| (r.text.as_str(), r.bold, r.italic, r.link.as_deref()))
            .collect();
        assert_eq!(
            styled,
            [
                ("a ", false, false, None),
                ("bold ", true, false, None),
                ("both", true, true, None),
                (" ", false, false, None),
                ("x < y", false, false, Some("https://e.org/?a=1&b=2")),
            ]
        );
    }

    #[test]
    fn nothing_but_breaks_and_spaces_makes_no_block() {
        assert!(blocks("\n\n  ").is_empty());
        assert!(blocks("").is_empty());
    }
}
