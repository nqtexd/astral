use crate::export::{ExportSpec, Job};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use astral_storage::Workspace;
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Request {
    AssetTools {
        request: crate::assets::AssetRequest,
    },
    PreviewCommands {
        batch: Batch,
    },
    Artwork {
        request: crate::artwork::ArtworkRequest,
    },
    Production {
        request: crate::production::ProductionRequest,
    },
    CreateManagedProject {
        name: String,
        width: u32,
        height: u32,
        fps: u32,
        duration: u32,
        path: String,
        background: Option<Color>,
    },
    ListSavedProjects,
    ProjectActivity {
        project: Id,
    },
    RenameProject {
        project: Id,
        expected_revision: u64,
        name: String,
    },
    TrashProject {
        project: Id,
        expected_revision: u64,
    },
    RestoreProject {
        path: String,
    },
    CloseProject {
        project: Id,
        expected_revision: u64,
        #[serde(default)]
        discard: bool,
    },
    Assist {
        request: crate::assistance::AssistRequest,
    },
    ListProjects,
    CreateProject {
        name: String,
        width: u32,
        height: u32,
        fps: u32,
        duration: u32,
    },
    OpenProject {
        path: String,
    },
    RecoverProject {
        project: Id,
    },
    InspectProject {
        project: Id,
        frame: Option<f64>,
    },
    ApplyCommands {
        batch: Batch,
    },
    Undo {
        project: Id,
        expected_revision: u64,
    },
    Redo {
        project: Id,
        expected_revision: u64,
    },
    SaveProject {
        project: Id,
        path: String,
    },
    RenderPreview {
        project: Id,
        frame: f64,
        width: u32,
        height: u32,
        path: Option<String>,
    },
    ListPresets,
    ListExportPresets,
    ImportAsset {
        project: Id,
        expected_revision: u64,
        path: String,
        tags: Vec<String>,
    },
    ImportLegacy {
        path: String,
    },
    ExportOriginal {
        project: Id,
        path: String,
    },
    AssembleVideo {
        project: Id,
        spec: crate::export::AssemblySpec,
    },
    ExportVideo {
        project: Id,
        spec: ExportSpec,
    },
    ExportMedia {
        project: Id,
        spec: ExportSpec,
    },
    JobStatus {
        job: Id,
    },
    CancelJob {
        job: Id,
    },
    Waveform {
        path: String,
        bins: usize,
        #[serde(default)]
        project: Option<Id>,
    },
}

pub struct Store {
    pub sessions: BTreeMap<Id, Session>,
    pub jobs: BTreeMap<Id, Arc<Job>>,
    pub locations: BTreeMap<Id, Workspace>,
    pub files: BTreeMap<Id, PathBuf>,
    pub saved_revisions: BTreeMap<Id, u64>,
}
#[derive(Clone)]
pub struct Service {
    pub workspace: Workspace,
    pub store: Arc<Mutex<Store>>,
    pub application_root: PathBuf,
    pub(crate) activity_lock: Arc<Mutex<()>>,
}
impl Service {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            workspace: Workspace::new(root)?,
            activity_lock: Arc::new(Mutex::new(())),
            store: Arc::new(Mutex::new(Store {
                sessions: BTreeMap::new(),
                jobs: BTreeMap::new(),
                locations: BTreeMap::new(),
                files: BTreeMap::new(),
                saved_revisions: BTreeMap::new(),
            })),
            application_root: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .filter(|root| root.join("scripts/astral-codec.py").is_file())
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")),
        })
    }
    pub fn snapshot(&self, id: Id) -> Result<Project> {
        Ok(self
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Document service unavailable"))?
            .sessions
            .get(&id)
            .context("Project is not open")?
            .project
            .clone())
    }
    pub fn insert(&self, p: Project) -> Result<Id> {
        let id = p.id;
        let session = Session::new(p)?;
        let mut store = self
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
        ensure!(
            !store.sessions.contains_key(&id),
            "Project is already open; use its existing handle"
        );
        ensure!(
            store.sessions.len() < 16,
            "At most 16 projects may be open at once"
        );
        store.sessions.insert(id, session);
        Ok(id)
    }
    pub fn request(&self, request: Request) -> Result<Value> {
        match request {
            Request::Artwork { request } => self.artwork(request),
            Request::AssetTools { request } => self.asset_tools(request),
            Request::ProjectActivity { project } => Ok(
                json!({"project":project,"entries":self.project_activity(project)?,"note":"Local activity log: last 200 edits; not a version-control backup"}),
            ),
            Request::RenameProject {
                project,
                expected_revision,
                name,
            } => self.rename_local_project(project, expected_revision, name),
            Request::TrashProject {
                project,
                expected_revision,
            } => {
                let path = self.trash_local_project(project, expected_revision)?;
                let relative = path
                    .strip_prefix(self.workspace.root())
                    .ok()
                    .map(|p| p.to_string_lossy().replace('\\', "/"));
                Ok(
                    json!({"project":project,"trash_path":relative,"local_trash_path":path,"restore_in_desktop":relative.is_none(),"recoverable":true}),
                )
            }
            Request::RestoreProject { path } => {
                let id = self.restore_local_project(&self.workspace.path(path)?)?;
                Ok(json!({"project":id,"restored":true}))
            }

            Request::CreateManagedProject {
                name,
                width,
                height,
                fps,
                duration,
                path,
                background,
            } => {
                let file = self.workspace.path(&path)?;
                let mut p = Project::new(name, width, height, fps, duration);
                p.background = background.unwrap_or([250, 250, 250, 255]);
                let layer = DrawingLayer::new("Layer 1");
                p.cels.push(DrawingCel {
                    id: Id::new_v4(),
                    layer: layer.id,
                    frame: 0,
                    strokes: vec![],
                });
                p.layers.push(layer);
                p.schema = 3;
                p.validate()?;
                let id = self.insert(p)?;
                if let Err(e) = self.save_local_project_as(id, &file) {
                    let _ = self.close_local_project(id);
                    return Err(e);
                }
                let p = self.snapshot(id)?;
                Ok(
                    json!({"project":id,"revision":p.revision,"path":path,"file":self.project_file(id),"root":self.project_workspace(id).root(),"layer":p.layers[0].id}),
                )
            }
            Request::ListSavedProjects => self.saved_project_files(),
            Request::CloseProject {
                project,
                expected_revision,
                discard,
            } => {
                let mut store = self.store.lock().unwrap();
                let p = &store
                    .sessions
                    .get(&project)
                    .context("Project is not open")?
                    .project;
                ensure!(p.revision == expected_revision, "Stale revision");
                ensure!(
                    discard || store.saved_revisions.get(&project) == Some(&p.revision),
                    "Project has unsaved edits; save it or explicitly set discard=true"
                );
                store.sessions.remove(&project);
                store.locations.remove(&project);
                store.files.remove(&project);
                store.saved_revisions.remove(&project);
                Ok(json!({"closed":project}))
            }
            Request::Assist { request } => self.assist(request),
            Request::Production { request } => self.production(request),
            Request::ListProjects => {
                let store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                Ok(
                    json!({"projects":store.sessions.values().map(|s|json!({"id":s.project.id,"name":s.project.name,"revision":s.project.revision,"root":store.locations.get(&s.project.id).unwrap_or(&self.workspace).root(),"file":store.files.get(&s.project.id)})).collect::<Vec<_>>(),"workspace":self.workspace.root()}),
                )
            }
            Request::CreateProject {
                name,
                width,
                height,
                fps,
                duration,
            } => {
                let p = Project::new(name, width, height, fps, duration);
                p.validate()?;
                let value = summary(&p);
                self.insert(p)?;
                Ok(value)
            }
            Request::OpenProject { path } => {
                let id = self.open_local_project(&self.workspace.path(&path)?)?;
                Ok(summary(&self.snapshot(id)?))
            }
            Request::RecoverProject { project } => {
                let p = self.workspace.recover(project)?;
                let value = summary(&p);
                self.insert(p)?;
                Ok(value)
            }
            Request::InspectProject { project, frame } => {
                let p = self.snapshot(project)?;
                ensure!(frame.is_none_or(f64::is_finite), "Frame must be finite");
                let mut light = p.clone();
                light.original_source = None;
                if let Some(legacy) = light
                    .metadata
                    .get_mut("legacy")
                    .and_then(|v| v.as_object_mut())
                {
                    legacy.remove("source_base64");
                    legacy.remove("document");
                }
                for bitmap in &mut light.bitmaps {
                    bitmap.png = std::sync::Arc::from("");
                }
                let mut document = serde_json::to_value(&light)?;
                if let Some(bitmaps) = document["bitmaps"].as_array_mut() {
                    for bitmap in bitmaps {
                        let len = p
                            .bitmaps
                            .iter()
                            .find(|b| json!(b.id) == bitmap["id"])
                            .map_or(0, |b| b.png.len());
                        bitmap.as_object_mut().unwrap().remove("png");
                        bitmap["encoded_bytes"] = json!(len);
                    }
                }
                Ok(
                    json!({"document":document,"scene":frame.map(|f|evaluate(&p,f)),"capabilities":capabilities()}),
                )
            }
            Request::PreviewCommands { batch } => {
                let p = self.checked_project(batch.project, batch.expected_revision)?;
                let mut preview = Session::new(p)?;
                let receipt = preview.apply(batch)?;
                for asset in &preview.project.assets {
                    self.project_workspace(preview.project.id)
                        .path(&asset.path)?;
                }
                astral_render::validate_artwork(&preview.project)?;
                Ok(
                    json!({"valid":true,"mutated":false,"project":receipt.project,"expected_revision":receipt.revision-1,"proposed_revision":receipt.revision,"created_ids_are_preview_only":true,"characters":preview.project.characters.len(),"drawing_cels":preview.project.cels.len(),"graphics":preview.project.graphics.len(),"compositions":preview.project.compositions.len(),"audio_clips":preview.project.audio.len(),"warnings":["Structural transaction preview; inspect rendered animation before committing. Generated IDs may differ at commit."]}),
                )
            }
            Request::ApplyCommands { batch } => {
                // Decode new embedded images before touching the live history. A bad
                // payload must not leave an invalid committed project behind.
                if batch
                    .commands
                    .iter()
                    .any(|c| matches!(c, Command::AddBitmap { .. }))
                {
                    let mut candidate = Session::new(
                        self.checked_project(batch.project, batch.expected_revision)?,
                    )?;
                    candidate.apply(batch.clone())?;
                    astral_render::validate_artwork(&candidate.project)?;
                }
                for command in &batch.commands {
                    if let Command::ReplaceAsset { asset } | Command::AddAsset { asset } = command {
                        self.project_workspace(batch.project).path(&asset.path)?;
                    }
                }
                let id = batch.project;
                let (before, receipt, after) = {
                    let mut store = self
                        .store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                    let session = store.sessions.get_mut(&id).context("Project is not open")?;
                    let before = session.project.clone();
                    let receipt = session.apply(batch)?;
                    let after = session.project.clone();
                    (before, receipt, after)
                };
                let warning = self
                    .record_activity(&before, &after, "Edit")
                    .err()
                    .map(|e| e.to_string());
                let mut value = serde_json::to_value(receipt)?;
                if let Some(warning) = warning {
                    value["activity_warning"] = json!(warning);
                }
                Ok(value)
            }
            Request::Undo {
                project,
                expected_revision,
            }
            | Request::Redo {
                project,
                expected_revision,
            } => {
                let redo = matches!(request, Request::Redo { .. });
                let before = self.snapshot(project)?;
                let mut store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                let s = store
                    .sessions
                    .get_mut(&project)
                    .context("Project not open")?;
                let receipt = if redo {
                    s.redo(project, expected_revision)?
                } else {
                    s.undo(project, expected_revision)?
                };
                let after = s.project.clone();
                drop(store);
                let warning = self
                    .record_activity(&before, &after, if redo { "Redo" } else { "Undo" })
                    .err()
                    .map(|e| e.to_string());
                let mut value = serde_json::to_value(receipt)?;
                if let Some(warning) = warning {
                    value["activity_warning"] = json!(warning);
                }
                Ok(value)
            }
            Request::SaveProject { project, path } => {
                let p = self.snapshot(project)?;
                let workspace = self.project_workspace(project);
                workspace.save(&path, &p)?;
                self.store
                    .lock()
                    .unwrap()
                    .files
                    .insert(project, workspace.path(&path)?);
                self.store
                    .lock()
                    .unwrap()
                    .saved_revisions
                    .insert(project, p.revision);
                Ok(json!({"project":project,"revision":p.revision,"path":path}))
            }
            Request::RenderPreview {
                project,
                frame,
                width,
                height,
                path,
            } => {
                let p = self.snapshot(project)?;
                let bytes = astral_render::png(&p, frame, width, height)?;
                let review_frame = (frame.floor().max(0.) as u32).min(p.duration - 1);
                let layout_review = match composition::review(&p, review_frame, review_frame, 1) {
                    Ok(issues) => {
                        json!({"issues":issues,"note":"Conservative bounds checks; inspect the returned image"})
                    }
                    Err(e) => json!({"error":e.to_string()}),
                };
                let mut actual_path = path.clone();
                let mut warnings = vec![];
                if let Some(path) = &path {
                    ensure!(
                        Path::new(path).extension().is_some_and(|e| e == "png"),
                        "Preview output must be PNG"
                    );
                    let saved = self
                        .project_workspace(project)
                        .write_preview(path, &bytes)?;
                    actual_path = Some(saved.0);
                    warnings = saved.1;
                }
                Ok(
                    json!({"project":project,"revision":p.revision,"frame":frame,"mime_type":"image/png","image_base64":STANDARD.encode(bytes),"path":actual_path,"requested_path":path,"warnings":warnings,"text_review":astral_render::review_text(&p,review_frame,review_frame),"layout_review":layout_review}),
                )
            }
            Request::ListPresets => Ok(
                json!({"actions":ActionKind::ALL,"effects":EffectKind::ALL,"graphics":GraphicKind::ALL,"dialogue_styles":["speech","thought","caption"],"learning":{"method":"RBF kernel ridge regression","minimum_examples":3,"maximum_examples":64,"editable_strokes":true,"editable_poses":true},"capabilities":capabilities()}),
            ),
            Request::ImportAsset {
                project,
                expected_revision,
                path,
                tags,
            } => {
                let absolute = self.project_workspace(project).path(&path)?;
                ensure!(absolute.is_file(), "Asset does not exist");
                let name = absolute
                    .file_name()
                    .context("Missing asset filename")?
                    .to_string_lossy()
                    .to_string();
                let extension = absolute
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                ensure!(
                    [
                        "wav", "aif", "aiff", "mp3", "m4a", "png", "jpg", "jpeg", "webp", "bmp",
                        "gif"
                    ]
                    .contains(&extension.as_str()),
                    "Supported assets: WAV, AIFF, MP3, M4A, PNG, JPEG, WebP, BMP, GIF"
                );
                let asset = Asset {
                    id: Id::new_v4(),
                    name,
                    path,
                    kind: if ["wav", "aif", "aiff", "mp3", "m4a"].contains(&extension.as_str()) {
                        "audio"
                    } else {
                        "image"
                    }
                    .into(),
                    tags,
                };
                self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands: vec![Command::AddAsset { asset }],
                    },
                })
            }
            Request::ImportLegacy { path } => {
                let (p, warnings) =
                    astral_codecs::import_legacy(&self.workspace, &path, &self.application_root)?;
                let mut value = summary(&p);
                self.insert(p)?;
                value["warnings"] = json!(warnings);
                Ok(value)
            }
            Request::ExportOriginal { project, path } => {
                let p = self.snapshot(project)?;
                let bytes = astral_codecs::export_original(&p)?;
                self.project_workspace(project)
                    .atomic_write(&path, &bytes)?;
                Ok(
                    json!({"path":path,"warning":"Archived original only; does not contain Astral edits"}),
                )
            }
            Request::AssembleVideo { project, spec } => {
                let p = self.snapshot(project)?;
                let workspace = self.project_workspace(project);
                let mut store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                ensure!(
                    store.jobs.values().filter(|j| !j.finished()).count() < 2,
                    "Two exports are already active"
                );
                if store.jobs.len() >= 32
                    && let Some(id) = store
                        .jobs
                        .iter()
                        .find(|(_, j)| j.finished())
                        .map(|(id, _)| *id)
                {
                    store.jobs.remove(&id);
                }
                let job = crate::export::assemble(workspace, project, p.revision, spec)?;
                let id = job.id;
                store.jobs.insert(id, job);
                Ok(json!({"job":id}))
            }
            Request::ListExportPresets => Ok(crate::export::catalog()),
            Request::ExportVideo { project, spec } | Request::ExportMedia { project, spec } => {
                let p = self.snapshot(project)?;
                let workspace = self.project_workspace(project);
                workspace.path(&spec.path)?;
                let mut store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                ensure!(
                    store.jobs.values().filter(|j| !j.finished()).count() < 2,
                    "Two exports are already active"
                );
                if store.jobs.len() >= 32
                    && let Some(id) = store
                        .jobs
                        .iter()
                        .find(|(_, j)| j.finished())
                        .map(|(id, _)| *id)
                {
                    store.jobs.remove(&id);
                }
                let job = crate::export::start(workspace, p, spec)?;
                let id = job.id;
                store.jobs.insert(id, job);
                Ok(json!({"job":id}))
            }
            Request::JobStatus { job } => {
                let store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                let job = store.jobs.get(&job).context("Unknown job")?;
                Ok(job.status())
            }
            Request::CancelJob { job } => {
                let store = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                let job = store.jobs.get(&job).context("Unknown job")?;
                job.cancel();
                Ok(job.status())
            }
            Request::Waveform {
                path,
                bins,
                project,
            } => {
                let workspace = project
                    .map(|id| self.project_workspace(id))
                    .unwrap_or_else(|| self.workspace.clone());
                let bytes = workspace.read(&path)?;
                Ok(json!({"peaks":astral_audio::waveform(&bytes,bins)?}))
            }
        }
    }
    pub fn demo(&self) -> Result<Id> {
        let mut p = motion_lab_demo()?;
        let impact_path = format!("assets/astral-demo-{}/impact.wav", p.id);
        self.workspace
            .atomic_write(&impact_path, &astral_audio::impact_wav()?)?;
        let asset = Asset {
            id: Id::new_v4(),
            name: "Impact".into(),
            path: impact_path,
            kind: "audio".into(),
            tags: vec!["impact".into(), "demo".into()],
        };
        for frame in [104, 134, 224] {
            p.audio.push(AudioClip {
                speed: 1.,
                id: Id::new_v4(),
                name: "Impact".into(),
                asset: asset.id,
                start: frame,
                offset_seconds: 0.,
                duration_seconds: 0.25,
                gain: 0.6,
                fade_in: 0.002,
                fade_out: 0.04,
                looping: false,
            });
        }
        p.assets.push(asset);
        self.insert(p)
    }
}
pub fn summary(p: &Project) -> Value {
    json!({"project":p.id,"revision":p.revision,"name":p.name,"width":p.width,"height":p.height,"fps":p.fps,"duration":p.duration})
}
pub fn capabilities() -> Value {
    json!({"schema":11,"curve_tracks":"root_joint_XY_Hermite_tangents","live_symbols":"shared_library_nested_8_levels","asset_packages":"hash_verified_astralpack","audio_waveform_editing":"trim_slip_split_seek","command_preview":true,"saved_version_backups":5,"compositing":"editable_DAG_blend_mask_gradient_blur_color_sRGB","native_export_formats":["avi","png","jpeg","png_sequence","wav"],"voice_providers":["offline","elevenlabs","compatible"],"vector_brushes":"pressure_cubic_nodes_fills_marker_dashed_stipple_procedural_texture_custom_presets","deformation":"translation_or_MLS_similarity_cubic_curvers_bone_correctives","single_frame_drawing":true,"offline_director":"seeded_stickman_spar_inspect_bounded_length_repairs_grounded_continuation","embedded_creative_model":false,"native_rigs":true,"motion_tools":"constrained_pose_styles_fight_builder_motion_brush_heuristic_coach","library":"editable_characters_motions_symbols_combos","multi_frame_editing":true,"timeline_markers":true,"rig_templates":["human","quadruped","winged"],"editable_action_clips":true,"learned_inbetweens":"offline_RBF_kernel_ridge_from_3_to_64_authored_examples","drawing_inbetweens":"corresponding_vector_strokes","dialogues":"vector_text_speech_thought_caption_typewriter","motion_graphics":"keyed_position_scale_rotation_opacity_Bezier_draw_on","clouds":true,"contacts":"two_bone_ik","preview":"shared_display_list","legacy_import":"source_evaluated_native_cels_rigs_images_audio_with_archived_original_and_reported_fallback","legacy_native_editing":"sampled_source_tweens_joined_geometry_custom_shapes_native_skin_bindings","image_asset_rendering":true,"raster_artwork":"embedded_PNG_JPEG_PSD_groups_masks_supported_blends_basic_text_sequences_per_frame_paint_mesh_pins","lip_sync":"offline_English_or_phonetic_recognized_visemes_custom_mouth_artwork_or_explicit_envelope_mode","tracking":"bounded_local_template_camera_or_object_translation_two_point_rotation_scale_proposals","microphone":"user_initiated_60_seconds","audio":"WAV_AIFF_MP3_M4A_native_decode_pitch_speed_streamed_export_mix","glow_blur":"stylized_overlays_not_filters","export":"native_AVI_MJPEG_PCM_images_sequences_WAV_or_FFmpeg_MP4_WebM_GIF_CPU_or_wgpu_rendering"})
}
