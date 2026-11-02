//! Editable, bounded compositing DAGs. Inputs are existing document entities.
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
    Add,
}
impl BlendMode {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::Difference,
        Self::Add,
    ];
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompositeOp {
    /// Print-inspired processing on any raster, drawing, graphic or composed input.
    Comic {
        source: Id,
        style: crate::ComicStyle,
    },
    Raster {
        layer: Id,
    },
    Drawing {
        layer: Id,
    },
    Graphic {
        graphic: Id,
    },
    Dialogue {
        dialogue: Id,
    },
    Solid {
        color: Color,
    },
    Gradient {
        start: Point,
        end: Point,
        from: Color,
        to: Color,
        #[serde(default)]
        radial: bool,
    },
    Blend {
        below: Id,
        above: Id,
        mode: BlendMode,
        opacity: f32,
    },
    Clip {
        below: Id,
        above: Id,
        mode: BlendMode,
        opacity: f32,
    },
    Mask {
        source: Id,
        mask: Id,
        #[serde(default)]
        invert: bool,
        #[serde(default)]
        luminance: bool,
    },
    Blur {
        source: Id,
        radius: u32,
    },
    Color {
        source: Id,
        exposure: f32,
        saturation: f32,
    },
}
impl CompositeOp {
    pub fn inputs(&self) -> Vec<Id> {
        match self {
            Self::Blend { below, above, .. } | Self::Clip { below, above, .. } => {
                vec![*below, *above]
            }
            Self::Mask { source, mask, .. } => vec![*source, *mask],
            Self::Blur { source, .. } | Self::Color { source, .. } | Self::Comic { source, .. } => {
                vec![*source]
            }
            _ => vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompositeNode {
    pub id: Id,
    pub name: String,
    #[serde(flatten)]
    pub op: CompositeOp,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Composition {
    pub id: Id,
    pub name: String,
    pub enabled: bool,
    pub layer: GraphicLayer,
    pub output: Id,
    pub nodes: Vec<CompositeNode>,
}
impl Composition {
    /// Dependency order, independent of storage/UI ordering. Reject cycles, including unreachable nodes.
    pub fn order(&self) -> Result<Vec<Id>> {
        let nodes: BTreeMap<_, _> = self.nodes.iter().map(|n| (n.id, n)).collect();
        ensure!(
            nodes.len() == self.nodes.len() && nodes.contains_key(&self.output),
            "Duplicate nodes or missing composition output"
        );
        fn visit(
            id: Id,
            nodes: &BTreeMap<Id, &CompositeNode>,
            active: &mut BTreeSet<Id>,
            done: &mut BTreeSet<Id>,
            out: &mut Vec<Id>,
        ) -> Result<()> {
            if done.contains(&id) {
                return Ok(());
            }
            ensure!(active.insert(id), "Compositing cycle");
            let n = nodes
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Missing compositing input"))?;
            for input in n.op.inputs() {
                visit(input, nodes, active, done, out)?;
            }
            active.remove(&id);
            done.insert(id);
            out.push(id);
            Ok(())
        }
        let (mut active, mut done, mut out) = (BTreeSet::new(), BTreeSet::new(), vec![]);
        for id in nodes.keys() {
            visit(*id, &nodes, &mut active, &mut done, &mut out)?;
        }
        Ok(out)
    }
}
pub fn validate(p: &Project) -> Result<()> {
    ensure!(p.compositions.len() <= 8, "At most eight compositions");
    ensure!(
        p.compositions.is_empty() || p.schema >= 10,
        "Compositions require schema 10"
    );
    let count: usize = p.compositions.iter().map(|c| c.nodes.len()).sum();
    ensure!(
        count <= 128 && count as u64 * p.width as u64 * p.height as u64 <= 67_108_864,
        "Compositing workspace exceeds 64 megapixels; reduce canvas size or node count"
    );
    let mut ids = BTreeSet::new();
    for c in &p.compositions {
        ensure!(
            ids.insert(c.id) && c.name.len() <= 512 && !c.nodes.is_empty() && c.nodes.len() <= 64,
            "Invalid composition"
        );
        c.order()?;
        for n in &c.nodes {
            ensure!(
                ids.insert(n.id) && n.name.len() <= 512,
                "Invalid/duplicate compositing node"
            );
            let input = match n.op {
                CompositeOp::Raster { layer } => {
                    ensure!(
                        p.rasters.iter().any(|r| r.id == layer),
                        "Raster input missing"
                    );
                    Some(layer)
                }
                CompositeOp::Drawing { layer } => {
                    ensure!(
                        p.layers.iter().any(|r| r.id == layer),
                        "Drawing input missing"
                    );
                    Some(layer)
                }
                CompositeOp::Graphic { graphic } => {
                    ensure!(
                        p.graphics.iter().any(|r| r.id == graphic),
                        "Graphic input missing"
                    );
                    Some(graphic)
                }
                CompositeOp::Dialogue { dialogue } => {
                    ensure!(
                        p.dialogues.iter().any(|r| r.id == dialogue),
                        "Dialogue input missing"
                    );
                    Some(dialogue)
                }
                CompositeOp::Blend { opacity, .. } | CompositeOp::Clip { opacity, .. } => {
                    ensure!(
                        !matches!(n.op, CompositeOp::Clip { .. }) || p.schema >= 11,
                        "Clipping nodes require schema 11"
                    );
                    ensure!(
                        opacity.is_finite() && (0.0..=1.0).contains(&opacity),
                        "Invalid blend opacity"
                    );
                    None
                }
                CompositeOp::Blur { radius, .. } => {
                    ensure!(radius <= 32, "Blur radius exceeds 32 pixels");
                    None
                }
                CompositeOp::Color {
                    exposure,
                    saturation,
                    ..
                } => {
                    ensure!(
                        exposure.is_finite()
                            && (-8.0..=8.0).contains(&exposure)
                            && saturation.is_finite()
                            && (0.0..=4.0).contains(&saturation),
                        "Invalid color adjustment"
                    );
                    None
                }
                CompositeOp::Gradient { start, end, .. } => {
                    ensure!(
                        start.finite()
                            && end.finite()
                            && start.distance(end) > 0.0001
                            && start.x.abs() <= 4.
                            && start.y.abs() <= 4.
                            && end.x.abs() <= 4.
                            && end.y.abs() <= 4.,
                        "Invalid normalized gradient endpoints"
                    );
                    None
                }
                _ => None,
            };
            let _ = input;
        }
    }
    Ok(())
}
