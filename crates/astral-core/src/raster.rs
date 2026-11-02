//! Self-contained raster artwork. Immutable PNG blobs make undo cheap and portable.
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Bitmap {
    pub id: Id,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub png: Arc<str>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RasterKey {
    pub frame: u32,
    pub position: Point,
    pub scale: Point,
    pub rotation: f32,
    pub opacity: f32,
    #[serde(default)]
    pub easing: Easing,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PinKey {
    pub frame: u32,
    pub offset: Point,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MeshPin {
    pub id: Id,
    pub name: String,
    pub rest: Point,
    pub keys: Vec<PinKey>,
    #[serde(default)]
    pub driver: Option<PinDriver>,
}
/// An additive corrective driven by a bone's local rotation relative to its parent.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PinDriver {
    pub character: Id,
    pub joint: usize,
    pub rest_angle: f32,
    pub angle_range: f32,
    pub offset: Point,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeformMode {
    #[default]
    Translation,
    Similarity,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CurveKey {
    pub frame: u32,
    pub controls: [Point; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeformMesh {
    pub columns: u32,
    pub rows: u32,
    pub pins: Vec<MeshPin>,
    #[serde(default)]
    pub mode: DeformMode,
    #[serde(default)]
    pub curve: Vec<CurveKey>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BitmapCel {
    pub frame: u32,
    pub bitmap: Option<Id>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RasterLayer {
    pub id: Id,
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub sources: Vec<Id>,
    pub source_fps: u32,
    #[serde(default)]
    pub looping: bool,
    pub size: Point,
    pub keys: Vec<RasterKey>,
    #[serde(default)]
    pub cels: Vec<BitmapCel>,
    #[serde(default)]
    pub mesh: Option<DeformMesh>,
    #[serde(default)]
    pub layer: GraphicLayer,
    #[serde(default = "visible")]
    pub visible: bool,
}
fn visible() -> bool {
    true
}
impl RasterLayer {
    pub fn bitmap_at(&self, frame: f64, fps: u32) -> Id {
        if let Some(c) = self.cels.iter().rev().find(|c| c.frame as f64 <= frame)
            && let Some(bitmap) = c.bitmap
        {
            return bitmap;
        }
        let index = (((frame - self.start as f64).max(0.) / fps as f64) * self.source_fps as f64)
            .floor() as usize;
        self.sources[if self.looping {
            index % self.sources.len()
        } else {
            index.min(self.sources.len() - 1)
        }]
    }
    pub fn at(&self, frame: f64) -> RasterKey {
        let i = self
            .keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &self.keys[i];
        let mut out = a.clone();
        if let Some(b) = self.keys.get(i + 1) {
            let t = ease(
                &a.easing,
                ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            );
            out.position = a.position.lerp(b.position, t);
            out.scale = a.scale.lerp(b.scale, t);
            let delta = (b.rotation - a.rotation + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            out.rotation = a.rotation + delta * t;
            out.opacity = a.opacity + (b.opacity - a.opacity) * t;
        }
        out
    }
    pub fn local_point(&self, uv: Point, frame: f64) -> Point {
        let q = self.mesh.as_ref().map_or(uv, |mesh| {
            crate::deformation::deform_uv(mesh, uv, frame, self.size.x / self.size.y)
        });
        Point::new((q.x - 0.5) * self.size.x, (q.y - 0.5) * self.size.y)
    }

    pub fn world_point(&self, uv: Point, frame: f64) -> Point {
        let key = self.at(frame);
        let local = self.local_point(uv, frame);
        let x = local.x * key.scale.x;
        let y = local.y * key.scale.y;
        key.position
            + Point::new(
                x * key.rotation.cos() - y * key.rotation.sin(),
                x * key.rotation.sin() + y * key.rotation.cos(),
            )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MouthShape {
    Rest,
    A,
    E,
    O,
    F,
    M,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MouthKey {
    pub frame: u32,
    pub shape: MouthShape,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MouthTrack {
    pub id: Id,
    pub name: String,
    pub character: Id,
    pub start: u32,
    pub end: u32,
    pub size: Point,
    pub offset: Point,
    pub color: Color,
    pub keys: Vec<MouthKey>,
}
pub fn validate(p: &Project) -> Result<()> {
    ensure!(
        p.bitmaps.len() <= 512 && p.rasters.len() <= 128 && p.mouths.len() <= 64,
        "Raster/mouth track budget exceeded"
    );
    if !p.bitmaps.is_empty() || !p.rasters.is_empty() || !p.mouths.is_empty() {
        ensure!(
            p.schema >= 8,
            "Raster artwork and mouth tracks require schema 8"
        );
    }
    let mut pixels = 0u64;
    let mut encoded = 0usize;
    for b in &p.bitmaps {
        ensure!(
            b.width > 0
                && b.height > 0
                && b.width <= 8192
                && b.height <= 8192
                && b.width as u64 * b.height as u64 <= 16_777_216
                && !b.png.is_empty()
                && b.name.len() <= 512,
            "Invalid bitmap dimensions or name"
        );
        pixels += b.width as u64 * b.height as u64;
        encoded += b.png.len();
    }
    ensure!(
        pixels <= 134_217_728 && encoded <= 96 * 1024 * 1024,
        "Artwork exceeds 128 megapixels or 96 MiB encoded budget"
    );
    let bitmap = |id: Id| p.bitmaps.iter().any(|b| b.id == id);
    for r in &p.rasters {
        ensure!(
            r.name.len() <= 512
                && r.start <= r.end
                && r.end < p.duration
                && !r.sources.is_empty()
                && r.sources.len() <= 512
                && r.sources.iter().all(|id| bitmap(*id))
                && (1..=240).contains(&r.source_fps)
                && r.size.finite()
                && r.size.x >= 0.001
                && r.size.y >= 0.001
                && r.size.x <= 16384.
                && r.size.y <= 16384.,
            "Invalid raster track, dimensions or sources"
        );
        ensure!(
            !r.keys.is_empty() && r.keys.len() <= 12000,
            "Raster track needs bounded transform keys"
        );
        for (i, k) in r.keys.iter().enumerate() {
            ensure!(
                k.frame < p.duration
                    && (i == 0 || k.frame > r.keys[i - 1].frame)
                    && k.position.finite()
                    && k.scale.finite()
                    && k.scale.x.abs() >= 0.001
                    && k.scale.y.abs() >= 0.001
                    && k.scale.x.abs() <= 100.
                    && k.scale.y.abs() <= 100.
                    && k.rotation.is_finite()
                    && k.opacity.is_finite()
                    && (0.0..=1.).contains(&k.opacity),
                "Invalid raster transform keys"
            );
            crate::model::validate_easing(&k.easing)?;
        }
        for (i, c) in r.cels.iter().enumerate() {
            ensure!(
                c.bitmap.is_none_or(bitmap)
                    && c.frame < p.duration
                    && (i == 0 || c.frame > r.cels[i - 1].frame),
                "Invalid bitmap cels"
            );
        }
        if let Some(m) = &r.mesh {
            ensure!(
                p.schema >= 9
                    || (m.mode == DeformMode::Translation
                        && m.curve.is_empty()
                        && m.pins.iter().all(|p| p.driver.is_none())),
                "Advanced deformation requires schema 9"
            );
            ensure!(
                (1..=32).contains(&m.columns) && (1..=32).contains(&m.rows) && m.pins.len() <= 64,
                "Invalid deformation grid"
            );
            ensure!(m.curve.len() <= 12000, "Too many curve keys");
            for (i, k) in m.curve.iter().enumerate() {
                ensure!(
                    k.frame < p.duration
                        && (i == 0 || k.frame > m.curve[i - 1].frame)
                        && k.controls
                            .iter()
                            .all(|q| q.finite() && q.x.abs() <= 8. && q.y.abs() <= 8.),
                    "Invalid curver keys"
                );
            }
            let mut ids = std::collections::BTreeSet::new();
            for pin in &m.pins {
                if let Some(d) = &pin.driver {
                    ensure!(
                        p.character(d.character).is_some_and(
                            |c| d.joint < c.joints.len() && c.joints[d.joint].parent.is_some()
                        ) && d.rest_angle.is_finite()
                            && d.angle_range.is_finite()
                            && d.angle_range.abs() >= 0.001
                            && d.angle_range.abs() <= std::f32::consts::TAU
                            && d.offset.finite()
                            && d.offset.x.abs() <= 4.
                            && d.offset.y.abs() <= 4.,
                        "Invalid corrective bone driver"
                    );
                }
                ensure!(
                    ids.insert(pin.id)
                        && pin.name.len() <= 128
                        && pin.rest.finite()
                        && (0.0..=1.).contains(&pin.rest.x)
                        && (0.0..=1.).contains(&pin.rest.y)
                        && !pin.keys.is_empty()
                        && pin.keys.len() <= 12000,
                    "Invalid mesh pin"
                );
                for (i, k) in pin.keys.iter().enumerate() {
                    ensure!(
                        k.frame < p.duration
                            && (i == 0 || k.frame > pin.keys[i - 1].frame)
                            && k.offset.finite()
                            && k.offset.x.abs() <= 4.
                            && k.offset.y.abs() <= 4.,
                        "Invalid pin keys"
                    );
                }
            }
        }
    }
    for m in &p.mouths {
        ensure!(
            p.character(m.character).is_some()
                && m.start <= m.end
                && m.end < p.duration
                && m.size.finite()
                && m.size.x > 0.
                && m.size.y > 0.
                && m.size.x <= 200.
                && m.size.y <= 200.
                && m.offset.finite()
                && !m.keys.is_empty()
                && m.keys.len() <= 12000,
            "Invalid mouth track"
        );
        for (i, k) in m.keys.iter().enumerate() {
            ensure!(
                k.frame >= m.start && k.frame <= m.end && (i == 0 || k.frame > m.keys[i - 1].frame),
                "Invalid mouth keys"
            );
        }
    }
    Ok(())
}
