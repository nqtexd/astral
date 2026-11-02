//! Conservative composition warnings. These are bounds checks, not an artistic verdict.
use crate::*;
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    fn rect(min: Point, size: Point) -> Self {
        Self {
            min,
            max: min + size,
        }
    }
    fn intersects(self, b: Self) -> bool {
        self.min.x < b.max.x && self.max.x > b.min.x && self.min.y < b.max.y && self.max.y > b.min.y
    }
    fn contains(self, b: Self) -> bool {
        b.min.x >= self.min.x
            && b.max.x <= self.max.x
            && b.min.y >= self.min.y
            && b.max.y <= self.max.y
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LayoutIssue {
    pub kind: String,
    pub subject: Id,
    pub subject_kind: String,
    pub obstacle: Option<Id>,
    pub frame: u32,
    pub bounds: Bounds,
    pub message: String,
}
fn bubble(d: &Dialogue, scene: &EvaluatedScene) -> Option<Bounds> {
    let origin = match d.character {
        Some(id) => scene.characters.iter().find(|c| c.id == id)?.pose.root,
        None => Point::default(),
    };
    Some(Bounds::rect(d.position + origin, d.size))
}
fn viewport(p: &Project, s: &EvaluatedScene) -> Bounds {
    let size = Point::new(p.width as f32 / s.zoom, p.height as f32 / s.zoom);
    Bounds::rect(s.center - Point::new(size.x / 2., size.y / 2.), size)
}
fn obstacles(p: &Project, s: &EvaluatedScene, frame: u32) -> Vec<(Id, &'static str, Bounds)> {
    let mut out = vec![];
    for c in &s.characters {
        let def = p.character(c.id).unwrap();
        for (j, point) in c.pose.joints.iter().enumerate() {
            let name = &def.joints[j].name;
            let radius = if name == "head" {
                c.thickness * 2.
            } else {
                c.thickness * 0.65 + 4.
            };
            let q = c.pose.root + *point;
            out.push((
                c.id,
                if name == "head" {
                    "head"
                } else if name.ends_with("foot") {
                    "foot"
                } else {
                    "body"
                },
                Bounds::rect(
                    q - Point::new(radius, radius),
                    Point::new(2. * radius, 2. * radius),
                ),
            ));
            if let Some(parent) = def.joints[j].parent {
                let a = c.pose.root + c.pose.joints[parent];
                let r = c.thickness * 0.5;
                out.push((
                    c.id,
                    "body",
                    Bounds {
                        min: Point::new(q.x.min(a.x) - r, q.y.min(a.y) - r),
                        max: Point::new(q.x.max(a.x) + r, q.y.max(a.y) + r),
                    },
                ));
            }
        }
    }
    for g in p
        .graphics
        .iter()
        .filter(|g| frame >= g.start && frame <= g.end)
    {
        let key = if let Some(binding) = p.bindings.iter().find(|b| b.graphic == g.id) {
            let Some(c) = s.characters.iter().find(|c| c.id == binding.character) else {
                continue;
            };
            bound_graphic_key(
                g,
                binding,
                p.character(c.id).unwrap(),
                &c.pose,
                frame as f64,
            )
        } else {
            g.at(frame as f64)
        };
        if key.opacity < 0.05 || g.color[3] == 0 {
            continue;
        }
        // Large background plates are intentional, not foreground obstructions.
        if g.layer == GraphicLayer::Background
            && g.size.x * g.size.y * key.scale * key.scale > p.width as f32 * p.height as f32 * 0.45
        {
            continue;
        }
        let points: Vec<Point> = if g.contours.is_empty() {
            vec![
                Point::new(-g.size.x / 2., -g.size.y / 2.),
                Point::new(g.size.x / 2., -g.size.y / 2.),
                Point::new(g.size.x / 2., g.size.y / 2.),
                Point::new(-g.size.x / 2., g.size.y / 2.),
            ]
        } else {
            g.contours.iter().flatten().copied().collect()
        };
        let mut bounds = Bounds {
            min: Point::new(f32::INFINITY, f32::INFINITY),
            max: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        };
        for q in points {
            let q = key.position
                + Point::new(
                    (q.x * key.rotation.cos() - q.y * key.rotation.sin()) * key.scale,
                    (q.x * key.rotation.sin() + q.y * key.rotation.cos()) * key.scale,
                );
            bounds.min.x = bounds.min.x.min(q.x);
            bounds.min.y = bounds.min.y.min(q.y);
            bounds.max.x = bounds.max.x.max(q.x);
            bounds.max.y = bounds.max.y.max(q.y);
        }
        out.push((
            g.id,
            if g.text.is_empty() {
                "graphic"
            } else {
                "text_graphic"
            },
            bounds,
        ));
    }
    for raster in &p.rasters {
        if !raster.visible
            || frame < raster.start
            || frame > raster.end
            || raster.at(frame as f64).opacity < 0.05
        {
            continue;
        }
        let resolved = raster.resolve_correctives(s);
        let raster = &resolved;
        let columns = raster.mesh.as_ref().map_or(1, |m| m.columns);
        let rows = raster.mesh.as_ref().map_or(1, |m| m.rows);
        let mut bounds = Bounds {
            min: Point::new(f32::INFINITY, f32::INFINITY),
            max: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        };
        for y in 0..=rows {
            for x in 0..=columns {
                let q = raster.world_point(
                    Point::new(x as f32 / columns as f32, y as f32 / rows as f32),
                    frame as f64,
                );
                bounds.min.x = bounds.min.x.min(q.x);
                bounds.min.y = bounds.min.y.min(q.y);
                bounds.max.x = bounds.max.x.max(q.x);
                bounds.max.y = bounds.max.y.max(q.y);
            }
        }
        if raster.layer == GraphicLayer::Background
            && (bounds.max.x - bounds.min.x) * (bounds.max.y - bounds.min.y)
                > p.width as f32 * p.height as f32 * 0.45
        {
            continue;
        }
        out.push((raster.id, "raster_artwork", bounds));
    }
    out
}
pub fn review(p: &Project, start: u32, end: u32, step: u32) -> Result<Vec<LayoutIssue>> {
    ensure!(
        start <= end && end < p.duration && step > 0,
        "Invalid composition review range"
    );
    ensure!(
        (end - start) / step <= 4096 && p.dialogues.len() <= 256 && p.graphics.len() <= 2048,
        "Composition review budget exceeded; review smaller scenes"
    );
    let mut frames: BTreeSet<u32> = (start..=end).step_by(step as usize).collect();
    frames.insert(end);
    for d in &p.dialogues {
        for f in [d.start, d.end] {
            if f >= start && f <= end {
                frames.insert(f);
            }
        }
    }
    let mut out = vec![];
    let mut seen = BTreeSet::new();
    for frame in frames {
        let scene = evaluate(p, frame as f64);
        let occupied = obstacles(p, &scene, frame);
        for d in p
            .dialogues
            .iter()
            .filter(|d| frame >= d.start && frame <= d.end)
        {
            let Some(bounds) = bubble(d, &scene) else {
                continue;
            };
            if !viewport(p, &scene).contains(bounds)
                && seen.insert((d.id, None, "off_canvas".to_owned()))
            {
                out.push(LayoutIssue {
                    kind: "off_canvas".into(),
                    subject: d.id,
                    subject_kind: "dialogue".into(),
                    obstacle: None,
                    frame,
                    bounds,
                    message: "Dialogue extends outside the camera frame".into(),
                });
            }
            for (id, kind, b) in &occupied {
                if bounds.intersects(*b) && seen.insert((d.id, Some(*id), kind.to_string())) {
                    out.push(LayoutIssue {
                        kind: format!("dialogue_{kind}_collision"),
                        subject: d.id,
                        subject_kind: "dialogue".into(),
                        obstacle: Some(*id),
                        frame,
                        bounds,
                        message: format!("Dialogue covers a {kind}; review the moving composition"),
                    });
                }
            }
            for other in p
                .dialogues
                .iter()
                .filter(|o| o.id > d.id && frame >= o.start && frame <= o.end)
            {
                if bubble(other, &scene).is_some_and(|b| bounds.intersects(b))
                    && seen.insert((d.id, Some(other.id), "dialogue".into()))
                {
                    out.push(LayoutIssue {
                        kind: "dialogue_dialogue_collision".into(),
                        subject: d.id,
                        subject_kind: "dialogue".into(),
                        obstacle: Some(other.id),
                        frame,
                        bounds,
                        message: "Dialogue boxes overlap".into(),
                    });
                }
            }
            if out.len() >= 512 {
                anyhow::bail!("More than 512 layout warnings; review a smaller shot");
            }
        }
        for g in p
            .graphics
            .iter()
            .filter(|g| !g.text.is_empty() && frame >= g.start && frame <= g.end)
        {
            let Some((_, _, bounds)) = occupied.iter().find(|(id, _, _)| *id == g.id) else {
                continue;
            };
            if !viewport(p, &scene).contains(*bounds)
                && seen.insert((g.id, None, "off_canvas".into()))
            {
                out.push(LayoutIssue {
                    kind: "off_canvas".into(),
                    subject: g.id,
                    subject_kind: "graphic_text".into(),
                    obstacle: None,
                    frame,
                    bounds: *bounds,
                    message: "Text graphic extends outside the camera frame".into(),
                });
            }
            for (id, kind, b) in &occupied {
                if *id != g.id
                    && bounds.intersects(*b)
                    && seen.insert((g.id, Some(*id), kind.to_string()))
                {
                    out.push(LayoutIssue {
                        kind: format!("graphic_text_{kind}_collision"),
                        subject: g.id,
                        subject_kind: "graphic_text".into(),
                        obstacle: Some(*id),
                        frame,
                        bounds: *bounds,
                        message: format!(
                            "Text graphic covers a {kind}; review the moving composition"
                        ),
                    });
                }
            }
            if out.len() >= 512 {
                anyhow::bail!("More than 512 layout warnings; review a smaller shot");
            }
        }
    }
    Ok(out)
}
/// Search stable placements over every frame of a short dialogue; never mutate.
pub fn place_dialogue(p: &Project, id: Id) -> Result<(Dialogue, Vec<String>)> {
    let d = p
        .dialogues
        .iter()
        .find(|d| d.id == id)
        .context("Dialogue missing")?;
    ensure!(
        d.end - d.start <= 1200,
        "Place dialogue shot-by-shot (at most 1201 frames)"
    );
    ensure!(
        (p.characters
            .iter()
            .map(|c| c.joints.len() * 2)
            .sum::<usize>()
            + p.graphics.len()
            + p.dialogues.len())
            * (d.end - d.start + 1) as usize
            <= 2_000_000,
        "Placement budget exceeded; shorten this dialogue or simplify the shot"
    );
    let samples: Vec<_> = (d.start..=d.end)
        .map(|f| {
            let scene = evaluate(p, f as f64);
            let mut objects = obstacles(p, &scene, f);
            for other in &p.dialogues {
                if other.id != d.id
                    && f >= other.start
                    && f <= other.end
                    && let Some(b) = bubble(other, &scene)
                {
                    objects.push((other.id, "dialogue", b));
                }
            }
            (scene, objects)
        })
        .collect();
    let first = &samples[0].0;
    let view = viewport(p, first);
    let margin = 24. / first.zoom;
    let origin = d
        .character
        .and_then(|id| first.characters.iter().find(|c| c.id == id))
        .map(|c| c.pose.root)
        .unwrap_or_default();
    let score = |candidate: &Dialogue| -> usize {
        samples
            .iter()
            .map(|(s, objects)| {
                let Some(b) = bubble(candidate, s) else {
                    return 0;
                };
                usize::from(!viewport(p, s).contains(b)) * 100
                    + objects.iter().filter(|(_, _, o)| b.intersects(*o)).count()
            })
            .sum()
    };
    let mut best = d.clone();
    let mut best_score = score(&best);
    for x in [
        view.min.x + margin,
        (view.min.x + view.max.x - d.size.x) / 2.,
        view.max.x - d.size.x - margin,
    ] {
        for y in [
            view.min.y + margin,
            (view.min.y + view.max.y - d.size.y) / 2.,
            view.max.y - d.size.y - margin,
        ] {
            let mut candidate = d.clone();
            candidate.position = Point::new(x, y) - origin;
            let value = score(&candidate);
            if value < best_score {
                best = candidate;
                best_score = value;
            }
        }
    }
    let warnings = if best_score > 0 {
        vec!["No collision-free stable placement was found over the whole dialogue. Shorten the cue, resize it or reframe the shot; inspect the proposal before applying.".into()]
    } else {
        vec![]
    };
    Ok((best, warnings))
}
