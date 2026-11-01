use crate::*;
use anyhow::{Context, Result, ensure};
use astral_ml::MotionModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LearnedMotion {
    pub id: Id,
    pub name: String,
    pub character: Id,
    pub start: u32,
    pub end: u32,
    pub source_frames: Vec<u32>,
    pub model: MotionModel,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DrawingMotion {
    pub id: Id,
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub sources: Vec<Id>,
    pub source_frames: Vec<u32>,
    pub model: MotionModel,
    pub color: Color,
    pub width: f32,
}

/// Polar coordinates keep predicted limb lengths independent of rotation.
pub fn pose_features(c: &Character, pose: &Pose) -> Vec<f64> {
    let mut v = vec![pose.root.x as f64, pose.root.y as f64];
    for (i, j) in c.joints.iter().enumerate() {
        let p = pose.joints[i];
        if let Some(parent) = j.parent {
            let delta = p - pose.joints[parent];
            v.push((delta.y as f64).atan2(delta.x as f64));
            v.push(delta.distance(Point::default()) as f64);
        } else {
            v.push(p.x as f64);
            v.push(p.y as f64);
        }
    }
    v
}
pub fn features_pose(c: &Character, features: &[f64]) -> Pose {
    let mut joints = vec![Point::default(); c.joints.len()];
    for (i, j) in c.joints.iter().enumerate() {
        let a = features[2 + i * 2] as f32;
        let b = features[3 + i * 2] as f32;
        if let Some(parent) = j.parent {
            joints[i] = joints[parent] + Point::new(a.cos() * b.max(0.), a.sin() * b.max(0.));
        } else {
            joints[i] = Point::new(a, b)
        }
    }
    Pose {
        root: Point::new(features[0] as f32, features[1] as f32),
        joints,
    }
}
pub fn train_character(p: &Project, id: Id, smoothing: f64, name: String) -> Result<LearnedMotion> {
    let c = p
        .character(id)
        .context("Select a character to learn its poses")?;
    ensure!(!c.locked, "Unlock the character before training");
    ensure!(
        (3..=64).contains(&c.keys.len()),
        "Create 3–64 key poses at different frames, then learn the inbetweens"
    );
    let mut samples: Vec<_> = c.keys.iter().map(|k| pose_features(c, &k.pose)).collect();
    // Unwrap angle histories so a +179° to -179° change takes the short arc.
    for j in 0..c.joints.len() {
        if c.joints[j].parent.is_some() {
            let d = 2 + j * 2;
            for i in 1..samples.len() {
                let prev = samples[i - 1][d];
                let mut delta = samples[i][d] - prev;
                while delta > std::f64::consts::PI {
                    delta -= std::f64::consts::TAU
                }
                while delta < -std::f64::consts::PI {
                    delta += std::f64::consts::TAU
                }
                samples[i][d] = prev + delta;
            }
        }
    }
    let source_frames: Vec<_> = c.keys.iter().map(|k| k.frame).collect();
    let times: Vec<_> = source_frames.iter().map(|f| *f as f64).collect();
    Ok(LearnedMotion {
        id: Id::new_v4(),
        name,
        character: id,
        start: source_frames[0],
        end: *source_frames.last().unwrap(),
        model: MotionModel::train(&times, &samples, smoothing)?,
        source_frames,
    })
}

pub fn resample_stroke(points: &[Point], count: usize) -> Result<Vec<Point>> {
    ensure!(
        points.len() >= 2 && points.iter().all(|p| p.finite()) && (2..=512).contains(&count),
        "A stroke needs at least two finite points"
    );
    let mut lengths = vec![0.];
    for w in points.windows(2) {
        lengths.push(lengths.last().unwrap() + w[0].distance(w[1]));
    }
    let total = *lengths.last().unwrap();
    ensure!(total > 0.001, "Cannot learn a zero-length stroke");
    let mut out = vec![];
    let mut j = 1;
    for i in 0..count {
        let target = total * i as f32 / (count - 1) as f32;
        while j + 1 < lengths.len() && lengths[j] < target {
            j += 1
        }
        let t = (target - lengths[j - 1]) / (lengths[j] - lengths[j - 1]).max(1e-9);
        out.push(points[j - 1].lerp(points[j], t));
    }
    Ok(out)
}
pub fn train_drawing(
    p: &Project,
    sources: &[Id],
    smoothing: f64,
    name: String,
) -> Result<DrawingMotion> {
    ensure!(
        (3..=64).contains(&sources.len()),
        "Learning a stroke needs 3–64 drawings of the same stroke at different frames"
    );
    let mut drawings = sources
        .iter()
        .map(|id| {
            p.drawings
                .iter()
                .find(|d| d.id == *id)
                .context("Source drawing is missing")
        })
        .collect::<Result<Vec<_>>>()?;
    drawings.sort_by_key(|d| d.start);
    ensure!(
        drawings.windows(2).all(|w| w[0].start < w[1].start),
        "Choose one corresponding stroke per example frame"
    );
    ensure!(
        !p.drawing_motions
            .iter()
            .any(|m| sources.iter().any(|id| m.sources.contains(id))),
        "A source stroke already belongs to a learned sequence"
    );
    let first = resample_stroke(&drawings[0].points, 64)?;
    let closed = first[0].distance(first[63]) < 0.01;
    ensure!(
        drawings
            .iter()
            .all(|d| d.color == drawings[0].color && (d.width - drawings[0].width).abs() < 0.001),
        "Use corresponding strokes with the same color and brush width"
    );
    let mut samples = vec![];
    for d in &drawings {
        let mut points = resample_stroke(&d.points, 64)?;
        ensure!(
            (points[0].distance(points[63]) < 0.01) == closed,
            "Open and closed strokes cannot share a learned sequence"
        );
        if closed {
            points = align_closed(&first, &points);
        }
        let forward = first[0].distance(points[0]) + first[63].distance(points[63]);
        let reversed = first[0].distance(points[63]) + first[63].distance(points[0]);
        if !closed && reversed < forward {
            points.reverse();
        }
        samples.push(
            points
                .iter()
                .flat_map(|p| [p.x as f64, p.y as f64])
                .collect(),
        );
    }
    let source_frames: Vec<_> = drawings.iter().map(|d| d.start).collect();
    let times: Vec<_> = source_frames.iter().map(|f| *f as f64).collect();
    Ok(DrawingMotion {
        id: Id::new_v4(),
        name,
        start: source_frames[0],
        end: *source_frames.last().unwrap(),
        sources: drawings.iter().map(|d| d.id).collect(),
        source_frames,
        model: MotionModel::train(&times, &samples, smoothing)?,
        color: drawings[0].color,
        width: drawings[0].width,
    })
}
pub fn learned_points(m: &DrawingMotion, frame: f64) -> Vec<Point> {
    m.model
        .predict((frame - m.start as f64) / (m.end - m.start) as f64)
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| Point::new(v[0] as f32, v[1] as f32))
        .collect()
}
pub fn sample_learned(p: &Project, c: &Character, frame: f64) -> Option<Pose> {
    let m = p
        .learned_motions
        .iter()
        .find(|m| m.character == c.id && frame >= m.start as f64 && frame <= m.end as f64)?;
    if let Some(k) = c
        .keys
        .iter()
        .find(|k| (frame - k.frame as f64).abs() < 1e-8)
    {
        return Some(k.pose.clone());
    }
    Some(features_pose(
        c,
        &m.model
            .predict((frame - m.start as f64) / (m.end - m.start) as f64),
    ))
}
pub fn sample_drawing(p: &Project, m: &DrawingMotion, frame: f64) -> Vec<Point> {
    if let Some((i, _)) = m
        .source_frames
        .iter()
        .enumerate()
        .find(|(_, f)| (frame - **f as f64).abs() < 1e-8)
        && let Some(d) = p.drawings.iter().find(|d| d.id == m.sources[i])
    {
        return d.points.clone();
    }
    learned_points(m, frame)
}
fn align_closed(reference: &[Point], points: &[Point]) -> Vec<Point> {
    let n = 63;
    let center = |points: &[Point]| {
        Point::new(
            points[..n].iter().map(|p| p.x).sum::<f32>() / n as f32,
            points[..n].iter().map(|p| p.y).sum::<f32>() / n as f32,
        )
    };
    let a = center(reference);
    let b = center(points);
    let mut best = (f32::INFINITY, 0, false);
    for reverse in [false, true] {
        for shift in 0..n {
            let score = (0..n)
                .map(|i| {
                    let j = if reverse {
                        (shift + n - i) % n
                    } else {
                        (shift + i) % n
                    };
                    let delta = (reference[i] - a) - (points[j] - b);
                    delta.x * delta.x + delta.y * delta.y
                })
                .sum::<f32>();
            if score < best.0 {
                best = (score, shift, reverse)
            }
        }
    }
    let mut out = (0..n)
        .map(|i| {
            let j = if best.2 {
                (best.1 + n - i) % n
            } else {
                (best.1 + i) % n
            };
            points[j]
        })
        .collect::<Vec<_>>();
    out.push(out[0]);
    out
}
pub fn validate_learning(p: &Project) -> Result<()> {
    let mut characters = std::collections::BTreeSet::new();
    for m in &p.learned_motions {
        let c = p
            .character(m.character)
            .context("Learned motion character missing")?;
        ensure!(
            characters.insert(m.character)
                && m.start < m.end
                && m.end < p.duration
                && m.source_frames == c.keys.iter().map(|k| k.frame).collect::<Vec<_>>(),
            "Learned model is stale or overlaps another learned model"
        );
        m.model.validate(2 + c.joints.len() * 2)?;
        ensure!(
            m.source_frames.first() == Some(&m.start) && m.source_frames.last() == Some(&m.end),
            "Learned range mismatch"
        );
    }
    let mut sources = std::collections::BTreeSet::new();
    for m in &p.drawing_motions {
        ensure!(
            m.start < m.end
                && m.end < p.duration
                && m.sources.len() == m.source_frames.len()
                && m.width.is_finite()
                && (0.1..=200.).contains(&m.width),
            "Invalid learned drawing range"
        );
        m.model.validate(128)?;
        let frames = m
            .sources
            .iter()
            .map(|id| {
                ensure!(
                    sources.insert(*id),
                    "A drawing cannot belong to two learned sequences"
                );
                let d = p
                    .drawings
                    .iter()
                    .find(|d| d.id == *id)
                    .context("Learned source drawing missing")?;
                Ok(d.start)
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            frames == m.source_frames
                && frames.windows(2).all(|w| w[0] < w[1])
                && frames.first() == Some(&m.start)
                && frames.last() == Some(&m.end),
            "Learned source timing mismatch"
        );
    }
    Ok(())
}
