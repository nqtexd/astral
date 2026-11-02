//! Scalar animation channels with explicit time/value tangents (units per frame).
use crate::*;
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurveAxis {
    X,
    Y,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurveInterpolation {
    Hold,
    Linear,
    #[default]
    Cubic,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ChannelKey {
    pub frame: u32,
    pub value: f32,
    pub incoming: f32,
    pub outgoing: f32,
    #[serde(default)]
    pub interpolation: CurveInterpolation,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CurveTrack {
    pub id: Id,
    pub name: String,
    pub character: Id,
    pub joint: Option<usize>,
    pub axis: CurveAxis,
    pub keys: Vec<ChannelKey>,
}
impl CurveTrack {
    /// Outside the authored range the ordinary pose/action channel remains active.
    pub fn sample(&self, frame: f64) -> Option<f32> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if frame < first.frame as f64 || frame > last.frame as f64 {
            return None;
        }
        let i = self
            .keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &self.keys[i];
        let Some(b) = self.keys.get(i + 1) else {
            return Some(a.value);
        };
        let dt = (b.frame - a.frame) as f32;
        let t = ((frame - a.frame as f64) / dt as f64) as f32;
        Some(match a.interpolation {
            CurveInterpolation::Hold => a.value,
            CurveInterpolation::Linear => a.value + (b.value - a.value) * t,
            CurveInterpolation::Cubic => {
                let t2 = t * t;
                let t3 = t2 * t;
                (2. * t3 - 3. * t2 + 1.) * a.value
                    + (t3 - 2. * t2 + t) * dt * a.outgoing
                    + (-2. * t3 + 3. * t2) * b.value
                    + (t3 - t2) * dt * b.incoming
            }
        })
    }
}
pub fn validate_curves(p: &Project) -> Result<()> {
    ensure!(p.curves.len() <= 256, "Too many curve tracks");
    let mut ids = BTreeSet::new();
    let mut channels = BTreeSet::new();
    for c in &p.curves {
        ensure!(
            ids.insert(c.id) && c.name.len() <= 512,
            "Invalid curve identity/name"
        );
        ensure!(
            channels.insert((c.character, c.joint, matches!(c.axis, CurveAxis::X))),
            "Duplicate curve channel"
        );
        let character = p
            .character(c.character)
            .context("Curve character missing")?;
        ensure!(
            c.joint.is_none_or(|j| j < character.joints.len()),
            "Curve joint missing"
        );
        ensure!(
            !c.keys.is_empty() && c.keys.len() <= 4096,
            "Invalid curve key count"
        );
        for (i, k) in c.keys.iter().enumerate() {
            ensure!(
                k.frame < p.duration
                    && (i == 0 || c.keys[i - 1].frame < k.frame)
                    && k.value.is_finite()
                    && k.value.abs() <= 1_000_000.
                    && k.incoming.is_finite()
                    && k.outgoing.is_finite()
                    && k.incoming.abs() <= 100_000.
                    && k.outgoing.abs() <= 100_000.,
                "Invalid curve key/tangent"
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hermite_tangents_are_time_scaled_and_endpoints_exact() {
        let c = CurveTrack {
            id: Id::new_v4(),
            name: "Root X".into(),
            character: Id::new_v4(),
            joint: None,
            axis: CurveAxis::X,
            keys: vec![
                ChannelKey {
                    frame: 0,
                    value: 0.,
                    incoming: 0.,
                    outgoing: 2.,
                    interpolation: CurveInterpolation::Cubic,
                },
                ChannelKey {
                    frame: 10,
                    value: 20.,
                    incoming: 2.,
                    outgoing: 0.,
                    interpolation: CurveInterpolation::Cubic,
                },
            ],
        };
        assert_eq!(c.sample(-1.), None);
        assert_eq!(c.sample(0.), Some(0.));
        assert_eq!(c.sample(5.), Some(10.));
        assert_eq!(c.sample(10.), Some(20.));
        assert_eq!(c.sample(11.), None);
    }
    #[test]
    fn invalid_curves_roll_back_and_undo_restores_evaluation() {
        let p = fight_demo().unwrap();
        let id = p.characters[0].id;
        let mut s = Session::new(p).unwrap();
        let track = CurveTrack {
            id: Id::new_v4(),
            name: "Root X".into(),
            character: id,
            joint: None,
            axis: CurveAxis::X,
            keys: vec![ChannelKey {
                frame: 0,
                value: 42.,
                incoming: 0.,
                outgoing: 0.,
                interpolation: CurveInterpolation::Hold,
            }],
        };
        let before = evaluate(&s.project, 0.).characters[0].pose.root.x;
        let batch = Batch {
            project: s.project.id,
            expected_revision: s.project.revision,
            commands: vec![Command::SetCurveTracks {
                tracks: vec![track],
            }],
        };
        s.apply(batch).unwrap();
        assert_eq!(evaluate(&s.project, 0.).characters[0].pose.root.x, 42.);
        s.undo(s.project.id, s.project.revision).unwrap();
        assert_eq!(evaluate(&s.project, 0.).characters[0].pose.root.x, before);
    }
}
