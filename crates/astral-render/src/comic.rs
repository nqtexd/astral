//! CPU image shaders on premultiplied sRGB, uploaded unchanged by every backend.
use astral_core::*;

pub fn process(src: &[u8], w: u32, h: u32, style: &ComicStyle, frame: f64, fps: u32) -> Vec<u8> {
    assert_eq!(src.len(), w as usize * h as usize * 4);
    let amount = style.strength_at(frame);
    if amount == 0. {
        return src.to_vec();
    }
    let mut out = src.to_vec();
    let sample = |x: i32, y: i32| -> [u8; 4] {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            [0; 4]
        } else {
            src[((y as u32 * w + x as u32) * 4) as usize..][..4]
                .try_into()
                .unwrap()
        }
    };
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let p = &mut out[i..i + 4];
            match style {
                ComicStyle::RgbSplit { offset, .. } => {
                    let dx = (offset.x * amount).round() as i32;
                    let dy = (offset.y * amount).round() as i32;
                    let r = sample(x as i32 - dx, y as i32 - dy);
                    let b = sample(x as i32 + dx, y as i32 + dy);
                    // Expanded alpha keeps shifted transparent artwork valid premultiplied RGBA.
                    p[0] = r[0];
                    p[2] = b[2];
                    p[3] = p[3].max(r[3]).max(b[3]);
                }
                ComicStyle::Glitch {
                    displacement,
                    band_height,
                    seed,
                    rate,
                    ..
                } => {
                    let tick = (frame.max(0.) * *rate as f64 / fps.max(1) as f64).floor() as u64;
                    let noise = hash(
                        seed.wrapping_add(tick.wrapping_mul(0x9e3779b97f4a7c15))
                            .wrapping_add((y / band_height) as u64),
                    );
                    let shift = if noise % 4 == 0 {
                        ((noise >> 16) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
                            * displacement
                            * amount
                    } else {
                        0.
                    };
                    p.copy_from_slice(&sample(x as i32 - shift.round() as i32, y as i32));
                }
                ComicStyle::Halftone {
                    spacing,
                    angle,
                    ink,
                    ..
                } => {
                    let alpha = p[3] as f32;
                    if alpha == 0. {
                        continue;
                    }
                    let luma = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
                        / alpha;
                    let sx = x as f32 + 0.5;
                    let sy = y as f32 + 0.5;
                    let u = sx * angle.cos() + sy * angle.sin();
                    let v = -sx * angle.sin() + sy * angle.cos();
                    let du = u.rem_euclid(*spacing) - spacing * 0.5;
                    let dv = v.rem_euclid(*spacing) - spacing * 0.5;
                    let radius =
                        ((1. - luma).clamp(0., 1.) / std::f32::consts::PI).sqrt() * spacing;
                    let coverage = (radius - (du * du + dv * dv).sqrt() + 0.5).clamp(0., 1.)
                        * amount
                        * ink[3] as f32
                        / 255.;
                    for c in 0..3 {
                        p[c] = (p[c] as f32 * (1. - coverage)
                            + ink[c] as f32 / 255. * alpha * coverage)
                            .round() as u8;
                    }
                }
            }
        }
    }
    out
}
fn hash(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
