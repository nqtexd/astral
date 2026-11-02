//! Production-oriented layer staging and linear-light finishing (schema 13).
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LayerDepth {
    /// 0 locks translation to the camera anchor, 1 is normal, >1 is foreground.
    pub parallax: f32,
    /// Independent response to camera zoom; 0 locks scale, 1 is normal.
    pub zoom: f32,
    /// Camera-space anchor. Omit to use the first camera key (or canvas center).
    #[serde(default)]
    pub anchor: Option<Point>,
}
impl LayerDepth {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.parallax.is_finite()
                && (0.0..=2.0).contains(&self.parallax)
                && self.zoom.is_finite()
                && (0.0..=2.0).contains(&self.zoom)
                && self.anchor.is_none_or(|p| p.finite()),
            "Invalid parallax or zoom response (0–2), or anchor"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum CinematicEffect {
    Bloom {
        threshold: f32,
        knee: f32,
        radius: u32,
        intensity: f32,
    },
    DropShadow {
        offset: Point,
        radius: u32,
        color: Color,
        opacity: f32,
    },
    RimLight {
        offset: Point,
        radius: u32,
        color: Color,
        intensity: f32,
    },
    Grade {
        exposure: f32,
        contrast: f32,
        saturation: f32,
        lift: [f32; 3],
        gamma: [f32; 3],
        gain: [f32; 3],
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CinematicStage {
    pub effect: CinematicEffect,
    pub mix: f32,
    #[serde(default)]
    pub keys: Vec<ComicKey>,
}
impl CinematicStage {
    pub fn mix_at(&self, frame: f64) -> f32 {
        // Reuse the validated scalar strength-key interpolation contract.
        if self.keys.is_empty() {
            return self.mix;
        }
        if frame <= self.keys[0].frame as f64 {
            return self.keys[0].strength;
        }
        let i = self
            .keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &self.keys[i];
        self.keys.get(i + 1).map_or(a.strength, |b| {
            a.strength
                + (b.strength - a.strength)
                    * ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32
        })
    }
    pub fn validate(&self, duration: u32) -> Result<()> {
        ensure!(
            self.mix.is_finite()
                && (0.0..=1.0).contains(&self.mix)
                && self.keys.len() <= 4096
                && self.keys.windows(2).all(|w| w[0].frame < w[1].frame)
                && self.keys.iter().all(|k| k.frame < duration
                    && k.strength.is_finite()
                    && (0.0..=1.0).contains(&k.strength)),
            "Invalid cinematic mix or keys"
        );
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        match &self.effect {
            CinematicEffect::Bloom {
                threshold,
                knee,
                radius,
                intensity,
            } => ensure!(
                unit(*threshold)
                    && unit(*knee)
                    && *radius <= 64
                    && intensity.is_finite()
                    && (0.0..=8.0).contains(intensity),
                "Invalid bloom controls"
            ),
            CinematicEffect::DropShadow {
                offset,
                radius,
                opacity,
                ..
            } => ensure!(
                offset.finite()
                    && offset.x.abs() <= 512.
                    && offset.y.abs() <= 512.
                    && *radius <= 64
                    && unit(*opacity),
                "Invalid shadow controls"
            ),
            CinematicEffect::RimLight {
                offset,
                radius,
                intensity,
                ..
            } => ensure!(
                offset.finite()
                    && offset.x.abs() <= 128.
                    && offset.y.abs() <= 128.
                    && *radius <= 32
                    && intensity.is_finite()
                    && (0.0..=8.0).contains(intensity),
                "Invalid rim light controls"
            ),
            CinematicEffect::Grade {
                exposure,
                contrast,
                saturation,
                lift,
                gamma,
                gain,
            } => ensure!(
                exposure.is_finite()
                    && (-8.0..=8.0).contains(exposure)
                    && contrast.is_finite()
                    && (0.0..=3.0).contains(contrast)
                    && saturation.is_finite()
                    && (0.0..=4.0).contains(saturation)
                    && lift
                        .iter()
                        .all(|v| v.is_finite() && (-0.25..=0.25).contains(v))
                    && gamma
                        .iter()
                        .all(|v| v.is_finite() && (0.1..=4.0).contains(v))
                    && gain
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=4.0).contains(v)),
                "Invalid linear-light grade controls"
            ),
        }
        Ok(())
    }
}
impl Project {
    pub fn depth_target_exists(&self, id: Id) -> bool {
        self.layers.iter().any(|v| v.id == id)
            || self.rasters.iter().any(|v| v.id == id)
            || self.graphics.iter().any(|v| v.id == id)
    }
    pub fn scene_for_layer<'a>(
        &self,
        id: Id,
        scene: &'a EvaluatedScene,
    ) -> Cow<'a, EvaluatedScene> {
        let Some(depth) = self.layer_depths.get(&id) else {
            return Cow::Borrowed(scene);
        };
        let anchor = depth
            .anchor
            .or_else(|| self.camera.first().map(|k| k.center))
            .unwrap_or(Point::new(self.width as f32 / 2., self.height as f32 / 2.));
        let anchor_zoom = self.camera.first().map_or(1., |k| k.zoom);
        let mut adjusted = scene.clone();
        adjusted.center = anchor.lerp(scene.center, depth.parallax);
        adjusted.zoom = anchor_zoom * (scene.zoom / anchor_zoom).powf(depth.zoom);
        Cow::Owned(adjusted)
    }
}
pub(crate) fn validate(p: &Project) -> Result<()> {
    let count = p
        .compositions
        .iter()
        .flat_map(|g| &g.nodes)
        .filter(|n| matches!(n.op, CompositeOp::Cinematic { .. }))
        .count() as u64;
    let area = p.width as u64 * p.height as u64;
    ensure!(
        count == 0 || (area <= 8_388_608 && area * count <= 33_554_432),
        "Cinematic workspace exceeds eight megapixels per frame or thirty-two megapixels of finishing nodes"
    );
    ensure!(
        (p.layer_depths.is_empty() && p.camera_easing.is_empty()) || p.schema >= 13,
        "Cinematic staging requires schema 13"
    );
    for (id, depth) in &p.layer_depths {
        ensure!(p.depth_target_exists(*id), "Depth layer missing");
        depth.validate()?;
    }
    for (frame, easing) in &p.camera_easing {
        ensure!(
            p.camera.iter().any(|k| k.frame == *frame),
            "Camera easing key missing"
        );
        crate::model::validate_easing(easing)?;
        if let Easing::Bezier { y1, y2, .. } = easing {
            ensure!(
                (0.0..=1.0).contains(y1) && (0.0..=1.0).contains(y2),
                "Camera easing must stay within segment endpoints"
            );
        }
    }
    for n in p.compositions.iter().flat_map(|g| &g.nodes) {
        if let CompositeOp::Cinematic { stage, .. } = &n.op {
            ensure!(p.schema >= 13, "Cinematic finishing requires schema 13");
            stage.validate(p.duration)?;
        }
    }
    Ok(())
}
