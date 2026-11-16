//! The names a zip file lists in its central directory, found from the end
//! record (or the zip64 one it points at). Nothing is decompressed.

use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};

const END: u32 = 0x0605_4b50;
const END64_LOCATOR: u32 = 0x0706_4b50;
const END64: u32 = 0x0606_4b50;
const ENTRY: u32 = 0x0201_4b50;
const END_LEN: u64 = 22;
const LONGEST_COMMENT: u64 = 0xFFFF;

fn damaged(what: &str) -> Error {
    Error::new(ErrorKind::InvalidData, format!("not a zip file: {what}"))
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_le_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32::from_le_bytes(
        bytes.get(at..at + 4)?.try_into().ok()?,
    )))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn read_at(file: &mut (impl Read + Seek), at: u64, len: u64) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(at))?;
    let mut bytes = Vec::new();
    file.take(len).read_to_end(&mut bytes)?;
    match bytes.len() as u64 == len {
        true => Ok(bytes),
        false => Err(damaged("cut short")),
    }
}

/// Where the central directory starts and how long it is.
fn directory(file: &mut (impl Read + Seek), length: u64) -> std::io::Result<(u64, u64)> {
    let tail_len = length.min(END_LEN + LONGEST_COMMENT);
    let tail_at = length - tail_len;
    let tail = read_at(file, tail_at, tail_len)?;
    let end = (0..tail.len().saturating_sub(END_LEN as usize - 1))
        .rev()
        .find(|&at| u32_at(&tail, at) == Some(u64::from(END)))
        .ok_or_else(|| damaged("no end record"))?;
    let size = u32_at(&tail, end + 12).ok_or_else(|| damaged("end record"))?;
    let offset = u32_at(&tail, end + 16).ok_or_else(|| damaged("end record"))?;
    if size != 0xFFFF_FFFF && offset != 0xFFFF_FFFF {
        return Ok((offset, size));
    }
    let locator_at = (tail_at + end as u64)
        .checked_sub(20)
        .ok_or_else(|| damaged("no zip64 locator"))?;
    let locator = read_at(file, locator_at, 20)?;
    if u32_at(&locator, 0) != Some(u64::from(END64_LOCATOR)) {
        return Err(damaged("no zip64 locator"));
    }
    let end64_at = u64_at(&locator, 8).ok_or_else(|| damaged("zip64 locator"))?;
    if end64_at.checked_add(56).is_none_or(|e| e > length) {
        return Err(damaged("zip64 end record"));
    }
    let end64 = read_at(file, end64_at, 56)?;
    if u32_at(&end64, 0) != Some(u64::from(END64)) {
        return Err(damaged("zip64 end record"));
    }
    let size = u64_at(&end64, 40).ok_or_else(|| damaged("zip64 end record"))?;
    let offset = u64_at(&end64, 48).ok_or_else(|| damaged("zip64 end record"))?;
    Ok((offset, size))
}

pub fn names(file: &mut (impl Read + Seek)) -> std::io::Result<Vec<String>> {
    let length = file.seek(SeekFrom::End(0))?;
    let (offset, size) = directory(file, length)?;
    if offset.checked_add(size).is_none_or(|e| e > length) {
        return Err(damaged("central directory past the end"));
    }
    let listed = read_at(file, offset, size)?;
    let mut names = Vec::new();
    let mut at = 0;
    while u32_at(&listed, at) == Some(u64::from(ENTRY)) {
        let field = |from| u16_at(&listed, at + from).map(|n| n as usize);
        let (Some(name), Some(extra), Some(comment)) = (field(28), field(30), field(32)) else {
            break;
        };
        let Some(text) = listed.get(at + 46..at + 46 + name) else {
            break;
        };
        names.push(String::from_utf8_lossy(text).into_owned());
        at += 46 + name + extra + comment;
    }
    Ok(names)
}

pub fn names_in(path: &std::path::Path) -> std::io::Result<Vec<String>> {
    names(&mut std::io::BufReader::new(std::fs::File::open(path)?))
}

#[cfg(test)]
pub(crate) fn zip_of(entries: &[&str], zip64: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for name in entries {
        let local_at = bytes.len() as u32;
        bytes.extend(0x0403_4b50u32.to_le_bytes());
        bytes.extend([0u8; 22]);
        bytes.extend((name.len() as u16).to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(name.as_bytes());
        directory.extend(ENTRY.to_le_bytes());
        directory.extend([0u8; 24]);
        directory.extend((name.len() as u16).to_le_bytes());
        directory.extend([0u8; 12]);
        directory.extend(local_at.to_le_bytes());
        directory.extend(name.as_bytes());
    }
    let offset = bytes.len() as u64;
    let size = directory.len() as u64;
    bytes.extend(&directory);
    let count = entries.len() as u16;
    if zip64 {
        let end64_at = bytes.len() as u64;
        bytes.extend(END64.to_le_bytes());
        bytes.extend(44u64.to_le_bytes());
        bytes.extend([0u8; 4 + 8]);
        bytes.extend(u64::from(count).to_le_bytes());
        bytes.extend(u64::from(count).to_le_bytes());
        bytes.extend(size.to_le_bytes());
        bytes.extend(offset.to_le_bytes());
        bytes.extend(END64_LOCATOR.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(end64_at.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
    }
    bytes.extend(END.to_le_bytes());
    bytes.extend([0u8; 4]);
    bytes.extend(count.to_le_bytes());
    bytes.extend(count.to_le_bytes());
    let (size, offset) = match zip64 {
        true => (0xFFFF_FFFF, 0xFFFF_FFFF),
        false => (size as u32, offset as u32),
    };
    bytes.extend(size.to_le_bytes());
    bytes.extend(offset.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(b"note");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn the_names_are_read_from_a_zip_and_a_zip64() {
        let entries = [
            "Textures/a.png.dds",
            "Textures/b/c.png.dds.split",
            "bake.manifest",
        ];
        for zip64 in [false, true] {
            let read = names(&mut Cursor::new(zip_of(&entries, zip64))).unwrap();
            assert_eq!(read, entries, "zip64: {zip64}");
        }
    }

    #[test]
    fn a_damaged_zip_is_an_error_or_fewer_names() {
        assert!(names(&mut Cursor::new(b"PK not really".to_vec())).is_err());
        assert!(names(&mut Cursor::new(Vec::new())).is_err());
        let whole = zip_of(&["Textures/a.png.dds", "Textures/b.png.dds"], false);
        let mut cut = whole.clone();
        cut.drain(..10);
        assert!(names(&mut Cursor::new(cut)).is_ok_and(|n| n.is_empty()));
        let mut wrong = whole;
        let end = wrong.len() - 26;
        wrong[end + 12..end + 16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(names(&mut Cursor::new(wrong)).is_err());
    }

    #[test]
    fn no_damage_to_a_zip64_panics() {
        let sound = zip_of(&["Textures/a.png.dds", "Textures/b.png.dds"], true);
        let mut seed: u64 = 0x5eed;
        let mut next = move || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            (seed >> 33) as usize
        };
        for _ in 0..20_000 {
            let mut bytes = sound.clone();
            for _ in 0..1 + next() % 4 {
                let at = next() % bytes.len();
                bytes[at] = next() as u8;
            }
            let _ = names(&mut Cursor::new(bytes));
        }
    }
}
