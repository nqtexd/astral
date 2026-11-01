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
    pub points: Vec<Point>,
    pub color: Color,
    pub width: f32,
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
