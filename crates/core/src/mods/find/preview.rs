use crate::base::error::Error;
use std::io::{BufRead, BufReader};
use std::path::Path;

const AROUND: usize = 150;
const LINE_CHARS: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Of `lines[0]`, counting from 1.
    pub first_line: usize,
    pub lines: Vec<String>,
    /// Index in `lines` of the line asked for; `None` when the file is shorter.
    pub marked: Option<usize>,
}

pub fn preview(file: &Path, line: usize) -> Result<Preview, Error> {
    let mut reader = BufReader::new(std::fs::File::open(file).map_err(|e| Error::file(file, e))?);
    let first_line = line.saturating_sub(AROUND).max(1);
    let last_line = line.saturating_add(AROUND);
    let mut lines = Vec::new();
    let mut buffer = Vec::new();
    for number in 1..=last_line {
        buffer.clear();
        let read = reader
            .read_until(b'\n', &mut buffer)
            .map_err(|e| Error::file(file, e))?;
        if read == 0 {
            break;
        }
        if number >= first_line {
            let text = String::from_utf8_lossy(&buffer);
            let text = text.trim_end_matches(['\n', '\r']);
            lines.push(text.chars().take(LINE_CHARS).collect());
        }
    }
    let marked = (line >= first_line && line - first_line < lines.len()).then(|| line - first_line);
    Ok(Preview {
        first_line,
        lines,
        marked,
    })
}
