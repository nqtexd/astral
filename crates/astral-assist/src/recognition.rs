//! Bounded image segmentation, Zhang-Suen thinning and editable stick-rig landmarks.
use anyhow::{Context, Result, ensure};
use astral_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Landmark {
    pub name: String,
    pub position: Point,
    pub observed: bool,
    pub score: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    pub width: u32,
    pub height: u32,
    pub landmarks: Vec<Landmark>,
    pub score: f32,
    pub warnings: Vec<String>,
    pub foreground_pixels: usize,
    pub endpoints: usize,
}
pub fn decode(bytes: &[u8]) -> Result<image::RgbaImage> {
    ensure!(bytes.len() <= 64 * 1024 * 1024, "Image exceeds 64 MiB");
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "Use a PNG or JPEG image"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?;
    ensure!(
        image.width() as u64 * image.height() as u64 <= 16 * 1024 * 1024,
        "Image exceeds 16 megapixels"
    );
    Ok(image.to_rgba8())
}
struct Component {
    pixels: Vec<usize>,
    minx: usize,
    miny: usize,
    maxx: usize,
    maxy: usize,
}
fn neighbors(index: usize, w: usize, h: usize) -> Vec<usize> {
    let x = index % w;
    let y = index / w;
    let mut out = vec![];
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
                out.push(ny as usize * w + nx as usize);
            }
        }
    }
    out
}
fn thin(mask: &mut [u8], w: usize, h: usize) {
    if w < 3 || h < 3 {
        return;
    }
    for _ in 0..256 {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = vec![];
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if mask[i] == 0 {
                        continue;
                    }
                    let p = [
                        mask[i - w],
                        mask[i - w + 1],
                        mask[i + 1],
                        mask[i + w + 1],
                        mask[i + w],
                        mask[i + w - 1],
                        mask[i - 1],
                        mask[i - w - 1],
                    ];
                    let n = p.iter().map(|v| *v as usize).sum::<usize>();
                    if !(2..=6).contains(&n) {
                        continue;
                    }
                    let transitions = (0..8)
                        .filter(|j| p[*j] == 0 && p[(*j + 1) % 8] == 1)
                        .count();
                    if transitions != 1 {
                        continue;
                    }
                    let keep = if pass == 0 {
                        p[0] * p[2] * p[4] == 0 && p[2] * p[4] * p[6] == 0
                    } else {
                        p[0] * p[2] * p[6] == 0 && p[0] * p[4] * p[6] == 0
                    };
                    if keep {
                        remove.push(i);
                    }
                }
            }
            changed |= !remove.is_empty();
            for i in remove {
                mask[i] = 0;
            }
        }
        if !changed {
            break;
        }
    }
}
fn path(mask: &[u8], w: usize, h: usize, start: usize, end: usize) -> Option<Vec<usize>> {
    let mut previous = vec![usize::MAX; mask.len()];
    let mut queue = VecDeque::from([start]);
    previous[start] = start;
    while let Some(i) = queue.pop_front() {
        if i == end {
            let mut out = vec![i];
            let mut p = i;
            while p != start {
                p = previous[p];
                out.push(p);
            }
            out.reverse();
            return Some(out);
        }
        for n in neighbors(i, w, h) {
            if mask[n] != 0 && previous[n] == usize::MAX {
                previous[n] = i;
                queue.push_back(n);
            }
        }
    }
    None
}
pub fn recognize(bytes: &[u8]) -> Result<Detection> {
    let original = decode(bytes)?;
    let (ow, oh) = original.dimensions();
    ensure!(ow >= 16 && oh >= 16, "Image is too small");
    let factor = (512. / ow.max(oh) as f32).min(1.);
    let (w, h) = (
        (ow as f32 * factor).round() as usize,
        (oh as f32 * factor).round() as usize,
    );
    let image = image::imageops::resize(
        &original,
        w as u32,
        h as u32,
        image::imageops::FilterType::Triangle,
    );
    let mut border = vec![];
    for x in (0..w).step_by((w / 64).max(1)) {
        border.push(image.get_pixel(x as u32, 0).0);
        border.push(image.get_pixel(x as u32, (h - 1) as u32).0);
    }
    for y in (0..h).step_by((h / 64).max(1)) {
        border.push(image.get_pixel(0, y as u32).0);
        border.push(image.get_pixel((w - 1) as u32, y as u32).0);
    }
    let transparent = border.iter().filter(|p| p[3] < 64).count() > border.len() / 2;
    let mut background = [255u8; 3];
    for c in 0..3 {
        let mut values: Vec<_> = border.iter().filter(|p| p[3] > 128).map(|p| p[c]).collect();
        values.sort_unstable();
        if !values.is_empty() {
            background[c] = values[values.len() / 2];
        }
    }
    let mut mask: Vec<u8> = image
        .pixels()
        .map(|p| {
            u8::from(
                p[3] > 80
                    && (transparent
                        || (0..3)
                            .map(|c| (p[c] as i32 - background[c] as i32).abs())
                            .sum::<i32>()
                            > 120),
            )
        })
        .collect();
    let mut visited = vec![false; mask.len()];
    let mut components = vec![];
    for i in 0..mask.len() {
        if mask[i] == 0 || visited[i] {
            continue;
        }
        let mut queue = VecDeque::from([i]);
        visited[i] = true;
        let mut component = Component {
            pixels: vec![],
            minx: w,
            miny: h,
            maxx: 0,
            maxy: 0,
        };
        while let Some(index) = queue.pop_front() {
            component.pixels.push(index);
            component.minx = component.minx.min(index % w);
            component.maxx = component.maxx.max(index % w);
            component.miny = component.miny.min(index / w);
            component.maxy = component.maxy.max(index / w);
            for n in neighbors(index, w, h) {
                if mask[n] != 0 && !visited[n] {
                    visited[n] = true;
                    queue.push_back(n);
                }
            }
        }
        components.push(component);
    }
    components.sort_by_key(|c| std::cmp::Reverse(c.pixels.len()));
    let largest = components.first().context("No visible stickman found")?;
    ensure!(largest.pixels.len() >= 20, "Not enough visible line work");
    let size = largest.pixels.len();
    let pad = (largest.maxy - largest.miny).max(20) / 2;
    let (left, right, top, bottom) = (
        largest.minx.saturating_sub(pad),
        (largest.maxx + pad).min(w - 1),
        largest.miny.saturating_sub(pad),
        (largest.maxy + pad).min(h - 1),
    );
    mask.fill(0);
    let mut bounds = (w, h, 0, 0);
    let mut count = 0;
    for c in &components {
        if c.pixels.len() < (size / 40).max(8)
            || c.minx > right
            || c.maxx < left
            || c.miny > bottom
            || c.maxy < top
        {
            continue;
        }
        for i in &c.pixels {
            mask[*i] = 1;
            count += 1;
        }
        bounds.0 = bounds.0.min(c.minx);
        bounds.1 = bounds.1.min(c.miny);
        bounds.2 = bounds.2.max(c.maxx);
        bounds.3 = bounds.3.max(c.maxy);
    }
    ensure!(
        count < mask.len() / 3,
        "The foreground is too dense for a line-stickman rig. Use a plain or transparent background."
    );
    let (minx, miny, maxx, maxy) = bounds;
    let bw = (maxx - minx).max(1) as f32;
    let bh = (maxy - miny).max(1) as f32;
    ensure!(bh > 20., "Stickman is too small");
    let foreground = mask.clone();
    thin(&mut mask, w, h);
    let pixels: Vec<_> = mask
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0)
        .map(|(i, _)| i)
        .collect();
    let point = |i: usize| Point::new((i % w) as f32, (i / w) as f32);
    let degree = |i: usize| neighbors(i, w, h).iter().filter(|n| mask[**n] != 0).count();
    let endpoints: Vec<_> = pixels.iter().copied().filter(|i| degree(*i) == 1).collect();
    ensure!(
        endpoints.len() >= 3 && endpoints.len() <= 24,
        "Could not identify distinct limbs. Use one connected, uncrossed stickman and review the joints."
    );
    let cx = (minx + maxx) as f32 / 2.;
    let mut head = Point::new(cx, miny as f32 + bh * 0.09);
    let nearest = |target: Point| {
        *pixels
            .iter()
            .min_by(|a, b| {
                point(**a)
                    .distance(target)
                    .total_cmp(&point(**b).distance(target))
            })
            .unwrap()
    };
    let junction = |low: f32, high: f32, prior: f32| {
        let target = Point::new(cx, miny as f32 + bh * prior);
        let candidates: Vec<_> = pixels
            .iter()
            .copied()
            .filter(|i| {
                let p = point(*i);
                degree(*i) >= 3
                    && p.y > miny as f32 + bh * low
                    && p.y < miny as f32 + bh * high
                    && (p.x - cx).abs() < bw * 0.28
            })
            .collect();
        let found = candidates
            .iter()
            .min_by(|a, b| {
                point(**a)
                    .distance(target)
                    .total_cmp(&point(**b).distance(target))
            })
            .copied();
        (found.unwrap_or_else(|| nearest(target)), found.is_some())
    };
    let (chest, has_chest) = junction(0.17, 0.43, 0.27);
    let (hips, has_hips) = junction(0.43, 0.77, 0.56);
    let mut widest = (0usize, 0usize, 0usize);
    for y in miny..((miny as f32 + bh * 0.24) as usize).min(h) {
        let xs: Vec<_> = (minx..=maxx)
            .filter(|x| {
                foreground[y * w + *x] != 0 && (*x as f32 - point(chest).x).abs() < bh * 0.18
            })
            .collect();
        if let (Some(left), Some(right)) = (xs.first(), xs.last())
            && right - left > widest.0
        {
            widest = (right - left, y, left + right);
        }
    }
    let brush = count as f32 / pixels.len().max(1) as f32;
    let has_head = widest.0 as f32 > (brush * 1.7).max(bh * 0.05);
    if has_head {
        head = Point::new(widest.2 as f32 / 2., widest.1 as f32);
    }
    let mut tips: Vec<_> = endpoints
        .iter()
        .copied()
        .filter(|i| point(*i).distance(head) > bh * 0.14)
        .collect();
    tips.sort_by(|a, b| point(*b).y.total_cmp(&point(*a).y));
    let mut feet = vec![];
    for i in &tips {
        if point(*i).y > point(hips).y + bh * 0.08 && feet.len() < 2 {
            feet.push(*i);
        }
    }
    feet.sort_by(|a, b| point(*a).x.total_cmp(&point(*b).x));
    let mut hands: Vec<_> = tips.iter().copied().filter(|i| !feet.contains(i)).collect();
    hands.sort_by(|a, b| point(*a).x.total_cmp(&point(*b).x));
    let mut result = vec![
        point(hips),
        point(chest),
        head,
        Point::new(minx as f32 + bw * 0.25, miny as f32 + bh * 0.4),
        Point::new(minx as f32, miny as f32 + bh * 0.55),
        Point::new(maxx as f32 - bw * 0.25, miny as f32 + bh * 0.4),
        Point::new(maxx as f32, miny as f32 + bh * 0.55),
        Point::new(cx - bw * 0.15, miny as f32 + bh * 0.78),
        Point::new(cx - bw * 0.22, maxy as f32),
        Point::new(cx + bw * 0.15, miny as f32 + bh * 0.78),
        Point::new(cx + bw * 0.22, maxy as f32),
    ];
    let mut known = [false; 11];
    known[0] = has_hips;
    known[1] = has_chest;
    known[2] = has_head;
    let mut warnings = vec![];
    if !has_head {
        warnings.push("A distinct head could not be identified; its center is estimated.".into());
    }
    for (joint, tip, start) in [
        (3, hands.first().copied(), chest),
        (
            5,
            if hands.len() > 1 {
                hands.last().copied()
            } else {
                None
            },
            chest,
        ),
        (7, feet.first().copied(), hips),
        (9, feet.get(1).copied(), hips),
    ] {
        if let Some(tip) = tip {
            result[joint + 1] = point(tip);
            known[joint + 1] = true;
            if let Some(route) = path(&mask, w, h, start, tip) {
                let middle = route[route.len() / 2];
                result[joint] = point(middle);
                known[joint] = true;
            } else {
                warnings.push("A limb is disconnected; its middle joint is estimated.".into());
            }
        }
    }
    if !has_hips || !has_chest {
        warnings.push("Torso junctions are uncertain. Correct the shoulder and hip landmarks before importing.".into());
    }
    if endpoints.len() > 6 {
        warnings.push("Extra branches were detected. Crossings, text or background details may affect limb assignment.".into());
    }
    warnings.push("Image mapping estimates one line stickman. Left/right labels refer to image sides; review the overlay.".into());
    let canonical = stick_character("Reference".into(), Point::default(), [0, 0, 0, 255]);
    let landmarks = canonical
        .joints
        .iter()
        .enumerate()
        .map(|(i, j)| Landmark {
            name: j.name.clone(),
            position: Point::new(result[i].x / factor, result[i].y / factor),
            observed: known[i],
            score: if known[i] { 0.85 } else { 0.3 },
        })
        .collect();
    let score = (known.iter().filter(|v| **v).count() as f32 / 11.
        * if endpoints.len() > 6 { 0.7 } else { 1. })
    .clamp(0., 1.);
    Ok(Detection {
        width: ow,
        height: oh,
        landmarks,
        score,
        warnings,
        foreground_pixels: count,
        endpoints: endpoints.len(),
    })
}
pub fn rig(
    d: &Detection,
    name: String,
    center: Point,
    target_height: f32,
    frame: u32,
) -> Result<Character> {
    ensure!(
        d.landmarks.len() == 11
            && d.landmarks.iter().all(|l| l.position.finite())
            && target_height.is_finite()
            && (20.0..=8000.).contains(&target_height),
        "Invalid rig landmarks or target size"
    );
    let min = d
        .landmarks
        .iter()
        .map(|l| l.position.y)
        .fold(f32::INFINITY, f32::min);
    let max = d
        .landmarks
        .iter()
        .map(|l| l.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    ensure!(max - min > 1., "Landmarks have no vertical extent");
    let scale = target_height / (max - min);
    let origin = d.landmarks[0].position;
    let mut c = stick_character(name, center, [35, 38, 44, 255]);
    for (i, j) in c.joints.iter_mut().enumerate() {
        ensure!(
            j.name == d.landmarks[i].name,
            "Landmark names/order do not match canonical controls"
        );
        j.position = Point::new(
            (d.landmarks[i].position.x - origin.x) * scale,
            (d.landmarks[i].position.y - origin.y) * scale,
        );
    }
    c.keys[0] = PoseKey {
        frame,
        pose: Pose {
            root: center,
            joints: c.joints.iter().map(|j| j.position).collect(),
        },
        easing: Easing::Smooth,
    };
    c.thickness = (target_height * 0.035).clamp(1., 50.);
    Ok(c)
}
