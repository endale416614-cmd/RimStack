//! The game refuses a DDS that is cut short, in a format its engine cannot read,
//! or block-compressed with a side that is not a multiple of four. From 1.7 it
//! reads only DXT1, DXT5 and BC7, and takes every DX10 file for BC7.

use crate::base::sentence::Sentence;
use crate::rimworld::engine::Engine;
use crate::say;

use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bad {
    pub file: PathBuf,
    pub why: Sentence,
}

/// DXGI formats of BC7, the one a DX10 file is read as from 1.7.
const BC7: std::ops::RangeInclusive<u32> = 97..=99;

fn dx10_format(header: &[u8]) -> Result<u32, Sentence> {
    header
        .get(128..132)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| say!("cut short inside its header"))
}

/// `None` for one that is not block-compressed, or not known.
fn block_bytes(header: &[u8], engine: Engine) -> Result<Option<u64>, Sentence> {
    let four_cc = &header[84..88];
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    if word(80) & 0x4 == 0 || (engine == Engine::Waylith && word(84) == 0) {
        return Ok(None);
    }
    let refused = |other: &[u8]| {
        say!(
            "format {} is not one the game reads",
            String::from_utf8_lossy(other)
        )
    };
    if engine == Engine::Waylith {
        return match four_cc {
            b"DXT1" => Ok(Some(8)),
            b"DXT5" => Ok(Some(16)),
            b"DX10" if BC7.contains(&dx10_format(header)?) => Ok(Some(16)),
            b"DX10" => Err(say!("not BC7, which the game reads every DX10 file as")),
            other => Err(refused(other)),
        };
    }
    Ok(Some(match four_cc {
        b"DXT1" | b"ATI1" | b"BC4U" => 8,
        b"DXT3" | b"DXT5" | b"ATI2" | b"BC5U" => 16,
        // DXGI: BC1 70-72 and BC4 79-81 take eight bytes a block;
        // BC2, BC3, BC5, BC6H and BC7 take sixteen.
        b"DX10" => match dx10_format(header)? {
            70..=72 | 79..=81 => 8,
            73..=78 | 82..=84 | 94..=99 => 16,
            _ => return Ok(None),
        },
        other => return Err(refused(other)),
    }))
}

/// `None` for a file that reads as sound.
pub fn problem(start: &[u8], length: u64, engine: Engine) -> Option<Sentence> {
    if start.len() < 128 || &start[..4] != b"DDS " {
        return Some(say!("not a DDS file"));
    }
    let word =
        |at: usize| u32::from_le_bytes([start[at], start[at + 1], start[at + 2], start[at + 3]]);
    if engine == Engine::Waylith && (word(4) != 124 || word(76) != 32) {
        return Some(say!("a DDS header the game does not read"));
    }
    let (high, wide) = (u64::from(word(12)), u64::from(word(16)));
    if high == 0 || wide == 0 {
        return Some(say!("says it has no size"));
    }
    let block = match block_bytes(start, engine) {
        Ok(block) => block?,
        Err(why) => return Some(why),
    };
    if high % 4 != 0 || wide % 4 != 0 {
        return Some(say!(
            "{}×{} is not a multiple of four, as its compression needs",
            wide,
            high
        ));
    }
    let header = if &start[84..88] == b"DX10" { 148 } else { 128 };
    let first = (wide / 4) * (high / 4) * block;
    (length < header + first).then(|| say!("cut short: less than its first picture"))
}

/// `loaded`: the mod's folders the game reads, for the textures its `Assets.zip` hides.
pub fn audit(mod_root: &Path, engine: Engine, loaded: &[PathBuf]) -> Vec<Bad> {
    let mut bad = Vec::new();
    for png in crate::mods::textures::pngs(mod_root) {
        let file = png.with_extension("dds");
        let Ok(mut opened) = std::fs::File::open(&file) else {
            continue;
        };
        let length = opened.metadata().map_or(0, |m| m.len());
        let mut start = [0u8; 148];
        let read = opened.read(&mut start).unwrap_or(0);
        if let Some(why) = problem(&start[..read], length, engine) {
            bad.push(Bad { file, why });
        }
    }
    if engine == Engine::Waylith {
        for file in crate::mods::packed::hiding(loaded) {
            if !bad.iter().any(|b| b.file == file) {
                let why = say!("hides the texture the mod packed in Assets.zip");
                bad.push(Bad { file, why });
            }
        }
    }
    bad
}

/// Renames, never deletes: the game then loads the mod's own texture.
pub fn set_aside(file: &Path) -> std::io::Result<()> {
    let mut name = file.as_os_str().to_owned();
    name.push(".rimstack-bad");
    std::fs::rename(file, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(wide: u32, high: u32, four_cc: &[u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes[..4].copy_from_slice(b"DDS ");
        bytes[4..8].copy_from_slice(&124u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&high.to_le_bytes());
        bytes[16..20].copy_from_slice(&wide.to_le_bytes());
        bytes[76..80].copy_from_slice(&32u32.to_le_bytes());
        bytes[80..84].copy_from_slice(&4u32.to_le_bytes());
        bytes[84..88].copy_from_slice(four_cc);
        bytes
    }

    #[test]
    fn a_sound_file_has_no_problem_and_each_fault_is_named() {
        let good = header(8, 8, b"DXT1");
        assert_eq!(problem(&good, 128 + 32, Engine::Unity), None);
        assert!(
            problem(&good, 128 + 31, Engine::Unity)
                .unwrap()
                .to_string()
                .contains("cut short")
        );
        assert!(
            problem(&header(10, 8, b"DXT5"), 9999, Engine::Unity)
                .unwrap()
                .to_string()
                .contains("multiple of four")
        );
        assert!(
            problem(&header(8, 8, b"PVRT"), 9999, Engine::Unity)
                .unwrap()
                .to_string()
                .contains("PVRT")
        );
        assert!(
            problem(&header(0, 8, b"DXT1"), 9999, Engine::Unity)
                .unwrap()
                .to_string()
                .contains("no size")
        );
        assert_eq!(
            problem(b"PNG....", 7, Engine::Unity)
                .map(|p| p.to_string())
                .as_deref(),
            Some("not a DDS file")
        );
        // Plain pixels are not held to the multiple of four.
        let mut plain = header(10, 6, b"\0\0\0\0");
        plain[80..84].copy_from_slice(&0x40u32.to_le_bytes());
        assert_eq!(problem(&plain, 128, Engine::Unity), None);
    }

    #[test]
    fn from_1_7_only_dxt1_dxt5_and_bc7_are_read() {
        let said = |bytes: &[u8], engine| problem(bytes, 9999, engine).map(|p| p.to_string());
        let dx10 = |format: u32| {
            let mut bytes = header(8, 8, b"DX10");
            bytes.extend(format.to_le_bytes());
            bytes.extend([0u8; 16]);
            bytes
        };
        for sound in [
            header(8, 8, b"DXT1"),
            header(8, 8, b"DXT5"),
            dx10(98),
            dx10(99),
        ] {
            assert_eq!(said(&sound, Engine::Waylith), None);
        }
        let older = header(8, 8, b"DXT3");
        assert_eq!(said(&older, Engine::Unity), None);
        assert!(said(&older, Engine::Waylith).unwrap().contains("DXT3"));
        assert_eq!(said(&dx10(71), Engine::Unity), None);
        assert!(said(&dx10(71), Engine::Waylith).unwrap().contains("BC7"));
        let mut odd = header(8, 8, b"DXT1");
        odd[76..80].copy_from_slice(&24u32.to_le_bytes());
        assert_eq!(said(&odd, Engine::Unity), None);
        assert!(said(&odd, Engine::Waylith).unwrap().contains("header"));
    }

    #[test]
    fn what_rimstack_writes_passes_its_own_audit() {
        let tmp = tempfile::tempdir().unwrap();
        let textures = tmp.path().join("Textures");
        std::fs::create_dir_all(&textures).unwrap();
        let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]));
        image.save(textures.join("a.png")).unwrap();
        let png = std::fs::read(textures.join("a.png")).unwrap();
        std::fs::write(
            textures.join("a.dds"),
            crate::mods::textures::encode(&png).unwrap(),
        )
        .unwrap();
        assert_eq!(audit(tmp.path(), Engine::Unity, &[]), []);
        assert_eq!(audit(tmp.path(), Engine::Waylith, &[]), []);
        std::fs::write(textures.join("a.dds"), b"DDS broken").unwrap();
        let bad = audit(tmp.path(), Engine::Unity, &[]);
        assert_eq!(bad.len(), 1);
        set_aside(&bad[0].file).unwrap();
        assert!(textures.join("a.dds.rimstack-bad").is_file());
        assert_eq!(audit(tmp.path(), Engine::Unity, &[]), []);
    }
}
