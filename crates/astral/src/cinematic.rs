//! Reviewable agent operations for cinematic staging and finishing.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Cursor};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DepthAssignment {
    pub layer: Id,
    pub depth: Option<LayerDepth>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type"="object"))]
pub enum CinematicRequest {
    /// Replaces the full camera track; preserves other animation. Easing is attached to each segment start.
    StageShot {
        project: Id,
        expected_revision: u64,
        keys: Vec<CameraKey>,
        easing: Easing,
        layers: Vec<DepthAssignment>,
        preview_frame: u32,
        #[serde(default)]
        apply: bool,
    },
    /// Append finishing nodes to an existing composition's output.
    Finish {
        project: Id,
        expected_revision: u64,
        composition: Id,
        stages: Vec<CinematicStage>,
        preview_frame: u32,
        #[serde(default)]
        apply: bool,
    },
    Review {
        project: Id,
        frames: Vec<u32>,
    },
}
fn preview(p: &Project, frame: u32, width: u32) -> Result<Vec<u8>> {
    let scale = (width as f32 / p.width.max(p.height) as f32).min(1.);
    astral_render::png(
        p,
        frame as f64,
        (p.width as f32 * scale).round().max(1.) as u32,
        (p.height as f32 * scale).round().max(1.) as u32,
    )
}
impl Service {
    pub fn cinematic_tools(&self, request: CinematicRequest) -> Result<Value> {
        match request {
            CinematicRequest::StageShot {
                project,
                expected_revision,
                keys,
                easing,
                layers,
                preview_frame,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    !keys.is_empty()
                        && keys.len() <= 64
                        && keys.windows(2).all(|w| w[0].frame < w[1].frame),
                    "Provide 1–64 increasing camera keys"
                );
                ensure!(layers.len() <= 128, "At most 128 depth assignments");
                let mut seen = std::collections::BTreeSet::new();
                ensure!(
                    layers.iter().all(|l| seen.insert(l.layer)),
                    "Duplicate depth assignment"
                );
                let easing: BTreeMap<_, _> = keys
                    .iter()
                    .take(keys.len().saturating_sub(1))
                    .map(|k| (k.frame, easing.clone()))
                    .collect();
                let mut commands = vec![Command::SetCinematicCamera { keys, easing }];
                commands.extend(layers.into_iter().map(|l| Command::SetLayerDepth {
                    layer: l.layer,
                    depth: l.depth,
                }));
                self.cinematic_plan(p, expected_revision, commands, preview_frame, apply)
            }
            CinematicRequest::Finish {
                project,
                expected_revision,
                composition,
                stages,
                preview_frame,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    !stages.is_empty() && stages.len() <= 4,
                    "Append 1–4 finishing stages"
                );
                let mut graphs = p.compositions.clone();
                let graph = graphs
                    .iter_mut()
                    .find(|g| g.id == composition && g.enabled)
                    .context("Enabled composition missing")?;
                for stage in stages {
                    stage.validate(p.duration)?;
                    let id = Id::new_v4();
                    graph.nodes.push(CompositeNode {
                        id,
                        name: "Cinematic finish".into(),
                        op: CompositeOp::Cinematic {
                            source: graph.output,
                            stage,
                        },
                    });
                    graph.output = id;
                }
                self.cinematic_plan(
                    p,
                    expected_revision,
                    vec![Command::SetCompositions {
                        compositions: graphs,
                    }],
                    preview_frame,
                    apply,
                )
            }
            CinematicRequest::Review { project, frames } => {
                let p = self.snapshot(project)?;
                ensure!(
                    !frames.is_empty()
                        && frames.len() <= 6
                        && frames.windows(2).all(|w| w[0] < w[1])
                        && frames.iter().all(|f| *f < p.duration),
                    "Review 1–6 increasing project frames"
                );
                let images: Vec<_> = frames
                    .iter()
                    .map(|f| Ok(image::load_from_memory(&preview(&p, *f, 384)?)?.into_rgba8()))
                    .collect::<Result<_>>()?;
                let (w, h) = images[0].dimensions();
                let columns = images.len().min(3) as u32;
                let rows = (images.len() as u32).div_ceil(columns);
                let mut sheet = image::RgbaImage::new(w * columns, h * rows);
                let mut metrics = vec![];
                let mut warnings = vec![];
                for (i, img) in images.iter().enumerate() {
                    image::imageops::overlay(
                        &mut sheet,
                        img,
                        (i as u32 % columns * w) as i64,
                        (i as u32 / columns * h) as i64,
                    );
                    let total = (w * h) as f64;
                    let mut luminance = 0.;
                    let mut black = 0;
                    let mut white = 0;
                    for pixel in img.pixels() {
                        let a = pixel[3] as f64 / 255.;
                        let rgb = [0, 1, 2].map(|c| {
                            astral_render::cinematic::srgb_to_linear(pixel[c] as f32 / 255.) as f64
                        });
                        let l = (rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722) * a;
                        luminance += l;
                        if l < 0.005 {
                            black += 1;
                        }
                        if l > 0.98 {
                            white += 1;
                        }
                    }
                    let change = if i == 0 {
                        None
                    } else {
                        Some(
                            img.pixels()
                                .zip(images[i - 1].pixels())
                                .map(|(a, b)| {
                                    (0..3)
                                        .map(|c| {
                                            ((a[c] as f64 * a[3] as f64
                                                - b[c] as f64 * b[3] as f64)
                                                / (255. * 255.))
                                                .abs()
                                        })
                                        .sum::<f64>()
                                })
                                .sum::<f64>()
                                / (total * 3.),
                        )
                    };
                    metrics.push(json!({"frame":frames[i],"time_seconds":frames[i]as f64/p.fps as f64,"mean_linear_luminance":luminance/total,"near_black_fraction":black as f64/total,"near_white_fraction":white as f64/total,"change_from_previous_sample":change}));
                    if white as f64 / total > 0.1 {
                        warnings.push(format!("Frame {}: more than 10% near-white pixels; inspect highlight detail and intended white areas.",frames[i]));
                    }
                    if black as f64 / total > 0.9 {
                        warnings.push(format!("Frame {}: more than 90% near-black pixels; inspect intended silhouette readability.",frames[i]));
                    }
                }
                let mut png = Cursor::new(vec![]);
                sheet.write_to(&mut png, image::ImageFormat::Png)?;
                Ok(
                    json!({"project":project,"revision":p.revision,"frames":frames,"sheet_columns":columns,"tile_width":w,"tile_height":h,"metrics":metrics,"warnings":warnings,"image_base64":STANDARD.encode(png.into_inner()),"note":"Tiles follow frames left-to-right, then top-to-bottom. Pixel diagnostics support visual review; they do not judge acting, composition, continuity or film quality. Geometric review_animation does not account for layer parallax."}),
                )
            }
        }
    }
    fn cinematic_plan(
        &self,
        p: Project,
        revision: u64,
        commands: Vec<Command>,
        frame: u32,
        apply: bool,
    ) -> Result<Value> {
        ensure!(p.revision == revision, "Stale revision");
        ensure!(frame < p.duration, "Preview frame outside project");
        let batch = Batch {
            project: p.id,
            expected_revision: revision,
            commands: commands.clone(),
        };
        let mut sandbox = Session::new(p.clone())?;
        sandbox.apply(batch.clone())?;
        let before = preview(&p, frame, 512)?;
        let after = preview(&sandbox.project, frame, 512)?;
        let mut out = json!({"project":p.id,"expected_revision":revision,"commands":commands,"applied":apply,"preview_frame":frame,"original_image_base64":STANDARD.encode(before),"image_base64":STANDARD.encode(after),"note":"Review the rendered proposal before committing. Finishing is linear-light SDR; parallax is a 2D multiplane camera approximation."});
        if apply {
            out["receipt"] = self.request(Request::ApplyCommands { batch })?;
        }
        Ok(out)
    }
}
