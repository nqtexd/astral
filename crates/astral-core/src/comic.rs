//! Editable print effects and independent source cadence (schema 12).
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ComicKey {
    pub frame: u32,
    pub strength: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum ComicStyle {
    RgbSplit {
        offset: Point,
        strength: f32,
        #[serde(default)]
        keys: Vec<ComicKey>,
    },
    Glitch {
        displacement: f32,
        band_height: u32,
        seed: u64,
        rate: u32,
        strength: f32,
        #[serde(default)]
        keys: Vec<ComicKey>,
    },
    Halftone {
        spacing: f32,
        angle: f32,
        ink: Color,
        strength: f32,
        #[serde(default)]
        keys: Vec<ComicKey>,
    },
}
impl ComicStyle {
    pub fn controls(&self) -> (f32, &[ComicKey]) {
        match self {
            Self::RgbSplit { strength, keys, .. }
            | Self::Glitch { strength, keys, .. }
            | Self::Halftone { strength, keys, .. } => (*strength, keys),
        }
    }
    pub fn strength_at(&self, frame: f64) -> f32 {
        let (base, keys) = self.controls();
        let Some(first) = keys.first() else {
            return base;
        };
        if frame <= first.frame as f64 {
            return first.strength;
        }
        let i = keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &keys[i];
        if let Some(b) = keys.get(i + 1) {
            let t = ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32;
            a.strength + (b.strength - a.strength) * t
        } else {
            a.strength
        }
    }
    pub fn animated(&self) -> bool {
        matches!(self, Self::Glitch { .. }) || !self.controls().1.is_empty()
    }
    pub fn validate(&self, duration: u32) -> Result<()> {
        let (strength, keys) = self.controls();
        ensure!(
            strength.is_finite() && (0.0..=1.0).contains(&strength),
            "Comic strength must be 0–1"
        );
        ensure!(
            keys.len() <= 4096
                && keys.windows(2).all(|k| k[0].frame < k[1].frame)
                && keys.iter().all(|k| k.frame < duration
                    && k.strength.is_finite()
                    && (0.0..=1.0).contains(&k.strength)),
            "Invalid comic strength keys"
        );
        match self {
            Self::RgbSplit { offset, .. } => ensure!(
                offset.finite() && offset.x.abs() <= 256. && offset.y.abs() <= 256.,
                "RGB offset exceeds 256 source pixels"
            ),
            Self::Glitch {
                displacement,
                band_height,
                rate,
                ..
            } => ensure!(
                displacement.is_finite()
                    && (0.0..=256.0).contains(displacement)
                    && (1..=1024).contains(band_height)
                    && (1..=240).contains(rate),
                "Invalid glitch displacement, band height or rate"
            ),
            Self::Halftone { spacing, angle, .. } => ensure!(
                spacing.is_finite()
                    && (2.0..=128.0).contains(spacing)
                    && angle.is_finite()
                    && angle.abs() <= std::f32::consts::TAU,
                "Invalid halftone spacing or angle (radians)"
            ),
        }
        Ok(())
    }
}
impl Project {
    pub fn timing_target_exists(&self, id: Id) -> bool {
        self.layers.iter().any(|v| v.id == id)
            || self.rasters.iter().any(|v| v.id == id)
            || self.graphics.iter().any(|v| v.id == id)
            || self.characters.iter().any(|v| v.id == id)
    }
    /// Absolute project-frame clock. A 12 FPS source in a 24 FPS project samples 0,0,2,2…
    pub fn layer_frame(&self, layer: Id, frame: f64) -> f64 {
        self.layer_frame_rates.get(&layer).map_or(frame, |fps| {
            let rate = (*fps).min(self.fps).max(1) as f64;
            (frame.max(0.) * rate / self.fps.max(1) as f64 + 1e-9).floor() * self.fps.max(1) as f64
                / rate
        })
    }
}
pub(crate) fn validate(p: &Project) -> Result<()> {
    ensure!(
        p.layer_frame_rates.is_empty() || p.schema >= 12,
        "Layer cadence requires schema 12"
    );
    for (id, fps) in &p.layer_frame_rates {
        ensure!(
            p.timing_target_exists(*id) && (1..=240).contains(fps),
            "Invalid layer cadence target or FPS"
        );
    }
    for n in p.compositions.iter().flat_map(|g| &g.nodes) {
        if let CompositeOp::Comic { style, .. } = &n.op {
            ensure!(p.schema >= 12, "Comic effects require schema 12");
            style.validate(p.duration)?;
        }
    }
    Ok(())
}
