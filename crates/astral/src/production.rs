//! File-safe motion tool adapter. Editing proposals share the core command service.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::studio::*;
use astral_core::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
pub enum ProductionRequest {
    AutoAnimate {
        project: Id,
        expected_revision: u64,
        spec: astral_core::director::AutoSpec,
        #[serde(default)]
        apply: bool,
    },
    DrawFrame {
        project: Id,
        expected_revision: u64,
        layer: Id,
        frame: u32,
        strokes: Vec<InkStroke>,
    },
    Interactions {
        project: Id,
        expected_revision: u64,
        cues: Option<Vec<astral_core::interactions::InteractionCue>>,
    },
    Animate {
        project: Id,
        expected_revision: u64,
        character: Id,
        action: ActionKind,
        start: u32,
        end: u32,
        intensity: f32,
        distance: Option<f32>,
        direction: f32,
    },
    Storyboard {
        project: Id,
        expected_revision: u64,
        /// Omit to inspect the board and continuity warnings; provide to replace it atomically.
        board: Option<astral_core::storyboard::Storyboard>,
    },
    Review {
        project: Id,
        start: u32,
        end: u32,
        step: u32,
    },
    Edit {
        project: Id,
        expected_revision: u64,
        tool: StudioOperation,
        #[serde(default)]
        apply: bool,
        preview_frame: Option<u32>,
    },
    Waveform {
        project: Id,
        asset: Id,
        columns: usize,
    },
    ExportLibrary {
        project: Id,
        path: String,
        entries: Vec<Id>,
    },
    ImportLibrary {
        project: Id,
        expected_revision: u64,
        path: String,
    },
}
impl Service {
    pub fn production(&self, request: ProductionRequest) -> Result<Value> {
        match request {
            ProductionRequest::DrawFrame {
                project,
                expected_revision,
                layer,
                frame,
                strokes,
            } => self.request(Request::ApplyCommands {
                batch: Batch {
                    project,
                    expected_revision,
                    commands: vec![Command::SetFrameDrawing {
                        layer,
                        frame,
                        strokes,
                    }],
                },
            }),
            ProductionRequest::AutoAnimate {
                project,
                expected_revision,
                spec,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before auto animation"
                );
                let preview_frame = spec.start;
                let result = astral_core::director::plan(&p, spec)?;
                let mut value = serde_json::to_value(&result)?;
                value["project"] = json!(project);
                value["revision"] = json!(p.revision);
                value["preview_frame"] = json!(preview_frame);
                value["applied"] = json!(false);
                if apply && !result.plan.commands.is_empty() {
                    ensure!(
                        result.quality_gate_passed,
                        "Quality gate failed: inspect the non-mutating proposal and fix contacts/spacing/interactions first"
                    );
                    let receipt = self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands: result.plan.commands,
                        },
                    })?;
                    value["revision"] = receipt["revision"].clone();
                    value["applied"] = json!(true);
                    value["created_ids"] = receipt["created_ids"].clone();
                }
                Ok(value)
            }
            ProductionRequest::Interactions {
                project,
                expected_revision,
                cues,
            } => {
                let mut p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before editing interactions"
                );
                if let Some(cues) = cues {
                    self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands: vec![Command::SetSceneInteractions { cues }],
                        },
                    })?;
                    p = self.snapshot(project)?;
                }
                Ok(
                    json!({"project":project,"revision":p.revision,"cues":astral_core::interactions::read(&p)?}),
                )
            }
            ProductionRequest::Animate {
                project,
                expected_revision,
                character,
                action,
                start,
                end,
                intensity,
                distance,
                direction,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before animating"
                );
                let c = p.character(character).context("Character missing")?;
                let travel = distance.unwrap_or_else(|| {
                    astral_core::locomotion::natural_distance(c, action, start, end, p.fps)
                });
                let proposal = plan(
                    &p,
                    StudioOperation::Preset {
                        character,
                        action,
                        start,
                        end,
                        distance: travel,
                        intensity,
                        direction,
                    },
                )?;
                self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands: proposal.commands,
                    },
                })
            }

            ProductionRequest::Storyboard {
                project,
                expected_revision,
                board,
            } => {
                let mut p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before editing storyboard"
                );
                if let Some(board) = board {
                    self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands: vec![Command::SetStoryboard { board }],
                        },
                    })?;
                    p = self.snapshot(project)?;
                }
                let board = astral_core::storyboard::read(&p)?;
                Ok(
                    json!({"project": project, "revision": p.revision, "continuity": astral_core::storyboard::analyze(&board), "board": board}),
                )
            }
            ProductionRequest::Review {
                project,
                start,
                end,
                step,
            } => {
                let p = self.snapshot(project)?;
                let mut result =
                    serde_json::to_value(astral_core::staging::review(&p, start, end, step)?)?;
                result["text_review"] =
                    serde_json::to_value(astral_render::review_text(&p, start, end))?;
                Ok(result)
            }

            ProductionRequest::Edit {
                project,
                expected_revision,
                tool,
                apply,
                preview_frame,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect the document before editing"
                );
                let plan = if let StudioOperation::LayoutDialogue { dialogue } = tool {
                    let (dialogue, warnings) = astral_render::fit_and_place(&p, dialogue)?;
                    StudioPlan {
                        name: "Fit and place dialogue".into(),
                        commands: vec![Command::ReplaceDialogue { dialogue }],
                        warnings,
                        ..Default::default()
                    }
                } else {
                    plan(&p, tool)?
                };
                let mut value = json!({"project":project,"revision":p.revision,"plan":plan,"preview_frame":preview_frame});
                let mut preview = p.clone();
                if !plan.commands.is_empty() {
                    let mut session = Session::new(p.clone())?;
                    session.apply(Batch {
                        project,
                        expected_revision,
                        commands: plan.commands.clone(),
                    })?;
                    preview = session.project;
                }
                if let Some(frame) = preview_frame {
                    ensure!(
                        frame < preview.duration,
                        "Preview frame is outside the proposed timeline"
                    );
                    let bytes = astral_render::rasterize_framed(
                        &astral_render::display_list(&preview, frame as f64),
                        480,
                        270,
                        false,
                    )?
                    .encode_png()?;
                    value["image_base64"] = json!(STANDARD.encode(bytes));
                    let original = astral_render::rasterize_framed(
                        &astral_render::display_list(&p, (frame.min(p.duration - 1)) as f64),
                        480,
                        270,
                        false,
                    )?
                    .encode_png()?;
                    value["original_image_base64"] = json!(STANDARD.encode(original));
                }
                if apply && !plan.commands.is_empty() {
                    let receipt = self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands: plan.commands.clone(),
                        },
                    })?;
                    value["revision"] = receipt["revision"].clone();
                    value["receipt"] = receipt;
                }
                Ok(value)
            }
            ProductionRequest::Waveform {
                project,
                asset,
                columns,
            } => {
                ensure!(
                    (16..=2048).contains(&columns),
                    "Waveform supports 16–2048 columns"
                );
                let p = self.snapshot(project)?;
                let asset = p
                    .assets
                    .iter()
                    .find(|a| a.id == asset && a.kind == "audio")
                    .context("Audio asset not found")?;
                let path = self.project_workspace(project).path(&asset.path)?;
                ensure!(
                    std::fs::metadata(&path)?.len() <= 64 * 1024 * 1024,
                    "WAV exceeds 64 MiB"
                );
                let bytes = std::fs::read(path)?;
                let (duration, peaks) = astral_audio::analyze(&bytes, columns)?;
                Ok(json!({"asset":asset.id,"duration":duration,"peaks":peaks}))
            }
            ProductionRequest::ExportLibrary {
                project,
                path,
                entries,
            } => {
                ensure!(
                    path.ends_with(".astral-library.json"),
                    "Use the .astral-library.json extension"
                );
                let p = self.snapshot(project)?;
                let entries: Vec<_> = library_entries(&p)?
                    .into_iter()
                    .filter(|e| entries.is_empty() || entries.contains(&e.id))
                    .collect();
                ensure!(!entries.is_empty(), "Choose library items to export");
                let bytes = serde_json::to_vec_pretty(
                    &json!({"format":"astral_library","version":1,"entries":entries}),
                )?;
                ensure!(bytes.len() <= 4 * 1024 * 1024, "Library file exceeds 4 MiB");
                self.project_workspace(project)
                    .atomic_write(&path, &bytes)?;
                Ok(json!({"path":path,"entries":entries.len()}))
            }
            ProductionRequest::ImportLibrary {
                project,
                expected_revision,
                path,
            } => {
                let p = self.snapshot(project)?;
                ensure!(p.revision == expected_revision, "Stale revision");
                let source = self.project_workspace(project).path(path)?;
                ensure!(
                    std::fs::metadata(&source)?.len() <= 4 * 1024 * 1024,
                    "Library file exceeds 4 MiB"
                );
                let value: Value = serde_json::from_slice(&std::fs::read(source)?)?;
                ensure!(
                    value["format"] == "astral_library" && value["version"] == 1,
                    "Unsupported library format"
                );
                let mut imported: Vec<LibraryEntry> =
                    serde_json::from_value(value["entries"].clone())?;
                for entry in &mut imported {
                    entry.id = Id::new_v4();
                }
                let count = imported.len();
                let mut entries = library_entries(&p)?;
                entries.extend(imported);
                let receipt = self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands: vec![Command::SetLibrary { entries }],
                    },
                })?;
                Ok(json!({"imported":count,"revision":receipt["revision"],"receipt":receipt}))
            }
        }
    }
}
