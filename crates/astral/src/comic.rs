//! Agent-facing comic looks, pixel inspection and bounded distortion refinement.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
pub enum ComicRequest {
    /// Wrap a free drawing/raster/graphic input; existing compositions are preserved.
    CreateLook {
        project: Id,
        expected_revision: u64,
        source: Id,
        styles: Vec<ComicStyle>,
        layer_fps: Option<u32>,
        #[serde(default)]
        apply: bool,
    },
    InspectFrame {
        project: Id,
        frame: f64,
    },
    /// Reduce effect strength until the mean RGB difference budget is met, at most 3 rounds.
    Refine {
        project: Id,
        expected_revision: u64,
        composition: Id,
        frames: Vec<u32>,
        max_difference: f32,
        rounds: u32,
        #[serde(default)]
        apply: bool,
    },
}
fn image(p: &Project, frame: f64) -> Result<Vec<u8>> {
    let scale = (512. / p.width.max(p.height) as f32).min(1.);
    astral_render::png(
        p,
        frame,
        (p.width as f32 * scale).round().max(1.) as u32,
        (p.height as f32 * scale).round().max(1.) as u32,
    )
}
fn difference(p: &Project, reference: &Project, frames: &[u32]) -> Result<f32> {
    let mut worst = 0f32;
    for frame in frames {
        let a = image::load_from_memory(&image(p, *frame as f64)?)?.into_rgba8();
        let b = image::load_from_memory(&image(reference, *frame as f64)?)?.into_rgba8();
        // Compare composited RGB over black, so transparent colors do not count as differences.
        let sum: f64 = a
            .pixels()
            .zip(b.pixels())
            .map(|(a, b)| {
                (0..3)
                    .map(|c| {
                        (a[c] as f64 * a[3] as f64 / 255. - b[c] as f64 * b[3] as f64 / 255.).abs()
                            / 255.
                    })
                    .sum::<f64>()
            })
            .sum();
        worst = worst.max((sum / (a.width() as f64 * a.height() as f64 * 3.)) as f32);
    }
    Ok(worst)
}
impl Service {
    pub fn comic_tools(&self, request: ComicRequest) -> Result<Value> {
        match request {
            ComicRequest::InspectFrame { project, frame } => {
                let p = self.snapshot(project)?;
                ensure!(
                    frame.is_finite() && frame >= 0. && frame <= (p.duration - 1) as f64,
                    "Frame outside project"
                );
                let png = image(&p, frame)?;
                let rgba = image::load_from_memory(&png)?.into_rgba8();
                let covered = rgba.pixels().filter(|p| p[3] > 0).count();
                Ok(
                    json!({"project":project,"revision":p.revision,"frame":frame,"width":rgba.width(),"height":rgba.height(),"covered_pixels":covered,"layer_frame_rates":p.layer_frame_rates,"image_base64":STANDARD.encode(png),"note":"Actual shared-renderer pixels; inspect the image for composition and acting."}),
                )
            }
            ComicRequest::CreateLook {
                project,
                expected_revision,
                source,
                styles,
                layer_fps,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(p.revision == expected_revision, "Stale revision");
                ensure!(
                    !styles.is_empty() && styles.len() <= 3,
                    "Use 1–3 effects per look"
                );
                ensure!(!p.compositions.iter().flat_map(|g| &g.nodes).any(|n| matches!(n.op,CompositeOp::Drawing{layer}|CompositeOp::Raster{layer} if layer==source) || matches!(n.op,CompositeOp::Graphic{graphic} if graphic==source)),"Source already composed; edit its existing graph using set_compositions");
                let (op, layer) = if p.layers.iter().any(|l| l.id == source) {
                    (
                        CompositeOp::Drawing { layer: source },
                        GraphicLayer::Foreground,
                    )
                } else if let Some(r) = p.rasters.iter().find(|r| r.id == source) {
                    (CompositeOp::Raster { layer: source }, r.layer)
                } else if let Some(g) = p.graphics.iter().find(|g| g.id == source) {
                    (CompositeOp::Graphic { graphic: source }, g.layer)
                } else {
                    anyhow::bail!("Source must be a drawing, raster or graphic ID");
                };
                let mut output = Id::new_v4();
                let mut nodes = vec![CompositeNode {
                    id: output,
                    name: "Artwork".into(),
                    op,
                }];
                for style in styles {
                    style.validate(p.duration)?;
                    let id = Id::new_v4();
                    nodes.push(CompositeNode {
                        id,
                        name: "Comic effect".into(),
                        op: CompositeOp::Comic {
                            source: output,
                            style,
                        },
                    });
                    output = id;
                }
                let id = Id::new_v4();
                let mut graphs = p.compositions.clone();
                graphs.push(Composition {
                    id,
                    name: "Comic look".into(),
                    enabled: true,
                    layer,
                    output,
                    nodes,
                });
                let mut commands = vec![Command::SetCompositions {
                    compositions: graphs,
                }];
                if let Some(fps) = layer_fps {
                    commands.push(Command::SetLayerFrameRate {
                        layer: source,
                        fps: Some(fps),
                    });
                }
                self.comic_plan(
                    p,
                    expected_revision,
                    commands,
                    apply,
                    json!({"composition":id}),
                )
            }
            ComicRequest::Refine {
                project,
                expected_revision,
                composition,
                frames,
                max_difference,
                rounds,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(p.revision == expected_revision, "Stale revision");
                ensure!(
                    !frames.is_empty()
                        && frames.len() <= 8
                        && frames.iter().all(|f| *f < p.duration),
                    "Sample 1–8 project frames"
                );
                ensure!(
                    (1..=3).contains(&rounds)
                        && max_difference.is_finite()
                        && (0.01..=0.5).contains(&max_difference),
                    "Use 1–3 rounds and difference budget 0.01–0.5"
                );
                let index = p
                    .compositions
                    .iter()
                    .position(|c| c.id == composition && c.enabled)
                    .context("Enabled composition missing")?;
                ensure!(
                    p.compositions[index]
                        .nodes
                        .iter()
                        .any(|n| matches!(n.op, CompositeOp::Comic { .. })),
                    "Composition has no comic effects"
                );
                let mut reference = p.clone();
                for n in &mut reference.compositions[index].nodes {
                    if let CompositeOp::Comic { style, .. } = &mut n.op {
                        scale_strength(style, 0.);
                    }
                }
                let mut proposal = p.clone();
                let before = difference(&proposal, &reference, &frames)?;
                let mut after = before;
                let mut iterations = 0;
                while after > max_difference && iterations < rounds {
                    for n in &mut proposal.compositions[index].nodes {
                        if let CompositeOp::Comic { style, .. } = &mut n.op {
                            scale_strength(style, 0.5);
                        }
                    }
                    iterations += 1;
                    after = difference(&proposal, &reference, &frames)?;
                }
                let evidence = json!({"frames":frames,"iterations":iterations,"before_difference":before,"after_difference":after,"budget_met":after<=max_difference,"max_difference":max_difference,"original_image_base64":STANDARD.encode(image(&p,frames[0] as f64)?),"image_base64":STANDARD.encode(image(&proposal,frames[0] as f64)?),"note":"Measured worst sampled mean RGB difference from the same scene with this graph's comic effects bypassed. Reduces distortion only; does not judge artistic quality or certify unsampled frames."});
                let commands = if iterations > 0 {
                    vec![Command::SetCompositions {
                        compositions: proposal.compositions,
                    }]
                } else {
                    vec![]
                };
                self.comic_plan(p, expected_revision, commands, apply, evidence)
            }
        }
    }
    fn comic_plan(
        &self,
        p: Project,
        revision: u64,
        commands: Vec<Command>,
        apply: bool,
        mut evidence: Value,
    ) -> Result<Value> {
        let id = p.id;
        if !commands.is_empty() {
            let batch = Batch {
                project: id,
                expected_revision: revision,
                commands: commands.clone(),
            };
            let mut sandbox = Session::new(p)?;
            sandbox.apply(batch.clone())?;
            if apply {
                evidence["receipt"] = self.request(Request::ApplyCommands { batch })?;
            }
        }
        evidence["project"] = json!(id);
        evidence["expected_revision"] = json!(revision);
        evidence["commands"] = json!(commands);
        evidence["applied"] = json!(apply && !commands.is_empty());
        Ok(evidence)
    }
}
fn scale_strength(style: &mut ComicStyle, factor: f32) {
    let (strength, keys) = match style {
        ComicStyle::RgbSplit { strength, keys, .. }
        | ComicStyle::Glitch { strength, keys, .. }
        | ComicStyle::Halftone { strength, keys, .. } => (strength, keys),
    };
    *strength *= factor;
    for key in keys {
        key.strength *= factor;
    }
}
