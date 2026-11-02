//! Linear-light SDR finishing. All inputs/outputs are premultiplied sRGB bytes.
use astral_core::*;
type Pixel = [f32; 4];
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
fn decode(src: &[u8]) -> Vec<Pixel> {
    src.chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.;
            if a == 0. {
                [0.; 4]
            } else {
                [
                    srgb_to_linear(p[0] as f32 / 255. / a) * a,
                    srgb_to_linear(p[1] as f32 / 255. / a) * a,
                    srgb_to_linear(p[2] as f32 / 255. / a) * a,
                    a,
                ]
            }
        })
        .collect()
}
fn encode(pixels: &[Pixel]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|p| {
            let a = p[3].clamp(0., 1.);
            let mut out = [0; 4];
            if a > 0. {
                for c in 0..3 {
                    out[c] = (linear_to_srgb((p[c] / a).clamp(0., 1.)) * a * 255.).round() as u8;
                }
                out[3] = (a * 255.).round() as u8;
            }
            out
        })
        .collect()
}
/// Three separable box passes approximate a soft blur in O(pixel count), including large radii.
fn blur<const N: usize>(mut data: Vec<[f32; N]>, w: usize, h: usize, radius: u32) -> Vec<[f32; N]> {
    if radius == 0 {
        return data;
    }
    let r = (radius as usize).div_ceil(3);
    let denom = (2 * r + 1) as f32;
    let mut scratch = vec![[0.; N]; data.len()];
    for _ in 0..3 {
        for y in 0..h {
            let mut sum = [0.; N];
            for x in 0..=r.min(w - 1) {
                for (c, s) in sum.iter_mut().enumerate() {
                    *s += data[y * w + x][c];
                }
            }
            for x in 0..w {
                for (c, s) in sum.iter().enumerate() {
                    scratch[y * w + x][c] = *s / denom;
                }
                if x >= r {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s -= data[y * w + x - r][c];
                    }
                }
                if x + r + 1 < w {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += data[y * w + x + r + 1][c];
                    }
                }
            }
        }
        for x in 0..w {
            let mut sum = [0.; N];
            for y in 0..=r.min(h - 1) {
                for (c, s) in sum.iter_mut().enumerate() {
                    *s += scratch[y * w + x][c];
                }
            }
            for y in 0..h {
                for (c, s) in sum.iter().enumerate() {
                    data[y * w + x][c] = *s / denom;
                }
                if y >= r {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s -= scratch[(y - r) * w + x][c];
                    }
                }
                if y + r + 1 < h {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += scratch[(y + r + 1) * w + x][c];
                    }
                }
            }
        }
    }
    data
}
fn alpha_at(mask: &[[f32; 1]], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let tx = x - ix as f32;
    let ty = y - iy as f32;
    let at = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            0.
        } else {
            mask[y as usize * w + x as usize][0].max(0.)
        }
    };
    at(ix, iy) * (1. - tx) * (1. - ty)
        + at(ix + 1, iy) * tx * (1. - ty)
        + at(ix, iy + 1) * (1. - tx) * ty
        + at(ix + 1, iy + 1) * tx * ty
}
pub fn process(src: &[u8], width: u32, height: u32, stage: &CinematicStage, frame: f64) -> Vec<u8> {
    assert_eq!(src.len(), width as usize * height as usize * 4);
    let mix = stage.mix_at(frame);
    if mix == 0. {
        return src.to_vec();
    }
    let (w, h) = (width as usize, height as usize);
    let base = decode(src);
    let mut out: Vec<Pixel> = match &stage.effect {
        CinematicEffect::Bloom {
            threshold,
            knee,
            radius,
            intensity,
        } => {
            let glow: Vec<Pixel> = base
                .iter()
                .map(|p| {
                    let a = p[3];
                    if a == 0. {
                        return [0.; 4];
                    }
                    let l = (p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722) / a;
                    let soft = (l - threshold + knee).clamp(0., 2. * knee);
                    let soft = soft * soft / (4. * knee + 1e-6);
                    let weight = (l - threshold).max(soft).max(0.) / l.max(1e-6);
                    p.map(|v| v * weight)
                })
                .collect();
            let glow = blur(glow, w, h, *radius);
            base.iter()
                .zip(glow)
                .map(|(a, b)| {
                    [
                        a[0] + b[0] * intensity,
                        a[1] + b[1] * intensity,
                        a[2] + b[2] * intensity,
                        a[3] + (1. - a[3]) * (b[3] * intensity).clamp(0., 1.),
                    ]
                })
                .collect()
        }
        CinematicEffect::DropShadow {
            offset,
            radius,
            color,
            opacity,
        } => {
            let mask = blur(base.iter().map(|p| [p[3]]).collect(), w, h, *radius);
            let rgb = [
                srgb_to_linear(color[0] as f32 / 255.),
                srgb_to_linear(color[1] as f32 / 255.),
                srgb_to_linear(color[2] as f32 / 255.),
            ];
            base.iter()
                .enumerate()
                .map(|(i, p)| {
                    let a = alpha_at(
                        &mask,
                        w,
                        h,
                        (i % w) as f32 - offset.x,
                        (i / w) as f32 - offset.y,
                    ) * opacity
                        * color[3] as f32
                        / 255.;
                    let underneath = a * (1. - p[3]);
                    [
                        p[0] + rgb[0] * underneath,
                        p[1] + rgb[1] * underneath,
                        p[2] + rgb[2] * underneath,
                        p[3] + underneath,
                    ]
                })
                .collect()
        }
        CinematicEffect::RimLight {
            offset,
            radius,
            color,
            intensity,
        } => {
            let mask = blur(base.iter().map(|p| [p[3]]).collect(), w, h, *radius);
            let rgb = [
                srgb_to_linear(color[0] as f32 / 255.),
                srgb_to_linear(color[1] as f32 / 255.),
                srgb_to_linear(color[2] as f32 / 255.),
            ];
            base.iter()
                .enumerate()
                .map(|(i, p)| {
                    let edge = (p[3]
                        - alpha_at(
                            &mask,
                            w,
                            h,
                            (i % w) as f32 - offset.x,
                            (i / w) as f32 - offset.y,
                        ))
                    .max(0.)
                        * intensity
                        * color[3] as f32
                        / 255.;
                    [
                        p[0] + rgb[0] * edge,
                        p[1] + rgb[1] * edge,
                        p[2] + rgb[2] * edge,
                        p[3],
                    ]
                })
                .collect()
        }
        CinematicEffect::Grade {
            exposure,
            contrast,
            saturation,
            lift,
            gamma,
            gain,
        } => base
            .iter()
            .map(|p| {
                let a = p[3];
                if a == 0. {
                    return [0.; 4];
                }
                let mut rgb = [0.; 3];
                for c in 0..3 {
                    rgb[c] = (p[c] / a * 2f32.powf(*exposure) * gain[c] + lift[c])
                        .max(0.)
                        .powf(1. / gamma[c]);
                }
                let l = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                for v in &mut rgb {
                    *v = ((l + (*v - l) * saturation - 0.18) * contrast + 0.18) * a;
                }
                [rgb[0], rgb[1], rgb[2], a]
            })
            .collect(),
    };
    for (p, a) in out.iter_mut().zip(base) {
        for c in 0..4 {
            p[c] = a[c] + (p[c] - a[c]) * mix;
        }
    }
    encode(&out)
}
