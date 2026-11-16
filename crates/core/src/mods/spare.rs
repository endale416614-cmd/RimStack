//! From 1.7 the game compresses each PNG itself, so a DDS beside a PNG that
//! shows the same picture only takes room: deleting it loses nothing. The two
//! are decoded and compared; a DDS may be stored either way up. A DDS passed
//! wrongly as the same leaves the game showing the PNG, the mod's own picture.

use crate::mods::textures::{fingerprint, pngs};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Mean difference per channel over the pixels shown, out of 255. Measured on
/// 18,780 Workshop DDS beside their PNG (2026-11-14): median 0.18, 97% at or
/// under 2. Another texture of the same mod and size: median 55, 4% under 2,
/// near twins such as a beard facing east or north.
const MEAN_DIFFERENCE: f64 = 2.0;
const CHANGED: u8 = 48;
/// More changed pixels than this share is another picture, even when the mean is low.
const CHANGED_SHARE: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spare {
    pub file: PathBuf,
    pub bytes: u64,
    png: PathBuf,
    /// Seconds since 1970, to delete it only if unchanged since it was compared.
    modified: u64,
}

/// Over the pixels either shows: an empty background would make any two sprites alike.
fn alike(png: &image::RgbaImage, dds: &image::RgbaImage, flipped: bool) -> bool {
    let high = png.height();
    let (mut total, mut changed, mut seen) = (0u64, 0u64, 0u64);
    for (x, y, p) in png.enumerate_pixels() {
        let q = dds.get_pixel(x, if flipped { high - 1 - y } else { y });
        if p.0[3] == 0 && q.0[3] == 0 {
            continue;
        }
        seen += 1;
        let offs = (0..4).map(|c| p.0[c].abs_diff(q.0[c]));
        total += offs.clone().map(u64::from).sum::<u64>();
        changed += u64::from(offs.max().unwrap_or(0) >= CHANGED);
    }
    let seen = seen.max(1) as f64;
    total as f64 / (seen * 4.0) <= MEAN_DIFFERENCE && changed as f64 / seen <= CHANGED_SHARE
}

/// False for any file that cannot be read, decoded, or is another size. Stored
/// upside down first, as the game's DDS files are.
pub fn shows_same(png: &[u8], dds: &[u8]) -> bool {
    let Ok(png) = image::load_from_memory_with_format(png, image::ImageFormat::Png) else {
        return false;
    };
    let png = png.to_rgba8();
    let Ok(read) = image_dds::ddsfile::Dds::read(dds) else {
        return false;
    };
    let Ok(dds) = image_dds::image_from_dds(&read, 0) else {
        return false;
    };
    png.dimensions() == dds.dimensions()
        && png.width() > 0
        && png.height() > 0
        && (alike(&png, &dds, true) || alike(&png, &dds, false))
}

fn spare(png: PathBuf, kept: &(dyn Fn(&Path) -> bool + Sync)) -> Option<Spare> {
    let file = png.with_extension("dds");
    if kept(&file) {
        return None;
    }
    let (bytes, modified) = fingerprint(&file)?;
    let (picture, dds) = (std::fs::read(&png).ok()?, std::fs::read(&file).ok()?);
    shows_same(&picture, &dds).then_some(Spare {
        file,
        bytes,
        png,
        modified,
    })
}

/// The DDS files beside a PNG in the mod's textures folders that show the same
/// picture, leaving out `kept` (those RimStack made, which it removes itself).
pub fn find(
    mod_root: &Path,
    kept: &(dyn Fn(&Path) -> bool + Sync),
    stop: &AtomicBool,
) -> Vec<Spare> {
    pngs(mod_root)
        .into_par_iter()
        .filter(|_| !stop.load(Ordering::Relaxed))
        .filter_map(|png| spare(png, kept))
        .collect()
}

/// Deletes those still as they were compared, with their PNG still there, and
/// returns the bytes freed; the rest are left.
pub fn delete(spare: &[Spare]) -> (u64, Vec<PathBuf>) {
    let deleted: Vec<Result<u64, PathBuf>> = spare
        .par_iter()
        .map(|one| {
            let unchanged = fingerprint(&one.file) == Some((one.bytes, one.modified));
            match unchanged && one.png.is_file() && std::fs::remove_file(&one.file).is_ok() {
                true => Ok(one.bytes),
                false => Err(one.file.clone()),
            }
        })
        .collect();
    let freed = deleted.iter().flatten().sum();
    let left = deleted.into_iter().filter_map(Result::err).collect();
    (freed, left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn picture(wide: u32, high: u32, alpha: u8, invert: u8) -> image::RgbaImage {
        image::RgbaImage::from_fn(wide, high, |x, y| {
            let (r, g) = ((x * 4) as u8, (y * 4) as u8);
            image::Rgba([r ^ invert, g ^ invert, 90 ^ invert, alpha])
        })
    }

    fn png(picture: &image::RgbaImage) -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn a_dds_of_the_picture_is_the_same_either_way_up_and_another_picture_is_not() {
        for alpha in [255, 120] {
            let source = png(&picture(32, 16, alpha, 0));
            let made = crate::mods::textures::encode(&source).unwrap();
            assert!(shows_same(&source, &made), "alpha {alpha}");
            let upright = image::imageops::flip_vertical(
                &image_dds::image_from_dds(&image_dds::ddsfile::Dds::read(&made[..]).unwrap(), 0)
                    .unwrap(),
            );
            let surface = image_dds::SurfaceRgba8::from_image(&upright)
                .encode(
                    image_dds::ImageFormat::BC7RgbaUnorm,
                    image_dds::Quality::Fast,
                    image_dds::Mipmaps::Disabled,
                )
                .unwrap();
            let mut stored = Vec::new();
            surface.to_dds().unwrap().write(&mut stored).unwrap();
            assert!(shows_same(&source, &stored), "upright, alpha {alpha}");
        }
        let other = png(&picture(32, 16, 255, 0xff));
        let made = crate::mods::textures::encode(&png(&picture(32, 16, 255, 0))).unwrap();
        assert!(!shows_same(&other, &made));
        let bigger = crate::mods::textures::encode(&png(&picture(32, 24, 255, 0))).unwrap();
        assert!(!shows_same(&png(&picture(32, 16, 255, 0)), &bigger));
        assert!(!shows_same(b"not a png", &made));
        assert!(!shows_same(&other, b"DDS not really"));
    }

    #[test]
    fn spare_files_are_found_and_deleted_only_while_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let textures = dir.path().join("mod/Textures");
        fs::create_dir_all(&textures).unwrap();
        let same = png(&picture(32, 32, 255, 0));
        for name in ["a", "b", "ours", "changed"] {
            fs::write(textures.join(format!("{name}.png")), &same).unwrap();
            let dds = crate::mods::textures::encode(&same).unwrap();
            fs::write(textures.join(format!("{name}.dds")), dds).unwrap();
        }
        fs::write(textures.join("other.png"), png(&picture(32, 32, 255, 0xff))).unwrap();
        fs::write(
            textures.join("other.dds"),
            crate::mods::textures::encode(&same).unwrap(),
        )
        .unwrap();
        fs::write(textures.join("lone.dds"), "no png beside it").unwrap();

        let ours = textures.join("ours.dds");
        let root = dir.path().join("mod");
        let found = find(&root, &|f| f == ours, &AtomicBool::new(false));
        let names: Vec<_> = found.iter().map(|s| s.file.file_name().unwrap()).collect();
        assert_eq!(names, ["a.dds", "b.dds", "changed.dds"]);

        fs::write(textures.join("changed.dds"), "edited since").unwrap();
        let (freed, left) = delete(&found);
        assert_eq!(left, [textures.join("changed.dds")]);
        assert_eq!(freed, found[0].bytes + found[1].bytes);
        assert!(!textures.join("a.dds").exists() && textures.join("a.png").exists());
        for kept in ["ours.dds", "other.dds", "lone.dds", "changed.dds"] {
            assert!(textures.join(kept).exists(), "{kept}");
        }
    }
}
