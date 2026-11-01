//! Verifiable prop contacts, state changes and passages. No inference from dialogue.
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const KEY: &str = "astral.scene_interactions.v1";

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InteractionCue {
    pub name: String,
    pub start: u32,
    pub end: u32,
    #[serde(flatten)]
    pub kind: InteractionKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionKind {
    /// The joint must touch this graphic's transformed local offset throughout the range.
    Touch {
        character: Id,
        joint: String,
        graphic: Id,
        offset: Point,
        tolerance: f32,
    },
    GraphicState {
        graphic: Id,
        min_opacity: f32,
        max_opacity: f32,
    },
    /// Root crosses a doorway only while its blocking graphic is absent/transparent.
    /// Use a separate parked/open leaf graphic if the open door remains visible.
    Passage {
        character: Id,
        barrier: Id,
        crossing_x: f32,
        half_width: f32,
        maximum_opacity: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteractionResult {
    pub name: String,
    pub samples: u32,
    pub passed: bool,
    pub first_failure: Option<u32>,
    pub message: String,
}

pub fn read(p: &Project) -> Result<Vec<InteractionCue>> {
    p.metadata
        .get(KEY)
        .map(|v| serde_json::from_value(v.clone()).map_err(Into::into))
        .unwrap_or_else(|| Ok(vec![]))
}

pub fn validate(p: &Project, cues: &[InteractionCue]) -> Result<()> {
    ensure!(cues.len() <= 256, "At most 256 scene interactions");
    for c in cues {
        ensure!(
            !c.name.is_empty()
                && c.name.len() <= 256
                && c.start <= c.end
                && c.end < p.duration
                && c.end - c.start <= 4096,
            "Invalid interaction range/name"
        );
        let graphic = match &c.kind {
            InteractionKind::Touch {
                character,
                joint,
                graphic,
                offset,
                tolerance,
            } => {
                ensure!(
                    p.character(*character)
                        .is_some_and(|c| c.joints.iter().any(|j| j.name == *joint)),
                    "Missing interaction rig/joint"
                );
                ensure!(
                    offset.finite() && tolerance.is_finite() && (0.1..=100.).contains(tolerance),
                    "Invalid touch offset/tolerance"
                );
                *graphic
            }
            InteractionKind::GraphicState {
                graphic,
                min_opacity,
                max_opacity,
            } => {
                ensure!(
                    min_opacity.is_finite()
                        && max_opacity.is_finite()
                        && (0.0..=1.0).contains(min_opacity)
                        && (*min_opacity..=1.0).contains(max_opacity),
                    "Invalid expected opacity"
                );
                *graphic
            }
            InteractionKind::Passage {
                character,
                barrier,
                crossing_x,
                half_width,
                maximum_opacity,
            } => {
                ensure!(
                    p.character(*character).is_some()
                        && crossing_x.is_finite()
                        && half_width.is_finite()
                        && (1.0..=500.).contains(half_width)
                        && maximum_opacity.is_finite()
                        && (0.0..=1.0).contains(maximum_opacity),
                    "Invalid passage"
                );
                *barrier
            }
        };
        ensure!(
            p.graphics.iter().any(|g| g.id == graphic),
            "Missing interaction graphic"
        );
    }
    Ok(())
}

pub fn review(p: &Project, start: u32, end: u32) -> Result<Vec<InteractionResult>> {
    let cues: Vec<_> = read(p)?
        .into_iter()
        .filter(|c| c.start <= end && c.end >= start)
        .collect();
    validate(p, &cues)?;
    let work: u64 = cues
        .iter()
        .map(|c| (c.end.min(end) - c.start.max(start) + 1) as u64)
        .sum();
    ensure!(work <= 32768, "Review interactions in shorter shot ranges");
    let mut results = vec![];
    for cue in cues {
        let mut result = InteractionResult {
            name: cue.name.clone(),
            samples: 0,
            passed: true,
            first_failure: None,
            message: "Declared interaction passed".into(),
        };
        let mut sides = (false, false);
        for frame in cue.start.max(start)..=cue.end.min(end) {
            let scene = evaluate(p, frame as f64);
            let graphic_key = |id| -> Option<GraphicKey> {
                let g = p.graphics.iter().find(|g| g.id == id)?;
                if frame < g.start || frame > g.end {
                    return None;
                }
                if let Some(b) = p.bindings.iter().find(|b| b.graphic == id) {
                    let c = scene.characters.iter().find(|c| c.id == b.character)?;
                    Some(bound_graphic_key(
                        g,
                        b,
                        p.character(c.id)?,
                        &c.pose,
                        frame as f64,
                    ))
                } else {
                    Some(g.at(frame as f64))
                }
            };
            let opacity = |id| graphic_key(id).map_or(0., |key| key.opacity);
            let good = match &cue.kind {
                InteractionKind::Touch {
                    character,
                    joint,
                    graphic,
                    offset,
                    tolerance,
                } => scene
                    .characters
                    .iter()
                    .find(|c| c.id == *character)
                    .and_then(|c| {
                        crate::staging::world_joint(&c.pose, p.character(*character)?, joint)
                    })
                    .zip(graphic_key(*graphic))
                    .is_some_and(|(hand, key)| {
                        let v = Point::new(offset.x * key.scale, offset.y * key.scale);
                        let goal = key.position
                            + Point::new(
                                v.x * key.rotation.cos() - v.y * key.rotation.sin(),
                                v.x * key.rotation.sin() + v.y * key.rotation.cos(),
                            );
                        key.opacity > 0.01 && hand.distance(goal) <= *tolerance
                    }),
                InteractionKind::GraphicState {
                    graphic,
                    min_opacity,
                    max_opacity,
                } => {
                    let alpha = opacity(*graphic);
                    alpha >= *min_opacity && alpha <= *max_opacity
                }
                InteractionKind::Passage {
                    character,
                    barrier,
                    crossing_x,
                    half_width,
                    maximum_opacity,
                } => scene
                    .characters
                    .iter()
                    .find(|c| c.id == *character)
                    .is_some_and(|c| {
                        sides.0 |= c.pose.root.x < crossing_x - half_width;
                        sides.1 |= c.pose.root.x > crossing_x + half_width;
                        (c.pose.root.x - crossing_x).abs() > *half_width
                            || opacity(*barrier) <= *maximum_opacity
                    }),
            };
            result.samples += 1;
            if !good {
                result.passed = false;
                result.first_failure.get_or_insert(frame);
            }
        }
        if matches!(cue.kind, InteractionKind::Passage { .. })
            && start <= cue.start
            && end >= cue.end
            && !(sides.0 && sides.1)
        {
            result.passed = false;
            result.message = "Character never completes the declared doorway crossing".into();
        } else if !result.passed {
            result.message =
                "Prop contact or visual state does not match the declared interaction".into();
        }
        results.push(result);
    }
    Ok(results)
}
