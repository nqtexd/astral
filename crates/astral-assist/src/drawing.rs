//! Learned cel interpolation and reversible geometric stroke suggestions.
use anyhow::{Context, Result, ensure};
use astral_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct StrokeSuggestion {
    pub stroke: InkStroke,
    pub ghost: InkStroke,
    pub label: String,
    pub score: f32,
}
pub fn suggest_stroke(stroke: &InkStroke, strength: f32) -> Option<StrokeSuggestion> {
    if stroke.bitmap.is_some()
        || stroke.skin.is_some()
        || stroke
            .pressures
            .windows(2)
            .any(|p| (p[0] - p[1]).abs() > 0.001)
        || stroke.filled
        || stroke.curved
        || stroke.points.len() < 2
        || !(0.1..=1.).contains(&strength)
    {
        return None;
    }
    let length: f32 = stroke.points.windows(2).map(|p| p[0].distance(p[1])).sum();
    if length < 20. {
        return None;
    }
    let points = &stroke.points;
    let first = points[0];
    let last = *points.last()?;
    if points.len() >= 8 {
        let mean = Point::new(
            points.iter().map(|p| p.x).sum::<f32>() / points.len() as f32,
            points.iter().map(|p| p.y).sum::<f32>() / points.len() as f32,
        );
        let mut xx = 0.;
        let mut xy = 0.;
        let mut yy = 0.;
        let mut xr = 0.;
        let mut yr = 0.;
        for p in points {
            let x = p.x - mean.x;
            let y = p.y - mean.y;
            let r = x * x + y * y;
            xx += x * x;
            xy += x * y;
            yy += y * y;
            xr += x * r;
            yr += y * r;
        }
        let determinant = xx * yy - xy * xy;
        if determinant.abs() > 1e-3 {
            let center = Point::new(
                mean.x + (xr * yy - yr * xy) / (2. * determinant),
                mean.y + (yr * xx - xr * xy) / (2. * determinant),
            );
            let radius =
                points.iter().map(|p| p.distance(center)).sum::<f32>() / points.len() as f32;
            let error = points
                .iter()
                .map(|p| (p.distance(center) - radius).powi(2))
                .sum::<f32>()
                / points.len() as f32;
            let mut turn = 0.;
            for pair in points.windows(2) {
                let a = (pair[0].y - center.y).atan2(pair[0].x - center.x);
                let b = (pair[1].y - center.y).atan2(pair[1].x - center.x);
                let delta = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                turn += delta;
            }
            if (0.7..5.8).contains(&turn.abs())
                && radius > 5.
                && radius < length * 3.
                && error.sqrt() / radius < 0.035
            {
                let start = (last.y - center.y).atan2(last.x - center.x);
                let remaining = std::f32::consts::TAU - turn.abs();
                let count = ((remaining * radius / 3.).ceil() as usize).clamp(8, 128);
                let mut tail = vec![last];
                for i in 1..=count {
                    let angle = start + turn.signum() * remaining * i as f32 / count as f32;
                    tail.push(Point::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin(),
                    ));
                }
                *tail.last_mut()? = first;
                let mut completed = stroke.clone();
                completed.points.extend_from_slice(&tail[1..]);
                let mut ghost = stroke.clone();
                ghost.points = tail;
                if let Some(pressure) = stroke.pressures.first() {
                    completed
                        .pressures
                        .resize(completed.points.len(), *pressure);
                    ghost.pressures = vec![*pressure; ghost.points.len()];
                }
                return Some(StrokeSuggestion {
                    stroke: completed,
                    ghost,
                    label: "Close curved stroke".into(),
                    score: (1. - error.sqrt() / radius * 4.).clamp(0., 1.),
                });
            }
        }
    }
    let start = points[points.len().saturating_sub(6)];
    let delta = last - start;
    let d = delta.distance(Point::default());
    if d < 1. {
        return None;
    }
    let distance = (length * strength * 0.5).clamp(8., 200.);
    let end = Point::new(
        last.x + delta.x / d * distance,
        last.y + delta.y / d * distance,
    );
    let straight = first.distance(last) / length;
    if straight < 0.9 {
        return None;
    }
    let mut completed = stroke.clone();
    completed.points.push(end);
    let mut ghost = stroke.clone();
    ghost.points = vec![last, end];
    if let Some(pressure) = stroke.pressures.first() {
        completed
            .pressures
            .resize(completed.points.len(), *pressure);
        ghost.pressures = vec![*pressure; ghost.points.len()];
    }
    Some(StrokeSuggestion {
        stroke: completed,
        ghost,
        label: "Continue line".into(),
        score: straight,
    })
}
pub struct CelLearner {
    models: Vec<DrawingMotion>,
    sources: Vec<u32>,
    styles: Vec<InkStroke>,
}
impl CelLearner {
    pub fn train(p: &Project, layer: Id, frames: &[u32], smoothing: f64) -> Result<Self> {
        ensure!(
            (3..=64).contains(&frames.len()),
            "Choose 3–64 drawing-key examples"
        );
        let mut frames = frames.to_vec();
        frames.sort_unstable();
        ensure!(
            frames.windows(2).all(|w| w[0] != w[1]),
            "Example frames must be unique"
        );
        let cels: Vec<_> = frames
            .iter()
            .map(|frame| {
                p.cels
                    .iter()
                    .find(|c| c.layer == layer && c.frame == *frame)
                    .context("Every example must be a drawing key on this layer")
            })
            .collect::<Result<_>>()?;
        let count = cels[0].strokes.len();
        ensure!(
            (1..=32).contains(&count) && cels.iter().all(|c| c.strokes.len() == count),
            "Examples need 1–32 corresponding strokes in the same order"
        );
        let mut models = vec![];
        for index in 0..count {
            let mut temp = Project::new("Training", p.width, p.height, p.fps, p.duration);
            let mut ids = vec![];
            for cel in &cels {
                let s = &cel.strokes[index];
                let first = &cels[0].strokes[index];
                ensure!(
                    s.skin.is_none() && s.bitmap.is_none(),
                    "Learn unbound vector strokes; rigged artwork and images use their own animation tracks"
                );
                ensure!(
                    s.style == first.style && s.filled == first.filled && s.curved == first.curved,
                    "Corresponding strokes must use the same fill, curve and brush style"
                );
                let id = Id::new_v4();
                ids.push(id);
                temp.drawings.push(Drawing {
                    id,
                    name: "Example".into(),
                    start: cel.frame,
                    end: cel.frame,
                    points: s.points.clone(),
                    width: s.width,
                    color: s.color,
                });
            }
            models.push(train_drawing(
                &temp,
                &ids,
                smoothing,
                "Cel trajectory".into(),
            )?);
        }
        Ok(Self {
            models,
            sources: frames,
            styles: cels[0].strokes.clone(),
        })
    }
    pub fn predict(&self, p: &Project, frame: u32) -> Result<Vec<InkStroke>> {
        ensure!(
            frame >= self.sources[0] && frame <= *self.sources.last().unwrap(),
            "Generate between the first and last example frames"
        );
        Ok(self
            .models
            .iter()
            .enumerate()
            .map(|(index, m)| {
                let mut stroke = self.styles[index].clone();
                let points = sample_drawing(p, m, frame as f64);
                if !stroke.pressures.is_empty() {
                    let original = stroke.pressures.clone();
                    stroke.pressures = (0..points.len())
                        .map(|i| {
                            let t =
                                i as f32 / (points.len() - 1) as f32 * (original.len() - 1) as f32;
                            let a = t.floor() as usize;
                            let b = (a + 1).min(original.len() - 1);
                            original[a] + (original[b] - original[a]) * (t - a as f32)
                        })
                        .collect();
                }
                stroke.points = points;
                stroke.color = m.color;
                stroke.width = m.width;
                stroke
            })
            .collect())
    }
    pub fn generate(
        &self,
        p: &Project,
        layer: Id,
        start: u32,
        end: u32,
        overwrite: bool,
    ) -> Result<Vec<Command>> {
        ensure!(
            start <= end && end < p.duration && end - start <= 600,
            "Generate at most 601 frames inside the project"
        );
        let mut commands = vec![];
        for frame in start..=end {
            if self.sources.contains(&frame) {
                continue;
            }
            let occupied = p.cels.iter().any(|c| c.layer == layer && c.frame == frame);
            if occupied && !overwrite {
                continue;
            }
            commands.push(Command::SetCel {
                layer,
                frame,
                strokes: self.predict(p, frame)?,
            });
        }
        ensure!(
            !commands.is_empty(),
            "No empty frames in this range; enable replacement only if intended"
        );
        Ok(commands)
    }
    pub fn complete(&self, p: &Project, layer: Id, frame: u32) -> Result<Vec<InkStroke>> {
        let mut result = self.predict(p, frame)?;
        if let Some(existing) = p.cels.iter().find(|c| c.layer == layer && c.frame == frame) {
            ensure!(
                existing.strokes.len() <= result.len(),
                "Current drawing has more strokes than the examples"
            );
            for (i, s) in existing.strokes.iter().enumerate() {
                result[i] = s.clone();
            }
        }
        Ok(result)
    }
}
