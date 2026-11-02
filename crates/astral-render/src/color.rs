//! Explicit SDR color contract: straight RGBA in sRGB; never transform alpha coverage.
use anyhow::{Result, ensure};
use image::{ExtendedColorType, ImageEncoder};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
pub fn srgb_profile() -> Result<Vec<u8>> {
    let mut profile = ColorProfile::new_srgb().encode()?;
    // moxcms writes wall-clock time regardless of the source profile header.
    // A fixed valid creation date keeps frames and independent runs reproducible.
    profile[24..36].copy_from_slice(&[0x07, 0xd0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
    Ok(profile)
}
pub fn to_srgb(image: &mut image::RgbaImage, icc: &[u8]) -> Result<()> {
    ensure!(
        !icc.is_empty() && icc.len() <= 4 * 1024 * 1024,
        "ICC profile must fit 4 MiB"
    );
    let source = ColorProfile::new_from_slice(icc)?;
    ensure!(
        source.color_space == DataColorSpace::Rgb,
        "Only RGB ICC profiles are supported for decoded RGB artwork"
    );
    let destination = ColorProfile::new_srgb();
    let transform = source.create_transform_8bit(
        Layout::Rgba,
        &destination,
        Layout::Rgba,
        TransformOptions::default(),
    )?;
    let mut row = vec![0; image.width() as usize * 4];
    for pixels in image.as_mut().chunks_exact_mut(row.len()) {
        transform.transform(pixels, &mut row)?;
        for (p, c) in pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(row.as_chunks::<4>().0)
        {
            p[..3].copy_from_slice(&c[..3]);
        }
    }
    Ok(())
}
pub fn png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder.set_icc_profile(srgb_profile()?)?;
    encoder.write_image(rgba, width, height, ExtendedColorType::Rgba8)?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageDecoder;
    #[test]
    fn identity_preserves_alpha_and_tagged_png_roundtrips() {
        let mut pixels =
            image::RgbaImage::from_raw(2, 1, vec![32, 64, 128, 47, 220, 140, 21, 255]).unwrap();
        let before = pixels.clone();
        to_srgb(&mut pixels, &srgb_profile().unwrap()).unwrap();
        for (a, b) in pixels.as_raw().iter().zip(before.as_raw()) {
            assert!((*a as i16 - *b as i16).abs() <= 1);
        }
        let bytes = png(pixels.as_raw(), 2, 1).unwrap();
        let mut decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes)).unwrap();
        let profile = decoder.icc_profile().unwrap().unwrap();
        assert_eq!(
            &profile[24..36],
            &[0x07, 0xd0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]
        );
        assert!(to_srgb(&mut pixels, b"invalid").is_err());
    }
    #[test]
    fn p3_conversion_changes_rgb_but_not_coverage() {
        let mut pixels = image::RgbaImage::from_raw(1, 1, vec![180, 100, 40, 77]).unwrap();
        to_srgb(
            &mut pixels,
            &ColorProfile::new_display_p3().encode().unwrap(),
        )
        .unwrap();
        assert_eq!(pixels.as_raw()[3], 77);
        assert_ne!(&pixels.as_raw()[..3], &[180, 100, 40]);
    }
}
