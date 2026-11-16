use super::search::Hit;
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// From 1; of a gap, the line that follows it.
    pub number: usize,
    pub text: String,
    /// Bytes of `text`, in order; empty on a line shown only as context.
    pub marks: Vec<Range<usize>>,
    pub gap: bool,
}

impl Line {
    pub fn is_match(&self) -> bool {
        !self.marks.is_empty()
    }

    pub fn parts(&self) -> Vec<(&str, bool)> {
        let mut parts = Vec::new();
        let mut at = 0;
        for mark in &self.marks {
            let start = mark.start.max(at);
            let end = mark.end.max(start).min(self.text.len());
            if start > at {
                parts.push((&self.text[at..start], false));
            }
            parts.push((&self.text[start..end], true));
            at = end;
        }
        if at < self.text.len() || parts.is_empty() && !self.gap {
            parts.push((&self.text[at..], false));
        }
        parts
    }
}

pub fn lines_of<'a>(hits: impl IntoIterator<Item = &'a Hit>) -> Vec<Line> {
    let mut by_number: BTreeMap<usize, Line> = BTreeMap::new();
    let context = |number: usize, text: &String| Line {
        number,
        text: text.clone(),
        marks: Vec::new(),
        gap: false,
    };
    for hit in hits {
        let first = hit.line - hit.before.len();
        let around = hit.before.iter().zip(first..).map(|(t, n)| (n, t));
        let after = hit.after.iter().zip(hit.line + 1..).map(|(t, n)| (n, t));
        for (number, text) in around.chain(after) {
            by_number
                .entry(number)
                .or_insert_with(|| context(number, text));
        }
        let line = by_number
            .entry(hit.line)
            .or_insert_with(|| context(hit.line, &hit.text));
        if !line.is_match() {
            line.text.clone_from(&hit.text);
        }
        if line.text == hit.text {
            line.marks.push(hit.marked.clone());
            line.marks.sort_by_key(|m| m.start);
        }
    }
    let mut lines = Vec::with_capacity(by_number.len());
    for (number, line) in by_number {
        if lines
            .last()
            .is_some_and(|last: &Line| last.number + 1 < number)
        {
            lines.push(Line {
                number,
                text: String::new(),
                marks: Vec::new(),
                gap: true,
            });
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> Vec<String> {
        (1..=30).map(|n| format!("line {n} word")).collect()
    }

    fn hit(file: &[String], line: usize, at: usize) -> Hit {
        let text = file[line - 1].clone();
        Hit {
            line,
            before: file[line.saturating_sub(4)..line - 1].to_vec(),
            marked: at..at + 4,
            text,
            after: file[line..(line + 3).min(file.len())].to_vec(),
        }
    }

    fn numbers(lines: &[Line]) -> Vec<usize> {
        lines
            .iter()
            .map(|l| if l.gap { 0 } else { l.number })
            .collect()
    }

    #[test]
    fn overlapping_windows_show_every_line_once() {
        let f = file();
        let hits = [hit(&f, 10, 0), hit(&f, 11, 0), hit(&f, 12, 0)];
        let lines = lines_of(&hits);
        assert_eq!(numbers(&lines), (7..=15).collect::<Vec<_>>());
        let matches: Vec<usize> = lines
            .iter()
            .filter(|l| l.is_match())
            .map(|l| l.number)
            .collect();
        assert_eq!(matches, [10, 11, 12]);
    }

    #[test]
    fn adjacent_windows_join_and_separate_ones_leave_a_gap() {
        let f = file();
        let adjacent = lines_of(&[hit(&f, 5, 0), hit(&f, 12, 0)]);
        assert_eq!(numbers(&adjacent), (2..=15).collect::<Vec<_>>());
        let apart = lines_of(&[hit(&f, 5, 0), hit(&f, 20, 0)]);
        assert_eq!(
            numbers(&apart),
            [2, 3, 4, 5, 6, 7, 8, 0, 17, 18, 19, 20, 21, 22, 23]
        );
        let gap = apart.iter().find(|l| l.gap).unwrap();
        assert_eq!(gap.number, 17);
        assert_eq!(gap.parts(), []);
    }

    #[test]
    fn a_line_with_several_matches_keeps_all_its_marks_in_order() {
        let f = vec!["a word b word".to_string()];
        let mut second = hit(&f, 1, 9);
        let first = hit(&f, 1, 2);
        second.marked = 9..13;
        let lines = lines_of(&[second, first]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marks, [2..6, 9..13]);
        assert_eq!(
            lines[0].parts(),
            [
                ("a ", false),
                ("word", true),
                (" b ", false),
                ("word", true)
            ]
        );
    }

    #[test]
    fn a_context_line_is_one_unmarked_part() {
        let f = file();
        let lines = lines_of(&[hit(&f, 2, 0)]);
        assert_eq!(lines[0].parts(), [("line 1 word", false)]);
    }
}
