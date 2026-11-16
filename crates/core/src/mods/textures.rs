//! The game loads a `.dds` beside a `.png` instead of it.
//! Only the `.dds` goes in a mod's folder, and only the ones in the manifest are deleted.

use crate::base::error::{Error, Kind};
use crate::said;

use image_dds::{ImageFormat, Mipmaps, Quality};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Made {
    pub source_len: u64,
    /// Seconds since 1970.
    pub source_modified: u64,
    pub output_hash: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    made: BTreeMap<PathBuf, Made>,
}

impl Manifest {
    /// A damaged file is an empty manifest, which deletes no DDS.
    pub fn read(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(Value::Object(entries)) = serde_json::from_str(&text) else {
            return Self::default();
        };
        let made = entries
            .iter()
            .filter_map(|(dds, v)| {
                let made = Made {
                    source_len: v.get("len")?.as_u64()?,
                    source_modified: v.get("modified")?.as_u64()?,
                    output_hash: u64::from_str_radix(v.get("output")?.as_str()?, 16).ok()?,
                };
                Some((PathBuf::from(dds), made))
            })
            .collect();
        Self { made }
    }

    pub fn write(&self, path: &Path) -> Result<(), crate::base::error::Error> {
        let entries: serde_json::Map<String, Value> = self
            .made
            .iter()
            .map(|(dds, m)| {
                let entry = json!({ "len": m.source_len, "modified": m.source_modified, "output": format!("{:016x}", m.output_hash) });
                (dds.to_string_lossy().into_owned(), entry)
            })
            .collect();
        crate::base::atomic_file::write(path, Value::Object(entries).to_string())
    }

    pub fn len(&self) -> usize {
        self.made.len()
    }

    pub fn made_in(&self, mod_root: &Path) -> usize {
        self.made
            .keys()
            .filter(|dds| dds.starts_with(mod_root))
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.made.is_empty()
    }

    pub fn lists(&self, dds: &Path) -> bool {
        self.made.contains_key(dds)
    }
}

/// FNV-1a, as the manifest records it.
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub(crate) fn fingerprint(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((meta.len(), modified.as_secs()))
}

fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(wanted))
}

fn pngs_under(dir: &Path, into: &mut Vec<PathBuf>) {
    let files = crate::base::walk::files(dir).into_iter();
    into.extend(files.filter(|file| has_extension(file, "png")));
}

pub(crate) fn is_textures(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("textures"))
}

fn textures_folders(mod_root: &Path, strict: bool) -> std::io::Result<Vec<PathBuf>> {
    let folders = |dir: &Path| -> std::io::Result<Vec<PathBuf>> {
        match std::fs::read_dir(dir) {
            Ok(entries) => Ok(entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()),
            Err(e) if strict => Err(e),
            Err(_) => Ok(Vec::new()),
        }
    };
    let mut found = Vec::new();
    for path in folders(mod_root)? {
        if is_textures(&path) {
            found.push(path);
            continue;
        }
        found.extend(folders(&path)?.into_iter().filter(|p| is_textures(p)));
    }
    Ok(found)
}

/// For display and reading: a folder that cannot be read has no textures.
pub fn pngs(mod_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for folder in textures_folders(mod_root, false).unwrap_or_default() {
        pngs_under(&folder, &mut found);
    }
    found.sort();
    found
}

/// For what writes: fails when any folder under the mod could not be read.
fn pngs_whole(mod_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for folder in textures_folders(mod_root, true)? {
        let files = crate::base::walk::strict_files(&folder)?.into_iter();
        found.extend(files.filter(|file| has_extension(file, "png")));
    }
    found.sort();
    Ok(found)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub textures: usize,
    pub converted: usize,
    pub out_of_date: usize,
    pub shipped: usize,
    pub png_bytes: u64,
    pub dds_bytes: u64,
    pub made_bytes: u64,
}

impl Summary {
    pub fn not_converted(&self) -> usize {
        self.textures - self.converted - self.out_of_date - self.shipped
    }
}

pub fn summarize(mod_root: &Path, manifest: &Manifest) -> Summary {
    let mut summary = Summary::default();
    for png in pngs(mod_root) {
        summary.textures += 1;
        let source = fingerprint(&png);
        summary.png_bytes += source.map_or(0, |(len, _)| len);
        let dds = png.with_extension("dds");
        let Ok(dds_len) = std::fs::metadata(&dds).map(|m| m.len()) else {
            continue;
        };
        summary.dds_bytes += dds_len;
        match manifest.made.get(&dds) {
            None => summary.shipped += 1,
            Some(made) => {
                summary.made_bytes += dds_len;
                if source == Some((made.source_len, made.source_modified)) {
                    summary.converted += 1
                } else {
                    summary.out_of_date += 1
                }
            }
        }
    }
    summary
}

/// Flipped top to bottom, as the game expects, with sides rounded up to a multiple of four
/// and mipmaps. Transparent pictures become BC7 under the DX10 header; opaque ones DXT1 under
/// the older header, as the shipped DDS files do.
pub fn encode(png: &[u8]) -> Result<Vec<u8>, Error> {
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| Error::format(said!("not a readable PNG: {}", e)))?;
    let mut picture = image::imageops::flip_vertical(&picture.to_rgba8());
    let round = |n: u32| n.div_ceil(4).max(1) * 4;
    let (wide, high) = (round(picture.width()), round(picture.height()));
    if (wide, high) != picture.dimensions() {
        let filter = image::imageops::FilterType::Triangle;
        picture = image::imageops::resize(&picture, wide, high, filter);
    }
    let failed = |e: &dyn std::fmt::Display| Error::format(said!("could not be encoded: {}", e));
    let opaque = picture.pixels().all(|p| p.0[3] == 255);
    let format = if opaque {
        ImageFormat::BC1RgbaUnorm
    } else {
        ImageFormat::BC7RgbaUnorm
    };
    let surface = image_dds::SurfaceRgba8::from_image(&picture)
        .encode(format, Quality::Normal, Mipmaps::GeneratedAutomatic)
        .map_err(|e| failed(&e))?;
    if opaque {
        let mut bytes = dxt1_header(wide, high, surface.mipmaps);
        bytes.extend_from_slice(&surface.data);
        return Ok(bytes);
    }
    let mut bytes = Vec::new();
    let dds = surface.to_dds().map_err(|e| failed(&e))?;
    dds.write(&mut bytes).map_err(|e| failed(&e))?;
    Ok(bytes)
}

/// The 128 bytes that start a DXT1 file in the older DDS layout.
fn dxt1_header(wide: u32, high: u32, mipmaps: u32) -> Vec<u8> {
    const CAPS_HEIGHT_WIDTH_FORMAT_MIPS_SIZE: u32 = 0x000A_1007;
    const FOUR_CC: u32 = 0x4;
    const TEXTURE: u32 = 0x1000;
    const COMPLEX_WITH_MIPMAPS: u32 = 0x0040_0008;
    let top_level_bytes = (wide / 4) * (high / 4) * 8;
    let mut words: Vec<u32> = vec![
        124,
        CAPS_HEIGHT_WIDTH_FORMAT_MIPS_SIZE,
        high,
        wide,
        top_level_bytes,
        1,
        mipmaps,
    ];
    words.extend([0; 11]);
    words.extend([32, FOUR_CC, u32::from_le_bytes(*b"DXT1"), 0, 0, 0, 0, 0]);
    let caps = if mipmaps > 1 {
        TEXTURE | COMPLEX_WITH_MIPMAPS
    } else {
        TEXTURE
    };
    words.extend([caps, 0, 0, 0, 0]);
    let mut bytes = b"DDS ".to_vec();
    bytes.extend(words.iter().flat_map(|w| w.to_le_bytes()));
    bytes
}

const PART: &str = "dds.rimstack-part";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Converted {
    pub made: usize,
    pub failed: Vec<(PathBuf, Error)>,
    pub parts: Parts,
}

/// The manifest that lists them must be on disk before `place`.
#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parts(Vec<(PathBuf, PathBuf)>);

impl Parts {
    pub fn place(self) -> Vec<(PathBuf, Error)> {
        let mut failed = Vec::new();
        for (part, dds) in self.0 {
            if let Err(e) = std::fs::rename(&part, &dds) {
                let _ = std::fs::remove_file(&part);
                failed.push((dds.clone(), Error::file(&dds, e)));
            }
        }
        failed
    }

    pub fn discard(self) {
        for (part, _) in self.0 {
            let _ = std::fs::remove_file(part);
        }
    }
}

/// A DDS RimStack did not make, or no longer as it made it, is left alone and forgotten.
/// The new files stay under another name until `Converted::parts` is placed.
/// `step` is called after each texture, from any thread.
pub fn convert(
    mod_root: &Path,
    manifest: &mut Manifest,
    stop: &AtomicBool,
    step: &(dyn Fn() + Sync),
) -> Result<Converted, Error> {
    let found = pngs_whole(mod_root).map_err(|e| {
        let said = said!(
            "{} could not be read in full, so nothing in it was converted",
            mod_root.display()
        );
        Error::caused(Kind::File, said, e)
    })?;
    let mut todo = Vec::new();
    let mut theirs = Vec::new();
    for png in found {
        let _ = std::fs::remove_file(png.with_extension(PART));
        let dds = png.with_extension("dds");
        let convert = match manifest.made.get(&dds) {
            None => !dds.exists(),
            Some(_) if !dds.exists() => true,
            Some(made) if fingerprint(&png) == Some((made.source_len, made.source_modified)) => {
                false
            }
            Some(made) => match std::fs::read(&dds) {
                Ok(bytes) if hash(&bytes) != made.output_hash => {
                    theirs.push(dds);
                    false
                }
                _ => true,
            },
        };
        if convert {
            todo.push(png);
        }
    }
    for dds in theirs {
        manifest.made.remove(&dds);
    }
    let results: Vec<(PathBuf, Result<Made, Error>)> = todo
        .into_par_iter()
        .filter_map(|png| {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            let result = convert_one(&png);
            step();
            Some((png, result))
        })
        .collect();
    let mut outcome = Converted::default();
    for (png, result) in results {
        match result {
            Ok(made) => {
                let dds = png.with_extension("dds");
                outcome
                    .parts
                    .0
                    .push((png.with_extension(PART), dds.clone()));
                manifest.made.insert(dds, made);
                outcome.made += 1;
            }
            Err(reason) => outcome.failed.push((png, reason)),
        }
    }
    Ok(outcome)
}

fn convert_one(png: &Path) -> Result<Made, Error> {
    let (source_len, source_modified) = fingerprint(png)
        .ok_or_else(|| Error::new(Kind::File, said!("{} cannot be read", png.display())))?;
    let bytes = std::fs::read(png).map_err(|e| Error::file(png, e))?;
    let encoded = encode(&bytes)?;
    let partial = png.with_extension(PART);
    std::fs::write(&partial, &encoded).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        Error::file(&partial, e)
    })?;
    Ok(Made {
        source_len,
        source_modified,
        output_hash: hash(&encoded),
    })
}

/// Only those still byte for byte what RimStack wrote are deleted.
pub fn remove(mod_root: &Path, manifest: &mut Manifest) -> usize {
    let ours: Vec<PathBuf> = manifest
        .made
        .keys()
        .filter(|dds| dds.starts_with(mod_root))
        .cloned()
        .collect();
    let mut removed = 0;
    for dds in ours {
        let wrote = manifest.made.get(&dds).map(|made| made.output_hash);
        let forget = match std::fs::read(&dds) {
            Ok(bytes) if Some(hash(&bytes)) == wrote => {
                let deleted = std::fs::remove_file(&dds).is_ok();
                removed += usize::from(deleted);
                deleted
            }
            Ok(_) => true,
            Err(problem) => problem.kind() == std::io::ErrorKind::NotFound,
        };
        if forget {
            manifest.made.remove(&dds);
        }
    }
    removed
}

mod rows;
pub use rows::{Row, State, in_order};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn png(wide: u32, high: u32, alpha: u8) -> Vec<u8> {
        let picture = image::RgbaImage::from_fn(wide, high, |x, y| {
            image::Rgba([(x * 40) as u8, (y * 40) as u8, 90, alpha])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn placed(root: &Path, manifest: &mut Manifest) -> Converted {
        let mut outcome = convert(root, manifest, &AtomicBool::new(false), &|| ()).unwrap();
        let parts = std::mem::take(&mut outcome.parts);
        assert!(parts.place().is_empty());
        outcome
    }

    fn a_mod() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("mod");
        for (file, alpha) in [
            ("Textures/Things/a.png", 255),
            ("1.6/Textures/b.PNG", 100),
            ("Textures/shipped.png", 255),
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, png(6, 5, alpha)).unwrap();
        }
        fs::write(root.join("Textures/shipped.dds"), "the author's own").unwrap();
        fs::create_dir_all(root.join("About")).unwrap();
        fs::write(root.join("About/Preview.png"), png(4, 4, 255)).unwrap();
        fs::create_dir_all(root.join("a/b/Textures")).unwrap();
        fs::write(root.join("a/b/Textures/deep.png"), png(4, 4, 255)).unwrap();
        (dir, root)
    }

    #[test]
    fn textures_are_found_at_the_root_and_one_folder_down_only() {
        let (_dir, root) = a_mod();
        let found = pngs(&root);
        let names: Vec<String> = found
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "1.6/Textures/b.PNG",
                "Textures/Things/a.png",
                "Textures/shipped.png"
            ]
        );
    }

    #[test]
    fn a_dds_has_the_format_for_its_transparency_and_sides_in_fours() {
        let side =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let opaque = encode(&png(6, 5, 255)).unwrap();
        assert_eq!(&opaque[..4], b"DDS ");
        assert_eq!((side(&opaque, 16), side(&opaque, 12)), (8, 8));
        let transparent = encode(&png(8, 4, 100)).unwrap();
        assert_eq!((side(&transparent, 16), side(&transparent, 12)), (8, 4));
        assert_eq!(&opaque[84..88], b"DXT1");
        assert_eq!(
            (&transparent[84..88], side(&transparent, 128)),
            (&b"DX10"[..], 98)
        );
        assert_eq!(opaque.len(), 128 + 7 * 8);
        assert_eq!(side(&opaque, 28), 4);
        let read = image_dds::ddsfile::Dds::read(&opaque[..]).unwrap();
        assert_eq!((read.get_width(), read.get_height()), (8, 8));
        assert!(image_dds::image_from_dds(&read, 0).is_ok());
        assert!(encode(b"not a png").is_err());
    }

    #[test]
    fn an_opaque_picture_survives_dxt1_with_no_holes_and_close_colours() {
        let source = image::RgbaImage::from_fn(32, 32, |x, y| {
            if x < 8 && y < 8 {
                image::Rgba([0, 0, 0, 255])
            } else if x > 20 {
                image::Rgba([250, (y * 8) as u8, 30, 255])
            } else {
                image::Rgba([(x * 8) as u8, (y * 8) as u8, 128, 255])
            }
        });
        let mut png = std::io::Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let dds = encode(&png.into_inner()).unwrap();
        assert_eq!(&dds[84..88], b"DXT1");

        let read = image_dds::ddsfile::Dds::read(&dds[..]).unwrap();
        let decoded = image::imageops::flip_vertical(&image_dds::image_from_dds(&read, 0).unwrap());
        assert_eq!(decoded.dimensions(), (32, 32));
        assert!(
            decoded.pixels().all(|p| p.0[3] == 255),
            "a pixel became transparent"
        );
        let mut worst = 0;
        let mut total: u64 = 0;
        for (before, after) in source.pixels().zip(decoded.pixels()) {
            for channel in 0..3 {
                let off = before.0[channel].abs_diff(after.0[channel]);
                worst = worst.max(off);
                total += u64::from(off);
            }
        }
        let mean = total as f64 / (32.0 * 32.0 * 3.0);
        assert!(mean < 6.0, "mean colour error {mean}");
        assert!(worst < 48, "worst colour error {worst}");
    }

    #[test]
    fn converting_makes_what_is_missing_and_leaves_the_author_s_dds() {
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        let before = summarize(&root, &manifest);
        assert_eq!(
            (before.textures, before.shipped, before.converted),
            (3, 1, 0)
        );
        assert_eq!(before.dds_bytes, "the author's own".len() as u64);

        let steps = std::sync::atomic::AtomicUsize::new(0);
        let mut outcome = convert(&root, &mut manifest, &AtomicBool::new(false), &|| {
            steps.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })
        .unwrap();
        assert!(!root.join("Textures/Things/a.dds").exists());
        assert!(root.join("Textures/Things/a.dds.rimstack-part").exists());
        assert!(std::mem::take(&mut outcome.parts).place().is_empty());
        assert_eq!((outcome.made, outcome.failed.len()), (2, 0));
        assert_eq!(steps.into_inner(), 2);
        assert_eq!(manifest.made_in(&root), 2);
        assert_eq!(manifest.made_in(&root.with_file_name("another mod")), 0);
        assert_eq!(
            fs::read_to_string(root.join("Textures/shipped.dds")).unwrap(),
            "the author's own"
        );
        let after = summarize(&root, &manifest);
        assert_eq!(
            (after.converted, after.shipped, after.not_converted()),
            (2, 1, 0)
        );
        assert!(after.png_bytes > 0 && after.made_bytes > 0);
        assert_eq!(
            after.dds_bytes,
            after.made_bytes + "the author's own".len() as u64
        );
        assert!(!root.join("Textures/Things/a.dds.rimstack-part").exists());

        assert_eq!(placed(&root, &mut manifest).made, 0);
    }

    #[test]
    fn a_changed_png_is_out_of_date_and_converted_again() {
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        fs::write(root.join("Textures/Things/a.png"), png(12, 12, 255)).unwrap();
        assert_eq!(summarize(&root, &manifest).out_of_date, 1);
        assert_eq!(placed(&root, &mut manifest).made, 1);
        assert_eq!(summarize(&root, &manifest).out_of_date, 0);
    }

    #[test]
    fn a_dds_changed_since_rimstack_made_it_is_kept_and_forgotten() {
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        let dds = root.join("Textures/Things/a.dds");
        fs::write(&dds, "edited by hand").unwrap();
        fs::write(root.join("Textures/Things/a.png"), png(12, 12, 255)).unwrap();

        assert_eq!(placed(&root, &mut manifest).made, 0);
        assert_eq!(fs::read_to_string(&dds).unwrap(), "edited by hand");
        assert_eq!(manifest.made_in(&root), 1);
        assert_eq!(summarize(&root, &manifest).shipped, 2);
    }

    #[test]
    fn stray_parts_are_removed_and_a_recorded_dds_that_is_missing_is_made_again() {
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        let stray = root.join("1.6/Textures/b.dds.rimstack-part");
        let other = root.join("Textures/Things/keep.rimstack-part");
        fs::write(&stray, "half").unwrap();
        fs::write(&other, "not ours").unwrap();
        fs::remove_file(root.join("Textures/Things/a.dds")).unwrap();

        let outcome = convert(&root, &mut manifest, &AtomicBool::new(false), &|| ()).unwrap();
        assert_eq!(outcome.made, 1);
        assert!(!stray.exists() && other.exists());
        outcome.parts.discard();
        assert!(!root.join("Textures/Things/a.dds.rimstack-part").exists());
        assert!(!root.join("Textures/Things/a.dds").exists());
    }

    #[test]
    fn a_stop_leaves_the_rest_of_a_mod_undone_and_what_was_made_complete() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("mod/Textures");
        fs::create_dir_all(&root).unwrap();
        let wanted = 400;
        for n in 0..wanted {
            fs::write(root.join(format!("{n}.png")), png(16, 16, 255)).unwrap();
        }
        let mut manifest = Manifest::default();
        let stop = AtomicBool::new(false);
        let mod_root = dir.path().join("mod");
        let outcome = convert(&mod_root, &mut manifest, &stop, &|| {
            stop.store(true, Ordering::Relaxed)
        })
        .unwrap();

        assert!(
            outcome.made >= 1 && outcome.made < wanted,
            "{}",
            outcome.made
        );
        assert!(outcome.failed.is_empty());
        assert_eq!(manifest.len(), outcome.made);
        for dds in manifest.made.keys() {
            assert!(outcome.parts.0.iter().any(|(_, to)| to == dds));
        }
        assert_eq!(outcome.parts.0.len(), outcome.made);
        assert!(outcome.parts.0.iter().all(|(part, _)| part.is_file()));
        assert!(outcome.parts.place().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_mod_with_an_unreadable_folder_is_counted_in_part_and_converts_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, root) = a_mod();
        let closed = root.join("Textures/Closed");
        fs::create_dir_all(&closed).unwrap();
        fs::write(closed.join("hidden.png"), png(4, 4, 255)).unwrap();
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read_dir(&closed).is_ok();
        let mut manifest = Manifest::default();
        let counted = summarize(&root, &manifest).textures;
        let outcome = convert(&root, &mut manifest, &AtomicBool::new(false), &|| ());
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o755)).unwrap();
        if readable {
            return;
        }

        assert_eq!(counted, 3);
        assert!(outcome.is_err());
        assert!(manifest.is_empty());
        assert!(!root.join("Textures/Things/a.dds.rimstack-part").exists());
    }

    #[test]
    fn a_bad_png_is_reported_and_the_rest_still_convert() {
        let (_dir, root) = a_mod();
        fs::write(root.join("Textures/broken.png"), "junk").unwrap();
        let mut manifest = Manifest::default();
        let outcome = placed(&root, &mut manifest);
        assert_eq!(outcome.made, 2);
        assert_eq!(outcome.failed.len(), 1);
        assert!(
            outcome.failed[0]
                .1
                .to_string()
                .contains("not a readable PNG")
        );
        assert!(!root.join("Textures/broken.dds").exists());
    }

    #[test]
    fn removing_deletes_only_unchanged_files_rimstack_made() {
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        fs::write(root.join("1.6/Textures/b.dds"), "replaced by hand").unwrap();

        assert_eq!(remove(&root, &mut manifest), 1);
        assert!(!root.join("Textures/Things/a.dds").exists());
        assert!(root.join("1.6/Textures/b.dds").exists());
        assert!(root.join("Textures/shipped.dds").exists());
        assert!(manifest.is_empty());
        assert!(root.join("Textures/Things/a.png").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_deleted_stays_rimstack_s() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        let folder = root.join("Textures/Things");
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        let removed = remove(&root, &mut manifest);
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!((removed, manifest.len()), (1, 1));
        assert_eq!(remove(&root, &mut manifest), 1);
        assert!(manifest.is_empty() && !folder.join("a.dds").exists());
    }

    #[test]
    fn removing_from_one_mod_leaves_another_mod_s_files() {
        let (_dir, root) = a_mod();
        let (_other_dir, other) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        placed(&other, &mut manifest);
        assert_eq!(remove(&root, &mut manifest), 2);
        assert_eq!(manifest.len(), 2);
        assert!(other.join("Textures/Things/a.dds").exists());
    }

    #[test]
    #[ignore = "needs RIMSTACK_TEXTURE_PAIRS: a folder with .png files that have a .dds beside them"]
    fn our_dds_is_the_same_way_up_as_one_made_for_the_game() {
        let dir = PathBuf::from(std::env::var_os("RIMSTACK_TEXTURE_PAIRS").unwrap());
        let mut found = Vec::new();
        pngs_under(&dir, &mut found);
        let mut checked = 0;
        for png in found
            .iter()
            .filter(|p| p.with_extension("dds").is_file())
            .take(12)
        {
            let decode = |bytes: &[u8]| {
                let dds = image_dds::ddsfile::Dds::read(bytes).ok()?;
                image_dds::image_from_dds(&dds, 0).ok()
            };
            let Some(theirs) = decode(&fs::read(png.with_extension("dds")).unwrap()) else {
                continue;
            };
            let ours = decode(&encode(&fs::read(png).unwrap()).unwrap()).unwrap();
            if ours.dimensions() != theirs.dimensions() {
                continue;
            }
            let distance = |a: &image::RgbaImage, b: &image::RgbaImage| -> u64 {
                a.pixels()
                    .zip(b.pixels())
                    .map(|(p, q)| {
                        u64::from(p.0[3].abs_diff(q.0[3])) + u64::from(p.0[0].abs_diff(q.0[0]))
                    })
                    .sum()
            };
            let same = distance(&ours, &theirs);
            let upside_down = distance(&image::imageops::flip_vertical(&ours), &theirs);
            println!(
                "{}: same way up {same}, upside down {upside_down}",
                png.display()
            );
            assert!(same <= upside_down, "{}", png.display());
            checked += 1;
        }
        assert!(checked > 0, "no pairs found");
    }

    #[test]
    fn the_manifest_reads_back_as_written_and_damage_reads_as_empty() {
        let (dir, root) = a_mod();
        let mut manifest = Manifest::default();
        placed(&root, &mut manifest);
        let path = dir.path().join("data").join("textures.json");
        manifest.write(&path).unwrap();
        assert_eq!(Manifest::read(&path), manifest);
        fs::write(&path, "{broken").unwrap();
        assert!(Manifest::read(&path).is_empty());
        assert!(Manifest::read(&dir.path().join("none.json")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_back_beside_the_textures_does_not_count_them_twice() {
        let (_dir, root) = a_mod();
        std::os::unix::fs::symlink(root.join("Textures"), root.join("Textures/again")).unwrap();
        assert_eq!(pngs(&root).len(), 3);
    }
}
