//! Live instances reference the editable library rather than baking its contents.
use crate::*;
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SymbolInstance {
    pub id: Id,
    pub name: String,
    pub entry: Id,
    pub parent: Option<Id>,
    /// Child start/end/offset are measured in the parent's local frames.
    pub start: u32,
    pub end: u32,
    pub offset: f32,
    pub speed: f32,
    pub looping: bool,
    pub layer: GraphicLayer,
    pub keys: Vec<GraphicKey>,
}
impl SymbolInstance {
    pub fn at(&self, frame: f64) -> GraphicKey {
        let i = self
            .keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let mut a = self.keys[i].clone();
        if let Some(b) = self.keys.get(i + 1) {
            let t = ease(
                &a.easing,
                ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            );
            a.position = a.position.lerp(b.position, t);
            a.scale += (b.scale - a.scale) * t;
            a.rotation += (b.rotation - a.rotation) * t;
            a.opacity += (b.opacity - a.opacity) * t;
        }
        a
    }
    pub fn local_frame(&self, frame: f64, duration: u32) -> Option<f64> {
        if frame < self.start as f64 || frame > self.end as f64 {
            return None;
        }
        let t = self.offset as f64 + (frame - self.start as f64) * self.speed as f64;
        Some(if self.looping {
            t.rem_euclid(duration as f64)
        } else {
            t.clamp(0., (duration - 1) as f64)
        })
    }
}
pub fn validate_symbols(p: &Project) -> Result<()> {
    ensure!(p.symbols.len() <= 256, "Too many symbol instances");
    if p.symbols.is_empty() {
        return Ok(());
    }
    let entries = studio::library_entries(p)?;
    let mut ids = BTreeSet::new();
    for s in &p.symbols {
        ensure!(
            ids.insert(s.id) && s.name.len() <= 512,
            "Invalid symbol identity/name"
        );
        ensure!(
            entries.iter().any(|e| e.id == s.entry),
            "Symbol library source missing"
        );
        let duration = if let Some(parent) = s.parent {
            let parent = p
                .symbols
                .iter()
                .find(|v| v.id == parent)
                .context("Symbol parent missing")?;
            entries
                .iter()
                .find(|e| e.id == parent.entry)
                .context("Parent source missing")?
                .duration
        } else {
            p.duration
        };
        ensure!(
            s.start <= s.end
                && s.end < duration
                && s.offset.is_finite()
                && s.offset >= 0.
                && s.offset <= 1_000_000.
                && s.speed.is_finite()
                && (0.01..=16.).contains(&s.speed),
            "Invalid symbol timing"
        );
        ensure!(
            !s.keys.is_empty() && s.keys.len() <= 4096,
            "Invalid symbol transform keys"
        );
        for (i, k) in s.keys.iter().enumerate() {
            ensure!(
                k.frame < duration
                    && (i == 0 || s.keys[i - 1].frame < k.frame)
                    && k.position.finite()
                    && k.position.x.abs() <= 1_000_000.
                    && k.position.y.abs() <= 1_000_000.
                    && k.scale.is_finite()
                    && (0.001..=100.).contains(&k.scale)
                    && k.rotation.is_finite()
                    && k.rotation.abs() <= 100_000.
                    && k.opacity.is_finite()
                    && (0.0..=1.).contains(&k.opacity),
                "Invalid symbol transform"
            );
            crate::validate_easing(&k.easing)?;
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(s.id);
        while let Some(id) = current {
            ensure!(
                seen.insert(id) && seen.len() <= 8,
                "Symbol hierarchy cycles or exceeds eight levels"
            );
            current = p
                .symbols
                .iter()
                .find(|v| v.id == id)
                .context("Symbol parent missing")?
                .parent;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_time_handles_loops_and_holds() {
        let s = SymbolInstance {
            id: Id::new_v4(),
            name: "Loop".into(),
            entry: Id::new_v4(),
            parent: None,
            start: 10,
            end: 30,
            offset: 0.,
            speed: 2.,
            looping: true,
            layer: GraphicLayer::Foreground,
            keys: vec![],
        };
        assert_eq!(s.local_frame(9., 8), None);
        assert_eq!(s.local_frame(15., 8), Some(2.));
    }
}
