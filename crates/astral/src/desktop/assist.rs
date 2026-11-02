use super::*;
use crate::{
    assistance::AssistRequest,
    voice::{Voice, VoiceOptions},
};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Task {
    Proposal,
    Voices,
    Job,
    Import,
    Recognize,
    Info,
}
struct Reply {
    task: Task,
    project: Id,
    value: anyhow::Result<Value>,
    preview: Option<(u32, u32, Vec<u8>)>,
}
pub(super) struct Review {
    project: Id,
    revision: u64,
    path: String,
    detection: astral_assist::recognition::Detection,
    texture: egui::TextureHandle,
    name: String,
    height: f32,
}
pub(super) struct Proposal {
    pub project: Id,
    pub revision: u64,
    pub frame: u32,
    pub layer: Option<Id>,
    pub ghost: Vec<InkStroke>,
    pub pose: Option<(Id, Pose)>,
    pub commands: Vec<Command>,
    pub label: String,
}
#[derive(Default)]
pub(super) struct AssistState {
    pub settings_open: bool,
    pub capture_binding: Option<String>,
    pub panel_open: bool,
    pub voice_open: bool,
    pub proposal: Option<Proposal>,
    pub examples: Vec<u32>,
    example_layer: Option<(Id, Id)>,
    pub deferred_image: Option<PathBuf>,
    pending: Option<std::sync::mpsc::Receiver<Reply>>,
    review: Option<Review>,
    voices: Vec<Voice>,
    voices_loaded: bool,
    job: Option<(Id, Id)>,
    model_report: Option<Value>,
    settings_tab: usize,
    voice_config: crate::voice_provider::Config,
    voice_key: String,
    range_start: u32,
    range_end: u32,
    overwrite: bool,
    voice_dialogue: bool,
    observed: Vec<String>,
}
impl AssistState {
    pub(super) fn new() -> Self {
        Self {
            voice_dialogue: true,
            voice_config: crate::voice_provider::load(),
            ..Default::default()
        }
    }
    pub(super) fn blocks_shortcuts(&self) -> bool {
        self.settings_open || self.voice_open || self.review.is_some() || self.panel_open
    }
}
impl Studio {
    fn assist_task(&mut self, request: AssistRequest, task: Task) {
        if self.assist.pending.is_some() {
            self.status = "Another analysis is running".into();
            return;
        }
        let service = self.service.clone();
        let project = self.project;
        let (send, receive) = std::sync::mpsc::channel();
        self.assist.pending = Some(receive);
        std::thread::spawn(move || {
            let value = service.assist(request);
            let _ = send.send(Reply {
                task,
                project,
                value,
                preview: None,
            });
        });
        self.context.request_repaint();
    }
    pub(super) fn suggest_after_stroke(&mut self) {
        self.assist.proposal = None;
        if !self.preferences.stroke_autocomplete && !self.preferences.frame_autocomplete {
            return;
        }
        let Ok(p) = self.service.snapshot(self.project) else {
            return;
        };
        let Some(layer) = self.selected_layer else {
            return;
        };
        if self.preferences.frame_autocomplete
            && self.assist.example_layer == Some((p.id, layer))
            && self.assist.examples.len() >= 3
            && !self.assist.examples.contains(&(self.frame as u32))
        {
            self.assist_task(
                AssistRequest::CompleteFrame {
                    project: p.id,
                    expected_revision: p.revision,
                    layer,
                    sources: self.assist.examples.clone(),
                    frame: self.frame as u32,
                    smoothing: self.learning_smoothing,
                    apply: false,
                },
                Task::Proposal,
            );
        } else if self.preferences.stroke_autocomplete
            && let Some(cel) = p.cel_at(layer, self.frame as u32)
            && let Some(stroke) = cel.strokes.last()
            && let Some(s) =
                astral_assist::drawing::suggest_stroke(stroke, self.preferences.completion_strength)
            && s.score >= self.preferences.suggestion_threshold
        {
            let mut strokes = cel.strokes.clone();
            *strokes.last_mut().unwrap() = s.stroke;
            self.assist.proposal = Some(Proposal {
                project: p.id,
                revision: p.revision,
                layer: Some(layer),
                frame: self.frame as u32,
                ghost: vec![s.ghost],
                pose: None,
                commands: vec![Command::SetCel {
                    layer,
                    frame: self.frame as u32,
                    strokes,
                }],
                label: s.label,
            });
        }
    }
    pub(super) fn accept_suggestion(&mut self) {
        if let Some(s) = self.assist.proposal.take() {
            if s.project != self.project || s.frame != self.frame as u32 {
                self.status =
                    "Return to the suggestion's project and frame before applying it".into();
                return;
            }
            match self.service.request(Request::ApplyCommands {
                batch: Batch {
                    project: s.project,
                    expected_revision: s.revision,
                    commands: s
                        .commands
                        .into_iter()
                        .map(|command| match command {
                            Command::SetCel {
                                layer,
                                frame,
                                strokes,
                            } => self.drawing_command(layer, frame, strokes),
                            other => other,
                        })
                        .collect(),
                },
            }) {
                Ok(_) => {
                    self.status = "Suggestion applied".into();
                    self.dirty = true;
                }
                Err(e) => self.status = format!("{e:#}"),
            }
            self.context.request_repaint();
        }
    }
    pub(super) fn assist_tick(&mut self, ctx: &egui::Context) {
        if self.assist.proposal.as_ref().is_some_and(|s| {
            s.project != self.project
                || s.frame != self.frame as u32
                || (s.pose.is_none() && s.layer != self.selected_layer)
                || !self
                    .service
                    .snapshot(s.project)
                    .is_ok_and(|p| p.revision == s.revision)
        }) {
            self.assist.proposal = None;
        }
        if !ctx.egui_wants_keyboard_input()
            && self.assist.proposal.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
        {
            self.accept_suggestion();
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.assist.proposal = None;
        }
        let reply = self.assist.pending.as_ref().and_then(|p| p.try_recv().ok());
        if let Some(reply) = reply {
            self.assist.pending = None;
            match reply.value {
                Err(e) => self.status = format!("{e:#}"),
                Ok(value) => match reply.task {
                    Task::Voices => {
                        self.assist.voices =
                            serde_json::from_value(value["voices"].clone()).unwrap_or_default();
                    }
                    Task::Info => self.assist.model_report = Some(value),
                    Task::Job => {
                        if let Ok(job) = serde_json::from_value(value["job"].clone()) {
                            self.assist.job = Some((job, reply.project));
                            self.status = "Background task queued".into();
                        }
                    }
                    Task::Proposal => {
                        if reply.project == self.project {
                            let commands: Vec<Command> =
                                serde_json::from_value(value["commands"].clone())
                                    .unwrap_or_default();
                            let ghost =
                                serde_json::from_value(value["ghost"].clone()).unwrap_or_default();
                            let pose = serde_json::from_value::<Id>(value["character"].clone())
                                .ok()
                                .zip(serde_json::from_value::<Pose>(value["pose"].clone()).ok());
                            if commands.is_empty() {
                                self.status = "No missing strokes to complete".into();
                                return;
                            }
                            self.assist.proposal = Some(Proposal {
                                project: self.project,
                                revision: value["revision"].as_u64().unwrap_or(0),
                                frame: value["frame"]
                                    .as_u64()
                                    .map_or(self.frame as u32, |v| v as u32),
                                layer: serde_json::from_value(value["layer"].clone()).ok(),
                                ghost,
                                pose,
                                commands,
                                label: value["label"].as_str().unwrap_or("Complete pose").into(),
                            });
                        }
                    }
                    Task::Recognize => {
                        if reply.project == self.project
                            && let Ok(detection) =
                                serde_json::from_value::<astral_assist::recognition::Detection>(
                                    value["detection"].clone(),
                                )
                            && let Some((width, height, rgba)) = reply.preview
                        {
                            let revision = self
                                .service
                                .snapshot(self.project)
                                .map_or(0, |p| p.revision);
                            let texture = ctx.load_texture(
                                "stickman-source",
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [width as usize, height as usize],
                                    &rgba,
                                ),
                                egui::TextureOptions::LINEAR,
                            );
                            self.assist.review = Some(Review {
                                project: self.project,
                                revision,
                                path: value["path"].as_str().unwrap_or("").into(),
                                detection,
                                texture,
                                name: "Imported character".into(),
                                height: self
                                    .service
                                    .snapshot(self.project)
                                    .map_or(300., |p| p.height as f32 * 0.55),
                            });
                        }
                    }
                    Task::Import => {
                        if reply.project == self.project {
                            self.selected = serde_json::from_value(value["character"].clone()).ok();
                            self.preferences.mode = WorkspaceMode::Rig;
                            self.preferences.advanced_timeline = true;
                            self.assist.review = None;
                            self.dirty = true;
                            self.status = "Editable rig imported".into();
                        }
                    }
                },
            }
            ctx.request_repaint();
        }
        let dropped = ctx.input_mut(|i| {
            let file = i.raw.dropped_files.first().map(|f| f.path().to_path_buf());
            i.raw.dropped_files.clear();
            file
        });
        if let Some(file) = dropped {
            if file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("astral"))
            {
                self.open_project_path(file);
            } else {
                self.begin_stickman_import(file);
            }
        }
        if self.assist.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    pub(super) fn choose_stickman(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .set_title("Import a stickman start image")
            .add_filter("Stickman image", &["png", "jpg", "jpeg"])
            .pick_file()
        {
            self.begin_stickman_import(file);
        }
    }
    pub(super) fn begin_stickman_import(&mut self, file: PathBuf) {
        if self.manager.home {
            self.new_project();
            self.manager.name = file
                .file_stem()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported animation".into());
            self.assist.deferred_image = Some(file);
            return;
        }
        if self.assist.pending.is_some() {
            self.status = "Wait for the current analysis to finish".into();
            return;
        }
        let service = self.service.clone();
        let project = self.project;
        let workspace = service.project_workspace(project);
        let (send, receive) = std::sync::mpsc::channel();
        self.assist.pending = Some(receive);
        self.status = "Mapping stickman image".into();
        std::thread::spawn(move || {
            let mut preview = None;
            let value = (|| -> anyhow::Result<Value> {
                anyhow::ensure!(
                    std::fs::metadata(&file)?.len() <= 64 * 1024 * 1024,
                    "Image exceeds 64 MiB"
                );
                let bytes = std::fs::read(&file)?;
                let original = astral_assist::recognition::decode(&bytes)?;
                let image = image::imageops::thumbnail(&original, 640, 640);
                preview = Some((image.width(), image.height(), image.into_raw()));
                let extension = file
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("png")
                    .to_ascii_lowercase();
                let path = format!("assets/reference/{}.{}", Id::new_v4(), extension);
                workspace.atomic_write(&path, &bytes)?;
                service.assist(AssistRequest::Recognize { project, path })
            })();
            let _ = send.send(Reply {
                task: Task::Recognize,
                project,
                value,
                preview,
            });
        });
        self.context.request_repaint();
    }
    pub(super) fn assist_windows(&mut self, ctx: &egui::Context, p: &Project) {
        if self.assist.settings_open {
            self.settings_window(ctx);
        }
        if self.assist.panel_open {
            self.assist_panel(ctx, p);
        }
        if self.assist.voice_open {
            self.voice_window(ctx, p);
        }
        if let Some(mut review) = self.assist.review.take() {
            let mut open = true;
            let mut import = false;
            egui::Window::new(i18n::tr("Review stickman joints"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(theme::dialog_size(ctx, Vec2::new(720., 560.)).x)
                .max_width(theme::dialog_size(ctx, Vec2::new(720., 560.)).x)
                .vscroll(true)
                .max_height(theme::dialog_size(ctx, Vec2::new(720., 560.)).y)
                .show(ctx, |ui| {
                    ui.label(i18n::tr(
                        "Drag the joints to correct the mapping before creating the rig.",
                    ));
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            let size = review.texture.size_vec2();
                            let factor = ((ui.available_width() - 280.).clamp(100., 420.) / size.x)
                                .min(390. / size.y);
                            let (rect, _) = ui.allocate_exact_size(size * factor, Sense::hover());
                            ui.painter().image(
                                review.texture.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                                Color32::WHITE,
                            );
                            let project = |point: Point| {
                                Pos2::new(
                                    rect.left()
                                        + point.x / review.detection.width as f32 * rect.width(),
                                    rect.top()
                                        + point.y / review.detection.height as f32 * rect.height(),
                                )
                            };
                            let rig =
                                stick_character("Mapping".into(), Point::default(), [0, 0, 0, 255]);
                            for (index, joint) in rig.joints.iter().enumerate() {
                                if let Some(parent) = joint.parent {
                                    ui.painter().line_segment(
                                        [
                                            project(review.detection.landmarks[index].position),
                                            project(review.detection.landmarks[parent].position),
                                        ],
                                        Stroke::new(1.5, Color32::from_rgb(69, 155, 211)),
                                    );
                                }
                            }
                            for (index, landmark) in
                                review.detection.landmarks.iter_mut().enumerate()
                            {
                                let at = project(landmark.position);
                                let response = ui.interact(
                                    Rect::from_center_size(at, Vec2::splat(16.)),
                                    ui.scope_id().with(index),
                                    Sense::drag(),
                                );
                                if response.dragged()
                                    && let Some(pos) = response.interact_pointer_pos()
                                {
                                    landmark.position = Point::new(
                                        ((pos.x - rect.left()) / rect.width()).clamp(0., 1.)
                                            * review.detection.width as f32,
                                        ((pos.y - rect.top()) / rect.height()).clamp(0., 1.)
                                            * review.detection.height as f32,
                                    );
                                    landmark.observed = true;
                                    landmark.score = 1.;
                                }
                                ui.painter().circle_filled(
                                    project(landmark.position),
                                    4.,
                                    if landmark.observed {
                                        Color32::from_rgb(48, 142, 198)
                                    } else {
                                        Color32::from_rgb(217, 142, 64)
                                    },
                                );
                                response.on_hover_text(&landmark.name);
                            }
                        });
                        ui.vertical(|ui| {
                            ui.set_width(240.);
                            ui.text_edit_singleline(&mut review.name);
                            ui.add(
                                egui::DragValue::new(&mut review.height)
                                    .range(20.0..=8000.)
                                    .prefix("Rig height "),
                            );
                            ui.label(format!(
                                "Mapping score {:.0}%",
                                review.detection.score * 100.
                            ));
                            ui.small(i18n::tr("A mapping score is not a calibrated probability."));
                            egui::ScrollArea::vertical()
                                .max_height(225.)
                                .show(ui, |ui| {
                                    for l in &mut review.detection.landmarks {
                                        ui.label(&l.name);
                                        ui.horizontal(|ui| {
                                            let x_changed = ui
                                                .add(
                                                    egui::DragValue::new(&mut l.position.x)
                                                        .speed(0.5)
                                                        .prefix("X "),
                                                )
                                                .changed();
                                            let y_changed = ui
                                                .add(
                                                    egui::DragValue::new(&mut l.position.y)
                                                        .speed(0.5)
                                                        .prefix("Y "),
                                                )
                                                .changed();
                                            if x_changed || y_changed {
                                                l.observed = true;
                                                l.score = 1.;
                                            }
                                        });
                                    }
                                });
                            if ui.button(i18n::tr("Fill uncertain joints")).clicked() {
                                let result = (|| -> anyhow::Result<()> {
                                    let d = &review.detection;
                                    let min = d
                                        .landmarks
                                        .iter()
                                        .map(|l| l.position.y)
                                        .fold(f32::INFINITY, f32::min);
                                    let max = d
                                        .landmarks
                                        .iter()
                                        .map(|l| l.position.y)
                                        .fold(f32::NEG_INFINITY, f32::max);
                                    let c = astral_assist::recognition::rig(
                                        d,
                                        "Mapping".into(),
                                        Point::default(),
                                        max - min,
                                        0,
                                    )?;
                                    let names: Vec<_> = d
                                        .landmarks
                                        .iter()
                                        .filter(|l| l.observed)
                                        .map(|l| l.name.clone())
                                        .collect();
                                    let model = astral_assist::network::PoseNetwork::bundled()?;
                                    let pose = astral_assist::network::complete_pose(
                                        &model,
                                        &c,
                                        &c.keys[0].pose,
                                        &names,
                                    )?;
                                    let origin = d.landmarks[0].position;
                                    for (i, l) in review.detection.landmarks.iter_mut().enumerate()
                                    {
                                        if !l.observed {
                                            l.position = pose.joints[i] + origin;
                                        }
                                    }
                                    Ok(())
                                })();
                                if let Err(e) = result {
                                    self.status = e.to_string();
                                }
                            }
                        });
                    });
                    for warning in &review.detection.warnings {
                        ui.small(warning);
                    }
                    if ui
                        .add_enabled(
                            self.project == review.project,
                            egui::Button::new(
                                RichText::new("Import editable rig").color(theme::ON_ACCENT),
                            )
                            .fill(theme::ACCENT),
                        )
                        .clicked()
                    {
                        import = true;
                    }
                    ui.label(RichText::new(&self.status).small().color(MUTED));
                });
            if import {
                self.assist_task(
                    AssistRequest::ImportRig {
                        project: review.project,
                        expected_revision: review.revision,
                        path: review.path.clone(),
                        detection: review.detection.clone(),
                        name: review.name.clone(),
                        center: Point::new(p.width as f32 / 2., p.height as f32 * 0.62),
                        height: review.height,
                        frame: self.frame as u32,
                    },
                    Task::Import,
                );
            }
            if open {
                self.assist.review = Some(review);
            }
        }
        if let Some((job, project)) = self.assist.job
            && let Ok(value) = self.service.request(Request::JobStatus { job })
        {
            egui::Window::new(i18n::tr("Assistance job"))
                .default_size(theme::dialog_size(ctx, Vec2::new(400., 260.)))
                .max_size(theme::dialog_size(ctx, Vec2::new(700., 600.)))
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.label(value["state"].as_str().unwrap_or("queued"));
                    ui.add(
                        egui::ProgressBar::new(value["progress"].as_f64().unwrap_or(0.) as f32)
                            .show_percentage(),
                    );
                    if let Some(error) = value["error"].as_str() {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    if let Some(path) = value["path"].as_str() {
                        ui.label(path);
                    }
                    if matches!(
                        value["state"].as_str(),
                        Some("completed" | "failed" | "cancelled")
                    ) {
                        if value["state"] == "completed" && project == self.project {
                            self.dirty = true;
                        }
                        if value["report"].is_object() {
                            if project == self.project
                                && ui.button(i18n::tr("Use this pose model")).clicked()
                                && let Some(path) = value["path"].as_str()
                            {
                                self.preferences.pose_model_path = path.into();
                            }
                            ui.collapsing(i18n::tr("Training report"), |ui| {
                                ui.monospace(
                                    serde_json::to_string_pretty(&value["report"])
                                        .unwrap_or_default(),
                                );
                            });
                        }
                        if ui.button(i18n::tr("Close")).clicked() {
                            self.assist.job = None;
                        }
                    } else {
                        if ui.button(i18n::tr("Cancel")).clicked() {
                            let _ = self.service.request(Request::CancelJob { job });
                        }
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                });
        }
    }
    fn assist_panel(&mut self, ctx: &egui::Context, p: &Project) {
        let mut open = true;
        egui::Window::new(i18n::tr("Drawing and frame assistance")).id(egui::Id::unique("astral.assist.v2")).open(&mut open).default_size(theme::dialog_size(ctx,Vec2::new(440.,520.))).max_size(theme::dialog_size(ctx,Vec2::new(700.,640.))).vscroll(true).show(ctx,|ui|{
            self.suggestion_controls(ui);ui.checkbox(&mut self.preferences.stroke_autocomplete,i18n::tr("Suggest stroke completion"));ui.checkbox(&mut self.preferences.frame_autocomplete,i18n::tr("Suggest missing strokes from selected examples"));
            ui.separator();theme::section(ui,"FRAME COMPLETION");ui.small(i18n::tr("Use 3–64 corresponding drawing keys, with matching stroke order, colors and widths."));
            if let Some(layer)=self.selected_layer{
                if self.assist.example_layer!=Some((p.id,layer)){self.assist.examples.clear();self.assist.example_layer=Some((p.id,layer));}
                ui.horizontal(|ui|{if ui.button(i18n::tr("Use current key as example")).clicked(){let frame=self.frame as u32;if p.cels.iter().any(|c|c.layer==layer&&c.frame==frame)&&!self.assist.examples.contains(&frame){self.assist.examples.push(frame);self.assist.examples.sort_unstable();self.assist.range_start = *self.assist.examples.first().unwrap();self.assist.range_end = *self.assist.examples.last().unwrap();}}
if ui.button(i18n::tr("Clear examples")).clicked(){self.assist.examples.clear();}});
                ui.label(format!("Example frames: {}",self.assist.examples.iter().map(|f|(f+1).to_string()).collect::<Vec<_>>().join(", ")));
                ui.horizontal(|ui|{ui.label(i18n::tr("Generate"));ui.add(egui::DragValue::new(&mut self.assist.range_start).range(0..=p.duration-1).custom_formatter(|v,_|format!("{}",v as u32+1)).custom_parser(|s|s.parse::<f64>().ok().map(|v|v-1.)));ui.label(i18n::tr("through"));ui.add(egui::DragValue::new(&mut self.assist.range_end).range(0..=p.duration-1).custom_formatter(|v,_|format!("{}",v as u32+1)).custom_parser(|s|s.parse::<f64>().ok().map(|v|v-1.)));});
                ui.checkbox(&mut self.assist.overwrite,i18n::tr("Replace existing keys (example keys are preserved)"));
                if ui.add_enabled(self.assist.examples.len()>=3,egui::Button::new("Suggest completion at playhead")).clicked(){self.assist_task(AssistRequest::CompleteFrame{project:p.id,expected_revision:p.revision,layer,sources:self.assist.examples.clone(),frame:self.frame as u32,smoothing:self.learning_smoothing,apply:false},Task::Proposal);}
                if ui.add_enabled(self.assist.examples.len()>=3,egui::Button::new("Generate editable inbetweens")).clicked(){self.assist_task(AssistRequest::GenerateFrames{project:p.id,expected_revision:p.revision,layer,sources:self.assist.examples.clone(),start:self.assist.range_start,end:self.assist.range_end,smoothing:self.learning_smoothing,overwrite:self.assist.overwrite},Task::Job);}
            }else{ui.label(i18n::tr("Select a drawing layer to use frame examples."));}
            if let Some(c)=self.selected.and_then(|id|p.character(id)){
                ui.separator();theme::section(ui,"POSE COMPLETION");ui.small(i18n::tr("Checked joints remain fixed. The model estimates the others."));
                if self.assist.observed.is_empty(){self.assist.observed=vec!["hips".into(),"chest".into(),"head".into()];}
                ui.horizontal_wrapped(|ui|{for joint in &c.joints{let mut known=self.assist.observed.contains(&joint.name);if ui.checkbox(&mut known,&joint.name).changed(){if known{self.assist.observed.push(joint.name.clone());}else{self.assist.observed.retain(|n|n!=&joint.name);}}}});
                if ui.button(i18n::tr("Suggest pose")).clicked(){self.assist_task(AssistRequest::CompletePose{project:p.id,expected_revision:p.revision,character:c.id,frame:self.frame as u32,observed:self.assist.observed.clone(),model_path:if self.preferences.pose_model_path.is_empty(){None}else{Some(self.preferences.pose_model_path.clone())},apply:false},Task::Proposal);}
            }
            ui.label(RichText::new(&self.status).small().color(MUTED));
        });
        self.assist.panel_open = open;
    }
    fn voice_window(&mut self, ctx: &egui::Context, p: &Project) {
        if !self.assist.voices_loaded
            && self.assist.pending.is_none()
            && self.assist.voice_config.provider == crate::voice_provider::Provider::Offline
        {
            self.assist.voices_loaded = true;
            self.assist_task(AssistRequest::ListVoices, Task::Voices);
        }
        let mut open = true;
        egui::Window::new(i18n::tr("Voice and dialogue")).id(egui::Id::unique("astral.voice.v2")).open(&mut open).default_size(theme::dialog_size(ctx,Vec2::new(460.,590.))).max_size(theme::dialog_size(ctx,Vec2::new(700.,700.))).vscroll(true).show(ctx,|ui|{
            ui.label(i18n::tr("Speaker"));ui.text_edit_singleline(&mut self.dialogue_speaker);ui.label(i18n::tr("Text"));ui.add(egui::TextEdit::multiline(&mut self.dialogue_text).desired_rows(5).desired_width(f32::INFINITY));
            self.voice_controls(ui);
            let provider_dirty=self.assist.voice_config!=crate::voice_provider::load()||!self.assist.voice_key.is_empty();
            if provider_dirty{ui.small("Save the provider settings before generating speech.");}
            ui.checkbox(&mut self.assist.voice_dialogue,i18n::tr("Add matching editable dialogue"));
            ui.small(i18n::tr("Generated speech becomes a separate editable WAV clip. Provider requests use the saved Voice settings."));
            if ui.add_enabled(!provider_dirty&&!self.dialogue_text.trim().is_empty()&&self.assist.pending.is_none(),egui::Button::new(RichText::new("Generate voice at playhead").color(theme::ON_ACCENT)).fill(theme::ACCENT)).clicked(){self.assist_task(AssistRequest::Voice{project:p.id,expected_revision:p.revision,text:self.dialogue_text.clone(),start:self.frame as u32,character:self.selected,options:self.voice_options(),dialogue:self.assist.voice_dialogue,formatting:self.dialogue_formatting.clone(),speaker:self.dialogue_speaker.clone()},Task::Job);}
            if ui.button(i18n::tr("Open text and caption tools")).clicked(){self.preferences.mode=WorkspaceMode::Dialogue;self.assist.voice_open=false;}
            ui.label(RichText::new(&self.status).small().color(MUTED));
        });
        self.assist.voice_open &= open;
    }
    fn voice_options(&self) -> VoiceOptions {
        let config = crate::voice_provider::load();
        VoiceOptions {
            provider: Some(config.provider),
            voice: if config.provider != crate::voice_provider::Provider::Offline
                || self.preferences.voice_id.is_empty()
            {
                None
            } else {
                Some(self.preferences.voice_id.clone())
            },
            rate: self.preferences.voice_rate,
            pitch: self.preferences.voice_pitch,
            volume: self.preferences.voice_volume,
        }
    }
    fn voice_controls(&mut self, ui: &mut egui::Ui) {
        use crate::voice_provider::{self, Provider};
        let previous = self.assist.voice_config.provider;
        egui::ComboBox::from_id_salt("voice.provider")
            .selected_text(previous.label())
            .show_ui(ui, |ui| {
                for provider in [
                    Provider::Offline,
                    Provider::ElevenLabs,
                    Provider::Compatible,
                ] {
                    ui.selectable_value(
                        &mut self.assist.voice_config.provider,
                        provider,
                        provider.label(),
                    );
                }
            });
        if previous != self.assist.voice_config.provider {
            self.assist.voice_key.clear();
            self.assist.voice_config.switch_from(previous);
        }
        if self.assist.voice_config.provider != Provider::Offline {
            ui.label(i18n::tr("Model ID"));
            ui.text_edit_singleline(&mut self.assist.voice_config.model);
            ui.label(i18n::tr("Voice ID"));
            ui.text_edit_singleline(&mut self.assist.voice_config.voice);
            if self.assist.voice_config.provider == Provider::Compatible {
                ui.label(i18n::tr("Speech endpoint"));
                ui.text_edit_singleline(&mut self.assist.voice_config.endpoint);
                ui.small(i18n::tr("OpenAI-compatible /audio/speech endpoint returning 16-bit WAV. Local HTTP servers are supported."));
            }
            ui.small(
                if voice_provider::has_key(
                    self.assist.voice_config.provider,
                    &self.assist.voice_config.endpoint,
                ) {
                    "Stored key available"
                } else if self.assist.voice_config.provider == Provider::Compatible {
                    "No stored key; local servers can work without one"
                } else {
                    "An API key is required"
                },
            );
            ui.label(i18n::tr("API key"));
            ui.add(
                egui::TextEdit::singleline(&mut self.assist.voice_key)
                    .password(true)
                    .hint_text("Leave blank to keep the stored key"),
            );
            ui.small(if cfg!(windows){"Keys are stored in Windows Credential Manager, outside your projects."}else{"Keys remain in this session. For persistence use ELEVENLABS_API_KEY or ASTRAL_VOICE_API_KEY."});
            ui.checkbox(
                &mut self.assist.voice_config.network_enabled,
                "Allow speech requests to the selected provider",
            );
            ui.small(i18n::tr("Generation sends your dialogue to this provider and may use paid credits. Local servers receive it locally. Astral never generates automatically."));
            if theme::button(ui, "Save provider", theme::Icon::Save, false).clicked() {
                let result = (|| -> anyhow::Result<()> {
                    voice_provider::save(&self.assist.voice_config)?;
                    if !self.assist.voice_key.is_empty() {
                        voice_provider::set_key(
                            self.assist.voice_config.provider,
                            &self.assist.voice_key,
                            &self.assist.voice_config.endpoint,
                        )?;
                    }
                    Ok(())
                })();
                self.assist.voice_key.clear();
                self.status = match result {
                    Ok(()) => "Voice provider saved".into(),
                    Err(e) => e.to_string(),
                };
            }
            if ui.button(i18n::tr("Remove stored API key")).clicked() {
                self.status = match voice_provider::set_key(
                    self.assist.voice_config.provider,
                    "",
                    &self.assist.voice_config.endpoint,
                ) {
                    Ok(()) => {
                        "Stored key removed; environment keys are managed outside Astral".into()
                    }
                    Err(e) => e.to_string(),
                };
            }
            theme::slider(
                ui,
                "Rate",
                egui::Slider::new(
                    &mut self.preferences.voice_rate,
                    if self.assist.voice_config.provider == Provider::ElevenLabs {
                        0.7..=1.2
                    } else {
                        0.5..=2.0
                    },
                ),
            );
            theme::slider(
                ui,
                "Volume",
                egui::Slider::new(&mut self.preferences.voice_volume, 0.0..=1.),
            );
            self.preferences.voice_pitch = 1.;
            return;
        }
        if voice_provider::load().provider != Provider::Offline
            && ui.button(i18n::tr("Use offline voice")).clicked()
        {
            self.status = match voice_provider::save(&self.assist.voice_config) {
                Ok(()) => "Offline voice selected".into(),
                Err(e) => e.to_string(),
            };
        }

        let selected = self
            .assist
            .voices
            .iter()
            .find(|v| v.id == self.preferences.voice_id)
            .map(|v| format!("{} ({})", v.name, v.language))
            .unwrap_or_else(|| "Default Windows voice".into());
        egui::ComboBox::from_id_salt("voice.selector")
            .selected_text(selected)
            .width(ui.available_width().min(350.))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.preferences.voice_id,
                    String::new(),
                    "Default Windows voice",
                );
                for voice in &self.assist.voices {
                    ui.selectable_value(
                        &mut self.preferences.voice_id,
                        voice.id.clone(),
                        format!("{} ({})", voice.name, voice.language),
                    );
                }
            });
        if theme::icon_button(ui, "Refresh installed voices", theme::Icon::Redo, false).clicked() {
            self.assist_task(AssistRequest::ListVoices, Task::Voices);
        }
        theme::slider(
            ui,
            "Rate",
            egui::Slider::new(&mut self.preferences.voice_rate, 0.5..=2.),
        );
        theme::slider(
            ui,
            "Pitch",
            egui::Slider::new(&mut self.preferences.voice_pitch, 0.5..=2.),
        );
        theme::slider(
            ui,
            "Volume",
            egui::Slider::new(&mut self.preferences.voice_volume, 0.0..=1.),
        );
    }
    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(i18n::tr("Settings")).id(egui::Id::unique("astral.settings.v2")).open(&mut open).default_size(theme::dialog_size(ctx,Vec2::new(620.,520.))).max_size(theme::dialog_size(ctx,Vec2::new(820.,680.))).vscroll(true).show(ctx,|ui|{
            ui.heading(i18n::tr("Make Astral yours"));ui.label(RichText::new("A workspace that fits the way you animate.").color(MUTED));ui.add_space(10.);ui.horizontal_wrapped(|ui|{for(index,label)in["Appearance","Drawing","Voice","Agents"].iter().enumerate(){let icon=match index{0=>theme::Icon::Settings,1=>theme::Icon::Pencil,2=>theme::Icon::Audio,_=>theme::Icon::Assist};if theme::button(ui,label,icon,self.assist.settings_tab==index).clicked(){self.assist.settings_tab=index;}}});ui.separator();
            match self.assist.settings_tab{
                0=>{
                    egui::ComboBox::from_id_salt("interface.language").selected_text(match self.preferences.language.as_str(){"es"=>"Español","fr"=>"Français",_=>"English"}).show_ui(ui,|ui|{for(code,name)in[("en","English"),("es","Español"),("fr","Français")]{ui.selectable_value(&mut self.preferences.language,code.into(),name);}});
                    ui.small("Core controls are localized; untranslated advanced controls and diagnostics use English.");
                    egui::Frame::new().fill(theme::CONTROL).corner_radius(12).inner_margin(14.).show(ui,|ui|{ui.set_width(ui.available_width());
                        ui.label(RichText::new("Colour & comfort").strong());ui.horizontal_wrapped(|ui|{for(label,color)in[("Ocean",[90,153,232,255]),("Iris",[167,139,250,255]),("Mint",[66,171,139,255]),("Amber",[222,160,70,255])]{if ui.add(egui::Button::new(RichText::new(label).color(theme::accent_foreground(rgba(color)))).fill(rgba(color))).clicked(){self.preferences.accent=color;}}});
                        ui.horizontal(|ui|{ui.label(i18n::tr("Accent"));ui.color_edit_button_srgba_unmultiplied(&mut self.preferences.accent);ui.label(i18n::tr("Canvas surround"));ui.color_edit_button_srgba_unmultiplied(&mut self.preferences.canvas_surround);});
                        ui.checkbox(&mut self.preferences.high_contrast,i18n::tr("Stronger contrast"));ui.checkbox(&mut self.preferences.compact_controls,i18n::tr("Compact controls"));ui.checkbox(&mut self.preferences.reduced_motion,i18n::tr("Reduced interface motion"));
                        theme::slider(ui,"Control corners",egui::Slider::new(&mut self.preferences.corners,0..=18));theme::slider(ui,"UI scale",egui::Slider::new(&mut self.preferences.scale,0.75..=2.));ui.checkbox(&mut self.preferences.fit_ui,i18n::tr("Fit interface to window"));
                    });ui.add_space(10.);
                    egui::Frame::new().fill(theme::CONTROL).corner_radius(12).inner_margin(14.).show(ui,|ui|{ui.set_width(ui.available_width());
                        ui.label(RichText::new("Workspace").strong());ui.horizontal_wrapped(|ui|{ui.checkbox(&mut self.preferences.left,i18n::tr("Tools"));ui.checkbox(&mut self.preferences.right,i18n::tr("Inspector"));ui.checkbox(&mut self.preferences.timeline,i18n::tr("Timeline"));});
                        ui.checkbox(&mut self.preferences.pro_mode,i18n::tr("Pro editing controls"));ui.checkbox(&mut self.preferences.smart_pose,i18n::tr("Smart posing"));ui.checkbox(&mut self.preferences.show_grid,i18n::tr("Canvas alignment grid (editor only)"));theme::slider(ui,"Frame card width",egui::Slider::new(&mut self.preferences.frame_cell_width,56.0..=128.0));theme::slider(ui,"Recovery interval",egui::Slider::new(&mut self.preferences.recovery_seconds,5..=120).suffix(" s"));
                    });ui.add_space(10.);
                    ui.collapsing(i18n::tr("Video encoder"),|ui|{ui.label(i18n::tr("Video and mixed-audio exports use a separately installed FFmpeg. PNG previews work without it."));ui.small(format!("Current: {}",crate::export::ffmpeg()));if ui.button(i18n::tr("Select FFmpeg executable")).clicked()&& let Some(path)=rfd::FileDialog::new().set_title("Choose installed FFmpeg").pick_file(){match crate::export::configure_encoder(&path){Ok(())=>self.status="Encoder configured".into(),Err(e)=>self.status=e.to_string()}}});
                    ui.collapsing(i18n::tr("Keyboard shortcuts"),|ui|self.shortcut_editor(ui));
                    ui.collapsing(i18n::tr("New project defaults"),|ui|{ui.horizontal_wrapped(|ui|{ui.add(egui::DragValue::new(&mut self.preferences.default_fps).range(1..=240).suffix(" fps"));ui.add(egui::DragValue::new(&mut self.preferences.default_width).range(1..=8192).prefix("Width "));ui.add(egui::DragValue::new(&mut self.preferences.default_height).range(1..=8192).prefix("Height "));});});
                    if ui.button(i18n::tr("Reset appearance")).clicked(){let d=Preferences::default();self.preferences.accent=d.accent;self.preferences.canvas_surround=d.canvas_surround;self.preferences.compact_controls=false;self.preferences.high_contrast=false;self.preferences.corners=9;self.preferences.scale=1.;self.preferences.fit_ui=true;self.preferences.left=true;self.preferences.right=true;self.preferences.timeline=true;}
                }
                1=>{ui.checkbox(&mut self.preferences.single_frame_drawing,i18n::tr("Draw on the current frame only"));ui.checkbox(&mut self.preferences.onion,i18n::tr("Onion skin"));ui.horizontal_wrapped(|ui|{ui.add(egui::DragValue::new(&mut self.preferences.onion_before).range(0..=6).prefix("Before "));ui.add(egui::DragValue::new(&mut self.preferences.onion_after).range(0..=6).prefix("After "));});theme::slider(ui,"Onion opacity",egui::Slider::new(&mut self.preferences.onion_opacity,0.05..=0.75));ui.separator();ui.checkbox(&mut self.preferences.stroke_autocomplete,i18n::tr("Suggest stroke completion"));ui.checkbox(&mut self.preferences.frame_autocomplete,i18n::tr("Suggest frame completion from selected examples"));theme::slider(ui,"Continuation length",egui::Slider::new(&mut self.preferences.completion_strength,0.1..=1.));theme::slider(ui,"Minimum fit score",egui::Slider::new(&mut self.preferences.suggestion_threshold,0.5..=1.));ui.small(i18n::tr("Suggestions are ghosts until accepted. Stroke fitting is geometric; learned frame trajectories use your examples."));ui.separator();ui.label(i18n::tr("Custom pose-model path (relative to the active project)"));ui.text_edit_singleline(&mut self.preferences.pose_model_path);ui.small(i18n::tr("Leave empty to use the bundled trained model."));if ui.button(i18n::tr("Model details")).clicked(){self.assist_task(AssistRequest::ModelInfo,Task::Info);}
if let Some(value)=&self.assist.model_report{ui.label(format!("{} parameters",value["parameters"]));ui.small(i18n::tr("Trained on synthetic stick poses. General image generation is not included."));ui.collapsing(i18n::tr("Evaluation"),|ui|{ui.monospace(serde_json::to_string_pretty(&value["report"]).unwrap_or_default());});}
if !self.manager.home&&ui.button(i18n::tr("Train a project pose model")).clicked()&&let Ok(p)=self.service.snapshot(self.project){self.assist_task(AssistRequest::TrainModel{project:p.id,expected_revision:p.revision,samples:8192,epochs:100},Task::Job);}}
                2=>self.voice_controls(ui),
                _=>{
                    ui.label(i18n::tr("MCP servers"));ui.small(i18n::tr("Both profiles connect to the running studio and share its documents."));
                    let executable=std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("astral.exe"))).unwrap_or_else(||PathBuf::from("astral.exe"));
                    let root=self.service.workspace.root().to_string_lossy().trim_start_matches(r"\\?\").to_string();
                    let config=json!({"mcpServers":{"astral-projects":{"command":executable,"args":["--workspace",root,"mcp","--profile","projects"]},"astral-animation":{"command":executable,"args":["--workspace",root,"mcp","--profile","animation"]}}});
                    let mut text=serde_json::to_string_pretty(&config).unwrap_or_default();if ui.button(i18n::tr("Copy MCP configuration")).clicked(){ctx.copy_text(text.clone());self.status="MCP configuration copied".into();}
                    ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_width(f32::INFINITY).desired_rows(12).interactive(false));
                    ui.small(i18n::tr("For one independent headless session, use: astral mcp --headless --profile all"));
                }
            }
            ui.separator();ui.small(i18n::tr("Settings are saved with the desktop preferences."));ui.small(format!("Astral {} · AI-assisted development · GPL-3.0-or-later",env!("CARGO_PKG_VERSION")));ui.hyperlink_to("Documentation & credits","https://github.com/nqtexd/astral#readme");
        });
        self.assist.settings_open = open;
    }
    pub(super) fn suggestion_controls(&mut self, ui: &mut egui::Ui) {
        let label = self.assist.proposal.as_ref().map(|p| p.label.clone());
        if let Some(label) = label {
            ui.horizontal(|ui| {
                ui.label(label);
                if ui
                    .add(theme::IconButton::new(
                        "Accept suggestion · Tab",
                        theme::Icon::Check,
                    ))
                    .clicked()
                {
                    self.accept_suggestion();
                }
                if ui
                    .add(theme::IconButton::new(
                        "Dismiss suggestion · Esc",
                        theme::Icon::Close,
                    ))
                    .clicked()
                {
                    self.assist.proposal = None;
                }
            });
        }
    }
    pub(super) fn paint_drawing_suggestion(
        &self,
        p: &Project,
        painter: &egui::Painter,
        to_screen: impl Fn(Point) -> Pos2,
        scale: f32,
    ) {
        if let Some(s) = &self.assist.proposal
            && s.project == p.id
            && s.revision == p.revision
            && s.frame == self.frame as u32
            && s.layer == self.selected_layer
        {
            for stroke in &s.ghost {
                painter.add(egui::Shape::line(
                    stroke.points.iter().map(|p| to_screen(*p)).collect(),
                    Stroke::new(
                        stroke.width * scale,
                        Color32::from_rgba_unmultiplied(60, 140, 190, 115),
                    ),
                ));
            }
        }
    }
    pub(super) fn paint_pose_suggestion(
        &self,
        p: &Project,
        painter: &egui::Painter,
        to_screen: impl Fn(Point) -> Pos2,
        scale: f32,
    ) {
        if let Some(s) = &self.assist.proposal
            && s.project == p.id
            && s.revision == p.revision
            && s.frame == self.frame as u32
            && let Some((id, pose)) = &s.pose
            && let Some(c) = p.character(*id)
        {
            for (i, j) in c.joints.iter().enumerate() {
                if let Some(parent) = j.parent {
                    painter.line_segment(
                        [
                            to_screen(pose.root + pose.joints[parent]),
                            to_screen(pose.root + pose.joints[i]),
                        ],
                        Stroke::new(
                            c.thickness * scale,
                            Color32::from_rgba_unmultiplied(60, 140, 190, 130),
                        ),
                    );
                }
            }
        }
    }
}
