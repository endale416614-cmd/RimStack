use std::path::Path;

/// The picture at `path` cut to fill `width` by `height` from its middle, as BGRA.
pub fn crop(path: &Path, (width, height): (u32, u32)) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }
    let picture = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let mut shown = picture
        .resize_to_fill(width, height, image::imageops::FilterType::Triangle)
        .to_rgba8();
    for pixel in shown.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(shown.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_cut_from_its_middle_to_the_size_in_bgra_and_a_broken_one_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let red = tmp.path().join("red.png");
        image::RgbaImage::from_pixel(64, 32, image::Rgba([255, 0, 0, 255]))
            .save(&red)
            .unwrap();
        let pixels = crop(&red, (8, 4)).unwrap();
        assert_eq!(pixels.len(), 8 * 4 * 4);
        assert_eq!(&pixels[..4], &[0, 0, 255, 255]);
        let broken = tmp.path().join("Preview.png");
        std::fs::write(&broken, b"not a picture").unwrap();
        assert_eq!(crop(&broken, (8, 4)), None);
        assert_eq!(crop(&red, (0, 4)), None);
    }
}
