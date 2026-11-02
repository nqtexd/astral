use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DrawingLayer {
    pub id: Id,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
}
impl DrawingLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Id::new_v4(),
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InkStroke {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brush: Option<StampBrush>,
    /// Optional embedded bitmap; four vector points define its editable destination quad.
    #[serde(default)]
    pub bitmap: Option<Id>,
    #[serde(default)]
    pub skin: Option<StrokeSkin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressures: Vec<f32>,
    #[serde(default)]
    pub style: BrushStyle,
    #[serde(default)]
    pub filled: bool,
    #[serde(default)]
    pub curved: bool,
    pub points: Vec<Point>,
    pub color: Color,
    pub width: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrushStyle {
    #[default]
    Solid,
    Marker,
    Dashed,
    Stipple,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StrokeSkin {
    pub character: Id,
    pub joints: Vec<usize>,
    pub rest: Vec<Point>,
}
impl InkStroke {
    pub fn world_node_points(&self, scene: &EvaluatedScene) -> Vec<Point> {
        let mut stroke = self.clone();
        stroke.curved = false;
        stroke.world_points(scene)
    }
    pub fn world_points(&self, scene: &EvaluatedScene) -> Vec<Point> {
        let points = self.sampled_points();
        let Some(skin) = &self.skin else {
            return points;
        };
        let Some(character) = scene.characters.iter().find(|c| c.id == skin.character) else {
            return vec![];
        };
        let pins: Vec<MeshPin> = skin
            .joints
            .iter()
            .zip(&skin.rest)
            .map(|(joint, rest)| MeshPin {
                id: Id::nil(),
                name: String::new(),
                rest: *rest,
                keys: vec![PinKey {
                    frame: 0,
                    offset: character.pose.root + character.pose.joints[*joint] - *rest,
                }],
                driver: None,
            })
            .collect();
        points
            .into_iter()
            .map(|point| crate::deformation::similarity(point, &pins, 0.))
            .collect()
    }
    pub fn sampled_points(&self) -> Vec<Point> {
        if !self.curved || self.points.len() < 3 {
            return self.points.clone();
        }
        let mut out = vec![self.points[0]];
        for i in 0..self.points.len() - 1 {
            let a = self.points[i.saturating_sub(1)];
            let b = self.points[i];
            let c = self.points[i + 1];
            let d = self.points[(i + 2).min(self.points.len() - 1)];
            for step in 1..=8 {
                let t = step as f32 / 8.;
                let t2 = t * t;
                let t3 = t * t2;
                out.push(Point::new(
                    0.5 * ((2. * b.x)
                        + (-a.x + c.x) * t
                        + (2. * a.x - 5. * b.x + 4. * c.x - d.x) * t2
                        + (-a.x + 3. * b.x - 3. * c.x + d.x) * t3),
                    0.5 * ((2. * b.y)
                        + (-a.y + c.y) * t
                        + (2. * a.y - 5. * b.y + 4. * c.y - d.y) * t2
                        + (-a.y + 3. * b.y - 3. * c.y + d.y) * t3),
                ));
            }
        }
        out
    }
    pub fn pressure_at(&self, index: usize) -> f32 {
        if self.pressures.is_empty() {
            return 1.;
        }
        let at = if self.curved && self.points.len() >= 3 {
            index as f32 / 8.
        } else {
            index as f32
        };
        let at = at.clamp(0., (self.pressures.len() - 1) as f32);
        let a = at.floor() as usize;
        let b = (a + 1).min(self.pressures.len() - 1);
        self.pressures[a] + (self.pressures[b] - self.pressures[a]) * (at - a as f32)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DrawingCel {
    pub id: Id,
    pub layer: Id,
    pub frame: u32,
    pub strokes: Vec<InkStroke>,
}
impl Project {
    /// A drawing holds until the next key on its own layer, including explicit blanks.
    pub fn cel_at(&self, layer: Id, frame: u32) -> Option<&DrawingCel> {
        self.cels
            .iter()
            .filter(|c| c.layer == layer && c.frame <= frame)
            .max_by_key(|c| c.frame)
    }
    pub fn cel_end(&self, cel: &DrawingCel) -> u32 {
        self.cels
            .iter()
            .filter(|c| c.layer == cel.layer && c.frame > cel.frame)
            .map(|c| c.frame)
            .min()
            .unwrap_or(self.duration)
            - 1
    }
}
pub(crate) fn validate(p: &Project) -> Result<()> {
    ensure!(
        p.layers.len() <= 128 && p.cels.len() <= 100_000,
        "Too many drawing layers or cels"
    );
    let mut keys = std::collections::BTreeSet::new();
    for layer in &p.layers {
        ensure!(
            !layer.name.trim().is_empty()
                && layer.name.len() <= 512
                && layer.opacity.is_finite()
                && (0.0..=1.).contains(&layer.opacity),
            "Invalid drawing layer"
        );
    }
    for cel in &p.cels {
        ensure!(
            p.layers.iter().any(|l| l.id == cel.layer)
                && cel.frame < p.duration
                && keys.insert((cel.layer, cel.frame)),
            "Cel needs a unique frame on an existing layer"
        );
        ensure!(cel.strokes.len() <= 100_000, "Too many strokes in one cel");
        for stroke in &cel.strokes {
            if let Some(brush) = &stroke.brush {
                brush.validate()?;
                ensure!(
                    p.schema >= 10 && stroke.bitmap.is_none() && !stroke.filled,
                    "Textured brush needs schema 10 and an unfilled vector stroke"
                );
                let length: f32 = stroke
                    .sampled_points()
                    .windows(2)
                    .map(|p| p[0].distance(p[1]))
                    .sum();
                ensure!(
                    length.is_finite()
                        && length / (stroke.width * brush.spacing).max(0.5) <= 50000.,
                    "Textured stroke exceeds 50,000 dabs"
                );
            }
            ensure!(
                !stroke.curved || stroke.points.len() <= 12500,
                "Cubic strokes exceed the 100000-sample budget"
            );
            if matches!(stroke.style, BrushStyle::Dashed | BrushStyle::Stipple) {
                let length: f32 = stroke.points.windows(2).map(|p| p[0].distance(p[1])).sum();
                ensure!(
                    length.is_finite() && length / (stroke.width * 1.5).max(1.) <= 50000.,
                    "Pattern brush exceeds its 50000-mark budget"
                );
            }
            ensure!(
                p.schema >= 9
                    || (stroke.pressures.is_empty()
                        && stroke.bitmap.is_none()
                        && stroke.skin.is_none()
                        && !stroke.filled
                        && !stroke.curved
                        && stroke.style == BrushStyle::Solid),
                "Advanced vector cels require schema 9"
            );
            if let Some(bitmap) = stroke.bitmap {
                ensure!(
                    stroke.points.len() == 4
                        && !stroke.curved
                        && !stroke.filled
                        && stroke.pressures.is_empty()
                        && stroke.style == BrushStyle::Solid
                        && p.bitmaps.iter().any(|b| b.id == bitmap),
                    "Image cel needs an embedded bitmap and four corners"
                );
            }
            if let Some(skin) = &stroke.skin {
                ensure!(
                    !skin.joints.is_empty()
                        && skin.joints.len() <= 64
                        && skin.rest.len() == skin.joints.len()
                        && skin.rest.iter().all(|q| q.finite())
                        && p.character(skin.character)
                            .is_some_and(|c| skin.joints.iter().all(|j| *j < c.joints.len())),
                    "Invalid vector skin binding"
                );
            }
            ensure!(
                (stroke.pressures.is_empty() || stroke.pressures.len() == stroke.points.len())
                    && stroke
                        .pressures
                        .iter()
                        .all(|p| p.is_finite() && (0.0..=1.).contains(p)),
                "Pressure samples must match the vector points and be within 0–1"
            );
            ensure!(
                (2..=100_000).contains(&stroke.points.len())
                    && stroke.points.iter().all(|q| q.finite())
                    && stroke.width.is_finite()
                    && (0.1..=500.).contains(&stroke.width),
                "Invalid cel stroke"
            );
        }
    }
    Ok(())
}
fn editable(p: &Project, id: Id) -> Result<()> {
    let layer = p
        .layers
        .iter()
        .find(|l| l.id == id)
        .ok_or_else(|| anyhow::anyhow!("Drawing layer not found"))?;
    ensure!(!layer.locked, "Drawing layer is locked");
    Ok(())
}
pub(crate) fn execute(p: &mut Project, command: Command, created: &mut Vec<Id>) -> Result<()> {
    match command {
        Command::SetFrameDrawing {
            layer,
            frame,
            strokes,
        } => {
            editable(p, layer)?;
            ensure!(frame < p.duration, "Drawing frame outside project");
            let next = frame + 1;
            // Snapshot before writing: editing an existing hold must restore its
            // old content afterward, while a new drawing must end with a blank.
            let boundary = (next < p.duration
                && !p.cels.iter().any(|c| c.layer == layer && c.frame == next))
            .then(|| {
                p.cel_at(layer, next)
                    .map(|c| c.strokes.clone())
                    .unwrap_or_default()
            });
            execute(
                p,
                Command::SetCel {
                    layer,
                    frame,
                    strokes,
                },
                created,
            )?;
            if let Some(strokes) = boundary {
                execute(
                    p,
                    Command::SetCel {
                        layer,
                        frame: next,
                        strokes,
                    },
                    created,
                )?;
            }
        }
        Command::AddLayer { layer } => {
            created.push(layer.id);
            p.layers.push(layer);
        }
        Command::SetLayer {
            layer,
            name,
            visible,
            locked,
            opacity,
        } => {
            let value = p
                .layers
                .iter_mut()
                .find(|l| l.id == layer)
                .ok_or_else(|| anyhow::anyhow!("Drawing layer not found"))?;
            if let Some(v) = name {
                value.name = v;
            }
            if let Some(v) = visible {
                value.visible = v;
            }
            if let Some(v) = locked {
                value.locked = v;
            }
            if let Some(v) = opacity {
                value.opacity = v;
            }
        }
        Command::MoveLayer { layer, index } => {
            editable(p, layer)?;
            ensure!(index < p.layers.len(), "Layer order is outside the stack");
            let source = p.layers.iter().position(|l| l.id == layer).unwrap();
            let value = p.layers.remove(source);
            p.layers.insert(index, value);
        }
        Command::SetCel {
            layer,
            frame,
            strokes,
        } => {
            editable(p, layer)?;
            if let Some(c) = p
                .cels
                .iter_mut()
                .find(|c| c.layer == layer && c.frame == frame)
            {
                c.strokes = strokes;
            } else {
                let id = Id::new_v4();
                created.push(id);
                p.cels.push(DrawingCel {
                    id,
                    layer,
                    frame,
                    strokes,
                });
            }
        }
        Command::DeleteCel { layer, frame } => {
            editable(p, layer)?;
            let len = p.cels.len();
            p.cels.retain(|c| !(c.layer == layer && c.frame == frame));
            ensure!(p.cels.len() != len, "No drawing key at this frame");
        }
        Command::MoveCel { layer, from, to } => {
            editable(p, layer)?;
            ensure!(to < p.duration, "Drawing key is outside the project");
            ensure!(
                from == to || !p.cels.iter().any(|c| c.layer == layer && c.frame == to),
                "Another drawing key is already at that frame"
            );
            let cel = p
                .cels
                .iter_mut()
                .find(|c| c.layer == layer && c.frame == from)
                .ok_or_else(|| anyhow::anyhow!("Drawing key not found"))?;
            cel.frame = to;
        }
        Command::ShiftCels {
            layer,
            start,
            delta,
        } => {
            editable(p, layer)?;
            for cel in p
                .cels
                .iter_mut()
                .filter(|c| c.layer == layer && c.frame >= start)
            {
                let next = cel.frame as i64 + delta as i64;
                ensure!(
                    (0..p.duration as i64).contains(&next),
                    "Shift moves a drawing outside the project; extend the duration first"
                );
                cel.frame = next as u32;
            }
        }
        Command::SetExposure {
            layer,
            frame,
            frames,
        } => {
            editable(p, layer)?;
            ensure!(
                frames > 0 && frames <= 1_000_000,
                "Hold must be 1–1000000 frames"
            );
            ensure!(
                p.cels.iter().any(|c| c.layer == layer && c.frame == frame),
                "Select a drawing key before changing its exposure"
            );
            let end = frame
                .checked_add(frames)
                .ok_or_else(|| anyhow::anyhow!("Exposure overflow"))?;
            let next = p
                .cels
                .iter()
                .filter(|c| c.layer == layer && c.frame > frame)
                .map(|c| c.frame)
                .min();
            if let Some(next) = next {
                let delta = end as i64 - next as i64;
                let mut duration = p.duration;
                for c in p
                    .cels
                    .iter_mut()
                    .filter(|c| c.layer == layer && c.frame >= next)
                {
                    let value = c.frame as i64 + delta;
                    ensure!(
                        (0..1_000_000).contains(&value),
                        "Exposure exceeds timeline limit"
                    );
                    c.frame = value as u32;
                    duration = duration.max(c.frame + 1);
                }
                p.duration = duration;
            } else if end < p.duration {
                let id = Id::new_v4();
                created.push(id);
                p.cels.push(DrawingCel {
                    id,
                    layer,
                    frame: end,
                    strokes: vec![],
                });
            } else {
                p.duration = p.duration.max(end);
            }
        }
        _ => anyhow::bail!("Not a cel command"),
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StampBrush {
    pub spacing: f32,
    pub scatter: f32,
    pub flow: f32,
    pub hardness: f32,
    pub grain: f32,
    pub seed: u32,
}
impl Default for StampBrush {
    fn default() -> Self {
        Self {
            spacing: 0.2,
            scatter: 0.,
            flow: 0.7,
            hardness: 0.8,
            grain: 0.4,
            seed: 17,
        }
    }
}
impl StampBrush {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0.05..=4.).contains(&self.spacing)
                && (0.0..=2.).contains(&self.scatter)
                && (0.01..=1.).contains(&self.flow)
                && (0.0..=1.).contains(&self.hardness)
                && (0.0..=1.).contains(&self.grain),
            "Invalid textured brush settings"
        );
        Ok(())
    }
}
