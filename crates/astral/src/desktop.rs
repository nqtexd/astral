use crate::service::{Request, Service};
use astral_core::*;
use astral_render::{Primitive, display_list};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};

const VIOLET: Color32 = theme::ACCENT;
const MUTED: Color32 = theme::MUTED;
mod artwork_panel;
mod assist;
mod export_panel;
mod features;
mod frame_editor;
mod motion;
mod project_manager;
mod storyboard;
mod theme;

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum WorkspaceMode {
    Storyboard,
    Artwork,
    Animate,
    Rig,
    Draw,
    Effects,
    Audio,
    Graphics,
    Dialogue,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Preferences {
    accent: Color,
    canvas_surround: Color,
    compact_controls: bool,
    high_contrast: bool,
    corners: u8,
    show_grid: bool,
    frame_cell_width: f32,
    recovery_seconds: u64,
    pro_mode: bool,
    smart_pose: bool,
    movement_style: astral_core::studio::MovementStyle,
    smart_onion: bool,
    ghost_path: bool,
    mode: WorkspaceMode,
    onion: bool,
    scale: f32,
    reduced_motion: bool,
    fit_ui: bool,
    left: bool,
    right: bool,
    timeline: bool,
    advanced_timeline: bool,
    onion_before: u32,
    onion_after: u32,
    onion_opacity: f32,
    stroke_autocomplete: bool,
    single_frame_drawing: bool,
    frame_autocomplete: bool,
    completion_strength: f32,
    suggestion_threshold: f32,
    pose_model_path: String,
    voice_id: String,
    voice_rate: f64,
    voice_pitch: f64,
    voice_volume: f64,
    default_fps: u32,
    default_width: u32,
    default_height: u32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            accent: [90, 153, 232, 255],
            canvas_surround: [13, 15, 23, 255],
            compact_controls: false,
            high_contrast: false,
            corners: 9,
            show_grid: false,
            frame_cell_width: 86.,
            recovery_seconds: 10,
            pro_mode: false,
            smart_pose: true,
            movement_style: astral_core::studio::MovementStyle::Natural,
            smart_onion: false,
            ghost_path: false,
            mode: WorkspaceMode::Draw,
            onion: false,
            scale: 1.,
            reduced_motion: false,
            fit_ui: true,
            left: true,
            right: true,
            timeline: true,
            advanced_timeline: false,
            onion_before: 1,
            onion_after: 1,
            onion_opacity: 0.22,
            stroke_autocomplete: true,
            single_frame_drawing: true,
            frame_autocomplete: false,
            completion_strength: 0.5,
            suggestion_threshold: 0.8,
            pose_model_path: String::new(),
            voice_id: String::new(),
            voice_rate: 1.,
            voice_pitch: 1.,
            voice_volume: 1.,
            default_fps: 24,
            default_width: 1920,
            default_height: 1080,
        }
    }
}

pub struct Studio {
    service: Service,
    context: egui::Context,
    manager: project_manager::ProjectManager,
    assist: assist::AssistState,
    artwork: artwork_panel::ArtworkState,
    motion: motion::MotionState,
    board: storyboard::BoardState,
    dialogue_formatting: TextFormatting,
    text_font_size: f32,
    selected_layer: Option<Id>,
    layer_rename: String,
    layer_opacity: Option<(Id, f32)>,
    draw_tool: frame_editor::DrawTool,
    frame_clipboard: Option<Vec<InkStroke>>,
    frame_cache: BTreeMap<(Id, u64, u32), egui::TextureHandle>,
    follow_frame: bool,
    hold_frames: u32,
    view_zoom: f32,
    view_pan: Vec2,
    cel_work: Option<Vec<InkStroke>>,
    cel_base: Vec<InkStroke>,
    cel_origin: Option<Point>,
    drag_cel: Option<(Id, u32, f32)>,
    project: Id,
    selected: Option<Id>,
    selected_action: Option<Id>,
    preferences: Preferences,
    frame: f64,
    playing: bool,
    last_tick: Instant,
    status: String,
    save_path: String,
    saved_revision: Option<u64>,
    last_recovery: Instant,
    action: ActionKind,
    effect: EffectKind,
    action_length: u32,
    action_distance: f32,
    natural_pace: bool,
    search: String,
    palette: bool,
    stroke: Vec<Point>,
    drag_pose: Option<Pose>,
    drag_joint: Option<usize>,
    drag_origin: Option<Pos2>,
    export_job: Option<Id>,
    export_panel: export_panel::ExportPanel,
    audio_sink: Option<rodio::Sink>,
    audio_stream: Option<rodio::OutputStream>,
    pending_audio: Option<std::sync::mpsc::Receiver<anyhow::Result<Vec<u8>>>>,
    draw_color: Color32,
    draw_width: f32,
    dirty: bool,
    confirm_close: bool,
    documents: BTreeMap<Id, (String, Option<u64>)>,
    exit_confirmed: bool,
    recovery_busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    learning_drawings: Vec<Id>,
    learning_smoothing: f64,
    dialogue_text: String,
    dialogue_speaker: String,
    dialogue_style: DialogueStyle,
    dialogue_speed: f32,
    feature_duration: u32,
    selected_dialogue: Option<Id>,
    graphic_kind: GraphicKind,
    graphic_preset: features::GraphicPreset,
    graphic_text: String,
    selected_graphic: Option<Id>,
    edit_stroke: bool,
    selected_stroke: Option<Id>,
}
impl Studio {
    fn new(cc: &eframe::CreationContext<'_>, service: Service, project: Id) -> Self {
        let preferences = cc
            .storage
            .and_then(|s| eframe::get_value(s, "astral.layout.v1"))
            .unwrap_or_default();
        Self::from_context(cc.egui_ctx.clone(), service, project, preferences)
    }
    fn from_context(
        context: egui::Context,
        service: Service,
        project: Id,
        preferences: Preferences,
    ) -> Self {
        theme::install(&context);
        let manager = project_manager::ProjectManager::new(service.workspace.root(), project);
        Self {
            manager,
            assist: assist::AssistState::new(),
            artwork: Default::default(),
            motion: motion::MotionState::default(),
            board: storyboard::BoardState::default(),
            dialogue_formatting: Default::default(),
            text_font_size: 30.,
            selected_layer: None,
            layer_rename: String::new(),
            layer_opacity: None,
            draw_tool: Default::default(),
            frame_clipboard: None,
            frame_cache: BTreeMap::new(),
            follow_frame: true,
            hold_frames: 2,
            view_zoom: 1.,
            view_pan: Vec2::ZERO,
            cel_work: None,
            cel_base: vec![],
            cel_origin: None,
            drag_cel: None,
            context,
            service,
            project,
            selected: None,
            selected_action: None,
            preferences,
            frame: 0.,
            playing: false,
            last_tick: Instant::now(),
            status: "Ready".into(),
            save_path: String::new(),
            saved_revision: None,
            last_recovery: Instant::now(),
            action: ActionKind::Walk,
            effect: EffectKind::Impact,
            action_length: 30,
            action_distance: 120.,
            natural_pace: true,
            search: String::new(),
            palette: false,
            stroke: vec![],
            drag_pose: None,
            drag_joint: None,
            drag_origin: None,
            export_job: None,
            export_panel: Default::default(),
            audio_sink: None,
            audio_stream: None,
            pending_audio: None,
            draw_color: Color32::from_rgb(30, 32, 37),
            draw_width: 5.,
            dirty: false,
            confirm_close: false,
            documents: BTreeMap::new(),
            exit_confirmed: false,
            recovery_busy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            learning_drawings: vec![],
            learning_smoothing: 0.0001,
            dialogue_text: String::new(),
            dialogue_speaker: String::new(),
            dialogue_style: DialogueStyle::Speech,
            dialogue_speed: 30.,
            feature_duration: 90,
            selected_dialogue: None,
            graphic_kind: GraphicKind::Cloud,
            graphic_preset: features::GraphicPreset::Drift,
            graphic_text: "Title".into(),
            selected_graphic: None,
            edit_stroke: false,
            selected_stroke: None,
        }
    }
    fn remember_document(&mut self) {
        self.documents
            .insert(self.project, (self.save_path.clone(), self.saved_revision));
    }
    fn switch_project(&mut self, id: Id) {
        if !self.flush_storyboard() {
            return;
        }
        if id == self.project {
            return;
        }
        self.remember_document();
        self.assist.proposal = None;
        self.assist.examples.clear();
        self.stop();
        self.stroke.clear();
        self.cel_work = None;
        self.cel_origin = None;
        self.frame_cache.clear();
        self.view_zoom = 1.;
        self.view_pan = Vec2::ZERO;
        if let Some(placeholder) = self.manager.placeholder
            && placeholder != id
            && self
                .service
                .snapshot(placeholder)
                .is_ok_and(|p| p.revision == 0)
        {
            let _ = self.service.close_local_project(placeholder);
            self.documents.remove(&placeholder);
            self.manager.placeholder = None;
        }
        self.selected_layer = self
            .service
            .snapshot(id)
            .ok()
            .and_then(|p| p.layers.last().map(|l| l.id));
        self.layer_rename = self
            .service
            .snapshot(id)
            .ok()
            .and_then(|p| p.layers.last().map(|l| l.name.clone()))
            .unwrap_or_default();
        if let Ok(p) = self.service.snapshot(id) {
            if !p.layers.is_empty() {
                self.preferences.mode = WorkspaceMode::Draw;
                self.preferences.advanced_timeline = false;
            } else if !p.characters.is_empty() {
                self.preferences.mode = WorkspaceMode::Animate;
                self.preferences.advanced_timeline = true;
            }
        }
        self.export_panel.open = false;
        self.project = id;
        self.selected = None;
        self.selected_action = None;
        self.frame = 0.;
        let (path, saved) = self.documents.get(&id).cloned().unwrap_or_else(|| {
            let store = self.service.store.lock().unwrap();
            let workspace = store.locations.get(&id).unwrap_or(&self.service.workspace);
            let path = store
                .files
                .get(&id)
                .and_then(|p| p.strip_prefix(workspace.root()).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("{id}.astral"));
            (path, store.saved_revisions.get(&id).copied())
        });
        self.save_path = path;
        self.saved_revision = saved;
    }
    fn has_unsaved_projects(&self) -> bool {
        if self.board.dirty {
            return true;
        }
        self.service
            .store
            .lock()
            .unwrap()
            .sessions
            .values()
            .any(|s| {
                if Some(s.project.id) == self.manager.placeholder && s.project.revision == 0 {
                    return false;
                }
                let saved = if s.project.id == self.project {
                    self.saved_revision
                } else {
                    self.documents.get(&s.project.id).and_then(|v| v.1)
                };
                saved != Some(s.project.revision)
            })
    }
    fn save_all(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.flush_storyboard(),
            "Resolve the storyboard draft before saving"
        );
        self.remember_document();
        let ids: Vec<_> = self
            .service
            .store
            .lock()
            .unwrap()
            .sessions
            .keys()
            .copied()
            .collect();
        let mut paths = std::collections::BTreeSet::new();
        let mut tasks = vec![];
        for id in ids {
            if Some(id) == self.manager.placeholder
                && self.service.snapshot(id).is_ok_and(|p| p.revision == 0)
            {
                continue;
            }
            let path = self
                .documents
                .get(&id)
                .map(|v| v.0.clone())
                .unwrap_or_else(|| format!("{id}.astral"));
            anyhow::ensure!(
                paths.insert(
                    self.service
                        .project_workspace(id)
                        .root()
                        .join(&path)
                        .to_string_lossy()
                        .to_lowercase()
                ),
                "Two open projects have the same save path; choose separate names first"
            );
            tasks.push((id, path));
        }
        for (id, path) in tasks {
            let value = self.service.request(Request::SaveProject {
                project: id,
                path: path.clone(),
            })?;
            let revision = value["revision"].as_u64();
            self.documents.insert(id, (path, revision));
            if id == self.project {
                self.saved_revision = revision;
                self.dirty = false;
            }
        }
        Ok(())
    }
    fn apply(&mut self, commands: Vec<Command>) {
        if let Ok(p) = self.service.snapshot(self.project) {
            match self.service.request(Request::ApplyCommands {
                batch: Batch {
                    project: self.project,
                    expected_revision: p.revision,
                    commands,
                },
            }) {
                Ok(_) => {
                    self.dirty = true;
                    self.status = "Changes applied".into();
                    self.context.request_repaint();
                }
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }
    fn history(&mut self, redo: bool) {
        if let Ok(p) = self.service.snapshot(self.project) {
            let req = if redo {
                Request::Redo {
                    project: self.project,
                    expected_revision: p.revision,
                }
            } else {
                Request::Undo {
                    project: self.project,
                    expected_revision: p.revision,
                }
            };
            match self.service.request(req) {
                Ok(_) => {
                    self.dirty = true;
                    self.context.request_repaint();
                }
                Err(e) => self.status = e.to_string(),
            }
        }
    }
    fn save_project(&mut self) {
        if !self.flush_storyboard() {
            return;
        }
        if self.manager.home && Some(self.project) == self.manager.placeholder {
            return;
        }
        if self.save_path.is_empty() || self.service.project_file(self.project).is_none() {
            self.save_project_as();
            return;
        }
        match self.service.request(Request::SaveProject {
            project: self.project,
            path: self.save_path.clone(),
        }) {
            Ok(v) => {
                self.saved_revision = v["revision"].as_u64();
                self.dirty = false;
                self.remember_document();
                self.status = "Project saved".into();
                if let (Ok(p), Some(file)) = (
                    self.service.snapshot(self.project),
                    self.service.project_file(self.project),
                ) {
                    self.manager.record(&p, file);
                }
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn new_project(&mut self) {
        self.stop();
        self.manager.create = true;
        self.manager.name = "Untitled".into();
        self.manager.set_defaults(
            self.preferences.default_width,
            self.preferences.default_height,
            self.preferences.default_fps,
        );
    }
    fn open_project(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .set_title("Open project")
            .set_directory(self.service.workspace.root())
            .add_filter("Astral project", &["astral"])
            .pick_file()
        {
            self.open_project_path(file);
        }
    }
    fn add_character(&mut self) {
        let p = self.service.snapshot(self.project).unwrap();
        self.apply(vec![Command::AddCharacter {
            name: format!("Character {}", p.characters.len() + 1),
            position: Point::new(p.width as f32 * 0.5, p.height as f32 * 0.65),
            color: [167, 139, 250, 255],
        }]);
    }
    fn add_action(&mut self) {
        if let Some(id) = self.selected {
            let p = self.service.snapshot(self.project).unwrap();
            let start = self.frame as u32;
            let end = (start + self.action_length.max(2) - 1).min(p.duration - 1);
            let clip = ActionClip {
                id: Id::new_v4(),
                character: id,
                kind: self.action,
                start,
                end,
                intensity: 1.,
                distance: if self.natural_pace {
                    p.character(id).map_or(self.action_distance, |c| {
                        astral_core::locomotion::natural_distance(c, self.action, start, end, p.fps)
                    })
                } else {
                    self.action_distance
                },
                direction: 1.,
                markers: BTreeMap::from([("accent".into(), (start + end) / 2)]),
            };
            self.apply(vec![Command::ApplyAction { clip }]);
        } else {
            self.status = "Select a character before adding an action".into()
        }
    }
    fn add_effect(&mut self) {
        let p = self.service.snapshot(self.project).unwrap();
        let start = self.frame as u32;
        self.apply(vec![Command::AddEffect {
            effect: Effect {
                id: Id::new_v4(),
                name: format!("{:?}", self.effect),
                kind: self.effect,
                start,
                end: (start + 20).min(p.duration - 1),
                position: if self.selected.is_some() {
                    Point::new(0., -30.)
                } else {
                    Point::new(p.width as f32 / 2., p.height as f32 / 2.)
                },
                character: self.selected,
                joint: None,
                color: [255, 211, 131, 255],
                intensity: 1.,
                seed: 42,
            },
        }]);
    }
    fn import_audio(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("PCM WAV", &["wav"])
            .pick_file()
        {
            let result = (|| -> anyhow::Result<()> {
                let name = file.file_name().unwrap().to_string_lossy().to_string();
                anyhow::ensure!(
                    std::fs::metadata(&file)?.len() <= 64 * 1024 * 1024,
                    "Audio file exceeds 64 MiB"
                );
                let bytes = std::fs::read(file)?;
                let _ = astral_audio::waveform(&bytes, 16)?;
                let path = format!("assets/{}-{name}", Id::new_v4());
                self.service
                    .project_workspace(self.project)
                    .atomic_write(&path, &bytes)?;
                let id = Id::new_v4();
                self.apply(vec![
                    Command::AddAsset {
                        asset: Asset {
                            id,
                            name: name.clone(),
                            path,
                            kind: "audio".into(),
                            tags: vec![],
                        },
                    },
                    Command::AddAudio {
                        clip: AudioClip {
                            id: Id::new_v4(),
                            name,
                            asset: id,
                            start: self.frame as u32,
                            offset_seconds: 0.,
                            duration_seconds: 1.,
                            gain: 0.8,
                            fade_in: 0.01,
                            fade_out: 0.04,
                            looping: false,
                        },
                    },
                ]);
                Ok(())
            })();
            if let Err(e) = result {
                self.status = e.to_string()
            }
        }
    }
    fn start_export(&mut self) {
        self.stop();
        if let Ok(p) = self.service.snapshot(self.project) {
            self.export_panel.open = true;
            self.export_panel.open(&p, self.frame, &self.context);
        }
    }
    fn stop(&mut self) {
        self.playing = false;
        self.pending_audio = None;
        if let Some(s) = self.audio_sink.take() {
            s.stop();
        }
        self.audio_stream = None;
    }
    fn toggle_play(&mut self) {
        if self.playing {
            self.stop();
            return;
        }
        self.playing = true;
        self.last_tick = Instant::now();
        if let Ok(p) = self.service.snapshot(self.project)
            && !p.audio.is_empty()
        {
            let workspace = self.service.project_workspace(self.project);
            let (send, receive) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = send.send(astral_audio::mix(&p, &workspace));
            });
            self.pending_audio = Some(receive);
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (save, undo, redo, palette, play) = shortcut_flags(ctx);
        if save {
            self.save_project()
        }
        if undo {
            self.history(false)
        }
        if redo {
            self.history(true)
        }
        if palette {
            self.palette = !self.palette
        }
        if play {
            self.toggle_play()
        }
    }
    fn header(&mut self, ui: &mut egui::Ui, p: &Project) {
        egui::Panel::top("editor.header")
            .exact_size(50.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("Astral").size(19.).strong());
                    if theme::icon_button(ui, "Project library", theme::Icon::Layers, false)
                        .clicked()
                    {
                        self.stop();
                        self.manager.home = true;
                    }
                    theme::icon_menu(ui, "File", theme::Icon::More, |ui| {
                        if ui.button("New project    Ctrl+N").clicked() {
                            self.new_project();
                            ui.close();
                        }
                        if ui.button("Open project    Ctrl+O").clicked() {
                            self.open_project();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Save    Ctrl+S").clicked() {
                            self.save_project();
                            ui.close();
                        }
                        if ui.button("Save as    Ctrl+Shift+S").clicked() {
                            self.save_project_as();
                            ui.close();
                        }
                        if ui.button("Close project    Ctrl+W").clicked() {
                            self.request_close_project(self.project);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Storyboard").clicked() {
                            self.stop();
                            self.preferences.mode = WorkspaceMode::Storyboard;
                            ui.close();
                        }
                        if ui.button("Project settings").clicked() {
                            self.open_project_settings(p);
                            ui.close();
                        }
                    });
                    theme::icon_menu(ui, "Add", theme::Icon::Plus, |ui| {
                        if theme::button(ui, "Image to rig", theme::Icon::Image, false).clicked() {
                            self.choose_stickman();
                            ui.close();
                        }
                        if theme::button(ui, "Voice", theme::Icon::Audio, false).clicked() {
                            self.assist.voice_open = true;
                            ui.close();
                        }
                        if theme::button(ui, "Text", theme::Icon::Text, false).clicked() {
                            self.preferences.mode = WorkspaceMode::Dialogue;
                            ui.close();
                        }
                        if theme::button(ui, "Drawing layer", theme::Icon::Layers, false).clicked()
                        {
                            self.add_drawing_layer();
                            ui.close();
                        }
                    });
                    ui.add_space(10.);
                    let title_width = (ui.available_width() - 210.).max(24.);
                    ui.add_sized(
                        [title_width, 30.],
                        egui::Label::new(RichText::new(&p.name).strong()).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, "Export", theme::Icon::Export, true).clicked() {
                            self.start_export();
                        }
                        if theme::icon_button(ui, "Settings", theme::Icon::Settings, false)
                            .clicked()
                        {
                            self.assist.settings_open = true;
                        }
                        if theme::icon_button(ui, "Save · Ctrl+S", theme::Icon::Save, false)
                            .clicked()
                        {
                            self.save_project();
                        }
                    });
                });
            });
        egui::Panel::top("editor.documents")
            .exact_size(34.)
            .show(ui, |ui| {
                let tabs: Vec<_> = self
                    .service
                    .store
                    .lock()
                    .unwrap()
                    .sessions
                    .values()
                    .filter(|s| {
                        Some(s.project.id) != self.manager.placeholder || s.project.revision > 0
                    })
                    .map(|s| (s.project.id, s.project.name.clone(), s.project.revision))
                    .collect();
                egui::ScrollArea::horizontal()
                    .id_salt("document.tabs")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (id, name, revision) in tabs {
                                let saved = if id == self.project {
                                    self.saved_revision
                                } else {
                                    self.documents.get(&id).and_then(|v| v.1)
                                };
                                let label = if saved == Some(revision) {
                                    name
                                } else {
                                    format!("{name} ·")
                                };
                                if ui.selectable_label(id == self.project, label).clicked() {
                                    self.switch_project(id);
                                }
                                if ui
                                    .add(
                                        theme::IconButton::new("Close project", theme::Icon::Close)
                                            .compact(),
                                    )
                                    .clicked()
                                {
                                    self.request_close_project(id);
                                }
                                ui.separator();
                            }
                        });
                    });
            });
        egui::Panel::top("editor.tools")
            .exact_size(44.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    egui::ComboBox::from_id_salt("editor.mode")
                        .selected_text(match self.preferences.mode {
                            WorkspaceMode::Storyboard => "Storyboard",
                            WorkspaceMode::Artwork => "Artwork",
                            WorkspaceMode::Draw => "Draw",
                            WorkspaceMode::Animate => "Animate",
                            WorkspaceMode::Rig => "Rig",
                            WorkspaceMode::Effects => "Effects",
                            WorkspaceMode::Audio => "Audio",
                            WorkspaceMode::Graphics => "Graphics",
                            WorkspaceMode::Dialogue => "Dialogue",
                        })
                        .width(90.)
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (WorkspaceMode::Storyboard, "Storyboard"),
                                (WorkspaceMode::Artwork, "Artwork"),
                                (WorkspaceMode::Draw, "Draw"),
                                (WorkspaceMode::Animate, "Animate"),
                                (WorkspaceMode::Rig, "Rig"),
                                (WorkspaceMode::Effects, "Effects"),
                                (WorkspaceMode::Audio, "Audio"),
                                (WorkspaceMode::Graphics, "Graphics"),
                                (WorkspaceMode::Dialogue, "Dialogue"),
                            ] {
                                ui.selectable_value(&mut self.preferences.mode, mode, label);
                            }
                        });
                    if self.preferences.mode == WorkspaceMode::Draw {
                        for (tool, label, icon) in [
                            (
                                frame_editor::DrawTool::Pencil,
                                "Pencil · B",
                                theme::Icon::Pencil,
                            ),
                            (
                                frame_editor::DrawTool::Eraser,
                                "Eraser · E",
                                theme::Icon::Eraser,
                            ),
                            (
                                frame_editor::DrawTool::Move,
                                "Move drawing · G",
                                theme::Icon::Move,
                            ),
                            (
                                frame_editor::DrawTool::Hand,
                                "Pan canvas · H",
                                theme::Icon::Hand,
                            ),
                        ] {
                            if theme::icon_button(ui, label, icon, self.draw_tool == tool).clicked()
                            {
                                self.draw_tool = tool;
                                self.stroke.clear();
                                self.cel_work = None;
                            }
                        }
                    } else if theme::icon_button(ui, "Add character", theme::Icon::Character, false)
                        .clicked()
                    {
                        self.add_character();
                    }
                    ui.separator();
                    if theme::icon_button(ui, "Undo · Ctrl+Z", theme::Icon::Undo, false).clicked()
                    {
                        self.history(false);
                    }
                    if theme::icon_button(ui, "Redo · Ctrl+Shift+Z", theme::Icon::Redo, false)
                        .clicked()
                    {
                        self.history(true);
                    }
                    theme::icon_menu(ui, "View options", theme::Icon::Sliders, |ui| {
                        ui.checkbox(&mut self.preferences.left, "Layers and scene");
                        ui.checkbox(&mut self.preferences.right, "Inspector");
                        ui.checkbox(&mut self.preferences.timeline, "Timeline");
                        ui.checkbox(&mut self.preferences.advanced_timeline, "Detailed timeline");
                        ui.checkbox(&mut self.preferences.pro_mode, "Pro controls");
                        ui.checkbox(&mut self.preferences.onion, "Onion skin");
                        ui.checkbox(&mut self.preferences.reduced_motion, "Reduced motion");
                        theme::slider(
                            ui,
                            "UI scale",
                            egui::Slider::new(&mut self.preferences.scale, 0.75..=2.),
                        );
                    });
                    if theme::icon_button(
                        ui,
                        "Motion studio",
                        theme::Icon::Assist,
                        self.motion.open,
                    )
                    .clicked()
                    {
                        self.motion.open = true;
                    }
                    if theme::icon_button(ui, "Command search · Ctrl+P", theme::Icon::Search, false)
                        .clicked()
                    {
                        self.palette = true;
                    }
                });
            });
    }
    fn scene_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.left {
            return;
        }
        egui::Panel::left("astral.scene")
            .default_size(225.)
            .min_size(170.)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("scene.panel")
                    .show(ui, |ui| {
                        ui.add_space(12.);
                        ui.heading("Scene");

                        ui.add_space(10.);
                        let tabs: Vec<_> = self
                            .service
                            .store
                            .lock()
                            .unwrap()
                            .sessions
                            .values()
                            .map(|s| (s.project.id, s.project.name.clone()))
                            .collect();
                        egui::ComboBox::from_id_salt("projects")
                            .selected_text(&p.name)
                            .width(ui.available_width() - 15.)
                            .show_ui(ui, |ui| {
                                for (id, name) in tabs {
                                    if ui.selectable_label(self.project == id, name).clicked() {
                                        self.switch_project(id);
                                    }
                                }
                            });
                        ui.separator();
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Filter scene…")
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(7.);
                        egui::ScrollArea::vertical()
                            .id_salt("cast")
                            .max_height(260.)
                            .show(ui, |ui| {
                                for c in &p.characters {
                                    if !c.name.to_lowercase().contains(&self.search.to_lowercase())
                                    {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        let mut visible = c.visible;
                                        if ui.checkbox(&mut visible, "").changed() {
                                            self.apply(vec![Command::SetCharacter {
                                                character: c.id,
                                                name: None,
                                                color: None,
                                                visible: Some(visible),
                                                locked: None,
                                                secondary_motion: None,
                                            }]);
                                        }
                                        let color = Color32::from_rgba_unmultiplied(
                                            c.color[0], c.color[1], c.color[2], c.color[3],
                                        );
                                        let (mark, _) =
                                            ui.allocate_exact_size(Vec2::splat(9.), Sense::hover());
                                        ui.painter().rect_filled(mark, 2., color);
                                        if ui
                                            .selectable_label(self.selected == Some(c.id), &c.name)
                                            .clicked()
                                        {
                                            self.selected = Some(c.id);
                                            self.selected_action = None;
                                        }
                                    });
                                }
                            });
                        ui.add_space(15.);
                        ui.separator();
                        ui.add_space(10.);
                        ui.heading("Motion library");
                        ui.label(
                            RichText::new("Editable actions, ready to direct.")
                                .size(11.)
                                .color(MUTED),
                        );
                        ui.add_space(8.);
                        egui::Grid::new("actions")
                            .num_columns(2)
                            .spacing([6., 6.])
                            .show(ui, |ui| {
                                for (i, kind) in ActionKind::ALL.into_iter().enumerate() {
                                    if ui
                                        .selectable_label(self.action == kind, format!("{kind:?}"))
                                        .clicked()
                                    {
                                        self.action = kind;
                                    }
                                    if i % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                        ui.add_space(8.);
                        ui.add(
                            egui::DragValue::new(&mut self.action_length)
                                .range(2..=3000)
                                .prefix("Frames  "),
                        );
                        ui.checkbox(&mut self.natural_pace,"Natural pace").on_hover_text("Match walk/run travel to character size, duration and frame rate. Disable for manual travel.");
                        ui.add_enabled(!self.natural_pace,
                            egui::DragValue::new(&mut self.action_distance)
                                .speed(1.)
                                .prefix("Travel  "),
                        );
                        if ui
                            .add_enabled(
                                self.selected.is_some(),
                                egui::Button::new("Apply action →"),
                            )
                            .clicked()
                        {
                            self.add_action()
                        }
                        ui.add_space(20.);
                        ui.separator();
                        ui.label(RichText::new("AGENT READY").size(10.).color(VIOLET));
                        ui.label(
                    RichText::new(
                        "Connect your coding agent through Astral MCP. Every edit stays yours.",
                    )
                    .size(12.)
                    .color(MUTED),
                );
                        ui.label(RichText::new("astral mcp").monospace().size(11.));
                    });
            });
    }
    fn inspector(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.right {
            return;
        }
        egui::Panel::right("astral.inspector")
            .default_size(265.)
            .min_size(210.)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(12.);
                ui.heading("Inspector");
                ui.label(RichText::new("REFINE THE DETAILS").size(10.).color(MUTED));
                ui.add_space(15.);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.preferences.mode == WorkspaceMode::Artwork {
                        self.artwork_panel(ui, p);
                        return;
                    }
                    if self.preferences.mode == WorkspaceMode::Audio {
                        self.voice_artwork_panel(ui, p);
                    }
                    if !self.preferences.pro_mode
                        && matches!(
                            self.preferences.mode,
                            WorkspaceMode::Animate
                                | WorkspaceMode::Rig
                                | WorkspaceMode::Audio
                                | WorkspaceMode::Effects
                        )
                    {
                        self.quick_motion_inspector(ui, p);
                        return;
                    }
                    self.feature_panel(ui, p);
                    if let Some(c) = self.selected.and_then(|id| p.character(id)) {
                        ui.label(RichText::new(&c.name).size(19.).color(VIOLET));
                        let mut locked = c.locked;
                        if ui.checkbox(&mut locked, "Lock character").changed() {
                            self.apply(vec![Command::SetCharacter {
                                character: c.id,
                                name: None,
                                color: None,
                                visible: None,
                                locked: Some(locked),
                                secondary_motion: None,
                            }]);
                        }
                        let mut color = c.color;
                        if ui
                            .color_edit_button_srgba_unmultiplied(&mut color)
                            .changed()
                        {
                            self.apply(vec![Command::SetCharacter {
                                character: c.id,
                                name: None,
                                color: Some(color),
                                visible: None,
                                locked: None,
                                secondary_motion: None,
                            }]);
                        }
                        let mut secondary = c.secondary_motion;
                        if theme::slider(
                            ui,
                            "Secondary motion",
                            egui::Slider::new(&mut secondary, 0.0..=10.),
                        )
                        .changed()
                        {
                            self.apply(vec![Command::SetCharacter {
                                character: c.id,
                                name: None,
                                color: None,
                                visible: None,
                                locked: None,
                                secondary_motion: Some(secondary),
                            }]);
                        }
                        if theme::icon_button(ui, "Save current pose", theme::Icon::Save, false)
                            .clicked()
                        {
                            self.apply(vec![Command::SavePose {
                                character: c.id,
                                frame: self.frame as u32,
                                name: format!("{} pose {}", c.name, p.poses.len() + 1),
                            }]);
                        }
                        if self.preferences.mode == WorkspaceMode::Rig {
                            ui.label("Drag a joint on the stage to pose it.");
                            if ui.button("Pin feet at current frame").clicked() {
                                let evaluated = evaluate(p, self.frame)
                                    .characters
                                    .into_iter()
                                    .find(|v| v.id == c.id)
                                    .unwrap();
                                let mut commands = vec![];
                                for (i, j) in c.joints.iter().enumerate() {
                                    if j.name.ends_with("foot") {
                                        commands.push(Command::SetContact {
                                            character: c.id,
                                            contact: Contact {
                                                joint: i,
                                                start: self.frame as u32,
                                                end: (self.frame as u32 + 29).min(p.duration - 1),
                                                target: evaluated.pose.root
                                                    + evaluated.pose.joints[i],
                                            },
                                        });
                                    }
                                }
                                if !commands.is_empty() {
                                    self.apply(commands)
                                }
                            }
                            if ui.button("Release contacts").clicked() {
                                self.apply(vec![Command::ClearContacts { character: c.id }]);
                            }
                        }
                        for pose in &p.poses {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(&pose.name);
                                if ui.small_button("Apply").clicked() {
                                    self.apply(vec![Command::ApplyPose {
                                        character: c.id,
                                        pose: pose.id,
                                        frame: self.frame as u32,
                                        blend: 1.,
                                        joints: vec![],
                                        mirror: false,
                                    }]);
                                }
                                if ui.small_button("Mirror").clicked() {
                                    self.apply(vec![Command::ApplyPose {
                                        character: c.id,
                                        pose: pose.id,
                                        frame: self.frame as u32,
                                        blend: 1.,
                                        joints: vec![],
                                        mirror: true,
                                    }]);
                                }
                            });
                        }
                    } else {
                        ui.label(RichText::new("Select a character").size(18.));
                        ui.label(
                            RichText::new("Shape a pose, apply an action, or add an effect.")
                                .color(MUTED),
                        );
                    }
                    if let Some(clip) = self
                        .selected_action
                        .and_then(|id| p.actions.iter().find(|a| a.id == id))
                    {
                        ui.separator();
                        ui.heading(format!("{:?} action", clip.kind));
                        let mut next = clip.clone();
                        let mut changed = false;
                        changed |= theme::slider(
                            ui,
                            "Intensity",
                            egui::Slider::new(&mut next.intensity, 0.0..=3.),
                        )
                        .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut next.distance).prefix("Travel  "))
                            .changed();
                        changed |= theme::slider(
                            ui,
                            "Direction",
                            egui::Slider::new(&mut next.direction, -1.0..=1.),
                        )
                        .changed();
                        if changed {
                            self.apply(vec![Command::ReplaceAction { clip: next }]);
                        }
                        if theme::icon_button(ui, "Delete action", theme::Icon::Delete, false)
                            .clicked()
                        {
                            self.apply(vec![Command::Remove { id: clip.id }]);
                            self.selected_action = None;
                        }
                    }
                    ui.add_space(15.);
                    ui.separator();
                    ui.add_space(8.);
                    if self.preferences.mode == WorkspaceMode::Draw {
                        ui.heading("Brush");
                        ui.color_edit_button_srgba(&mut self.draw_color);
                        theme::slider(
                            ui,
                            "Width",
                            egui::Slider::new(&mut self.draw_width, 1.0..=40.),
                        );
                        ui.label("Draw directly on the stage. Each stroke is one undo step.");
                    }
                    ui.heading("Effects");
                    egui::ComboBox::from_id_salt("effects")
                        .selected_text(format!("{:?}", self.effect))
                        .show_ui(ui, |ui| {
                            for kind in EffectKind::ALL {
                                ui.selectable_value(&mut self.effect, kind, format!("{kind:?}"));
                            }
                        });
                    if theme::icon_button(ui, "Add effect at playhead", theme::Icon::Plus, false)
                        .clicked()
                    {
                        self.add_effect()
                    }
                    ui.add_space(12.);
                    ui.heading("Audio");
                    if ui.button("Import WAV").clicked() {
                        self.import_audio()
                    }
                    for a in &p.audio {
                        ui.label(format!("♫ {} | frame {}", a.name, a.start));
                    }
                    ui.add_space(12.);
                    ui.heading("Project");
                    ui.label("Save path (inside workspace)");
                    ui.text_edit_singleline(&mut self.save_path);
                    ui.label(
                        RichText::new(
                            self.service
                                .project_workspace(self.project)
                                .root()
                                .display()
                                .to_string(),
                        )
                        .size(10.)
                        .color(MUTED),
                    );
                });
            });
    }
    fn timeline(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.timeline {
            return;
        }
        egui::Panel::bottom("astral.timeline")
            .default_size(220.)
            .min_size(120.)
            .resizable(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if theme::icon_button(
                        ui,
                        if self.playing {
                            "Pause · Space"
                        } else {
                            "Play · Space"
                        },
                        if self.playing {
                            theme::Icon::Pause
                        } else {
                            theme::Icon::Play
                        },
                        self.playing,
                    )
                    .clicked()
                    {
                        self.toggle_play();
                    }
                    if theme::icon_button(ui, "Go to start", theme::Icon::Start, false).clicked() {
                        self.stop();
                        self.frame = 0.;
                    }
                    ui.label(
                        RichText::new(format!(
                            "{:02}:{:02}:{:02}",
                            self.frame as u32 / p.fps / 60,
                            self.frame as u32 / p.fps % 60,
                            self.frame as u32 % p.fps
                        ))
                        .monospace()
                        .color(VIOLET),
                    );
                    ui.separator();

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Click an action to refine it")
                                .size(11.)
                                .color(MUTED),
                        );
                    });
                });
                let row_height = 32.;
                let name_width = 146.;
                let (ruler, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 30.),
                    Sense::click_and_drag(),
                );
                let track = Rect::from_min_max(
                    Pos2::new(ruler.left() + name_width + 9., ruler.top()),
                    ruler.max,
                );
                let step =
                    ((p.duration as f32 / ((track.width() / 75.).max(1.))).ceil() as u32).max(1);
                for frame in (0..p.duration).step_by(step as usize) {
                    let x = track.left() + track.width() * frame as f32 / p.duration as f32;
                    ui.painter().line_segment(
                        [
                            Pos2::new(x, track.bottom() - 7.),
                            Pos2::new(x, track.bottom()),
                        ],
                        Stroke::new(1., Color32::from_rgb(75, 79, 103)),
                    );
                    ui.painter().text(
                        Pos2::new(x + 3., track.top() + 2.),
                        egui::Align2::LEFT_TOP,
                        format!("{:.1}s", frame as f32 / p.fps as f32),
                        egui::FontId::monospace(10.),
                        MUTED,
                    );
                }
                for (name, frame) in &p.markers {
                    let x = track.left() + track.width() * *frame as f32 / p.duration as f32;
                    let mark = Rect::from_min_size(
                        Pos2::new(x - 4., track.bottom() - 12.),
                        Vec2::new(8., 12.),
                    );
                    ui.painter().rect_filled(mark, 2., theme::ACCENT);
                    if ui
                        .interact(mark, ui.scope_id().with(name), Sense::click())
                        .on_hover_text(name)
                        .clicked()
                    {
                        self.seek_frame(*frame);
                    }
                }
                let x = track.left() + track.width() * self.frame as f32 / p.duration as f32;
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(x - 5., track.top()),
                        Pos2::new(x + 5., track.top()),
                        Pos2::new(x, track.top() + 8.),
                    ],
                    Color32::from_rgb(245, 155, 147),
                    Stroke::NONE,
                ));
                if (response.clicked() || response.dragged())
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    self.stop();
                    self.frame = (((pos.x - track.left()) / track.width()).clamp(0., 1.)
                        * (p.duration - 1) as f32)
                        .round() as f64;
                }
                egui::ScrollArea::vertical()
                    .id_salt("timeline.rows")
                    .show(ui, |ui| {
                        let width = ui.available_width();
                        let pixels = (width - name_width - 12.).max(1.) / p.duration as f32;
                        self.cel_tracks(ui, p, width, name_width, pixels, row_height);
                        for c in &p.characters {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [name_width, 24.],
                                    egui::Label::new(RichText::new(&c.name).color(
                                        if self.selected == Some(c.id) {
                                            VIOLET
                                        } else {
                                            MUTED
                                        },
                                    )),
                                );
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(width - name_width - 12., row_height),
                                    Sense::click(),
                                );
                                let painter = ui.painter_at(rect);
                                painter.rect_filled(rect, 3., Color32::from_rgb(34, 36, 49));
                                for a in p.actions.iter().filter(|a| a.character == c.id) {
                                    let r = Rect::from_min_max(
                                        Pos2::new(
                                            rect.left() + a.start as f32 * pixels,
                                            rect.top() + 3.,
                                        ),
                                        Pos2::new(
                                            rect.left() + (a.end + 1) as f32 * pixels,
                                            rect.bottom() - 3.,
                                        ),
                                    );
                                    painter.rect_filled(
                                        r,
                                        3.,
                                        if self.selected_action == Some(a.id) {
                                            Color32::from_rgb(140, 108, 216)
                                        } else {
                                            Color32::from_rgb(75, 61, 113)
                                        },
                                    );
                                    if r.width() > 35. {
                                        painter.text(
                                            r.center(),
                                            egui::Align2::CENTER_CENTER,
                                            format!("{:?}", a.kind),
                                            egui::FontId::proportional(10.),
                                            Color32::WHITE,
                                        );
                                    }
                                    if response.clicked()
                                        && response
                                            .interact_pointer_pos()
                                            .is_some_and(|pos| r.contains(pos))
                                    {
                                        self.selected = Some(c.id);
                                        self.selected_action = Some(a.id);
                                    }
                                }
                                let x = rect.left() + self.frame as f32 * pixels;
                                for m in p.learned_motions.iter().filter(|m| m.character == c.id) {
                                    let r = Rect::from_min_max(
                                        Pos2::new(
                                            rect.left() + m.start as f32 * pixels,
                                            rect.bottom() - 8.,
                                        ),
                                        Pos2::new(
                                            rect.left() + (m.end + 1) as f32 * pixels,
                                            rect.bottom() - 2.,
                                        ),
                                    );
                                    painter.rect_filled(r, 2., Color32::from_rgb(70, 190, 170));
                                }
                                for key in &c.keys {
                                    painter.circle_filled(
                                        Pos2::new(
                                            rect.left() + key.frame as f32 * pixels,
                                            rect.top() + 4.,
                                        ),
                                        2.5,
                                        Color32::WHITE,
                                    );
                                }
                                painter.line_segment(
                                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                                    Stroke::new(1.5, VIOLET),
                                );
                            });
                        }
                        for (name, color, ranges) in [
                            (
                                "Graphics",
                                Color32::from_rgb(125, 150, 220),
                                p.graphics
                                    .iter()
                                    .map(|g| (g.start, g.end))
                                    .collect::<Vec<_>>(),
                            ),
                            (
                                "Dialogue",
                                Color32::from_rgb(199, 131, 186),
                                p.dialogues
                                    .iter()
                                    .map(|d| (d.start, d.end))
                                    .collect::<Vec<_>>(),
                            ),
                            (
                                "Drawings",
                                Color32::from_rgb(120, 180, 160),
                                p.drawings
                                    .iter()
                                    .map(|d| (d.start, d.end))
                                    .collect::<Vec<_>>(),
                            ),
                        ] {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [name_width, 24.],
                                    egui::Label::new(RichText::new(name).color(MUTED)),
                                );
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(width - name_width - 12., row_height),
                                    Sense::hover(),
                                );
                                let painter = ui.painter_at(rect);
                                painter.rect_filled(rect, 3., Color32::from_rgb(30, 32, 43));
                                for (start, end) in ranges {
                                    painter.rect_filled(
                                        Rect::from_min_max(
                                            Pos2::new(
                                                rect.left() + start as f32 * pixels,
                                                rect.top() + 5.,
                                            ),
                                            Pos2::new(
                                                rect.left() + (end + 1) as f32 * pixels,
                                                rect.bottom() - 5.,
                                            ),
                                        ),
                                        3.,
                                        color,
                                    );
                                }
                            });
                        }
                        self.finishing_tracks(ui, p, width, name_width, pixels, row_height);
                    });
            });
    }
    fn stage(&mut self, ui: &mut egui::Ui, p: &Project) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(rgba(self.preferences.canvas_surround)).inner_margin(20.)).show(ui,|ui|{
            ui.horizontal(|ui|{ui.label(RichText::new("STAGE").size(10.).color(MUTED));ui.label(RichText::new(" / ").color(Color32::from_gray(70)));ui.label(RichText::new(&p.name).size(11.).color(MUTED));});ui.add_space(12.);
            let size=ui.available_size();let scale=(size.x/p.width as f32).min((size.y-20.)/p.height as f32).max(0.01);let stage_size=Vec2::new(p.width as f32*scale,p.height as f32*scale);
            let (area,response)=ui.allocate_exact_size(size,Sense::click_and_drag());let rect=Rect::from_center_size(area.center(),stage_size);let painter=ui.painter_at(rect);
            let list=display_list(p,self.frame);painter.rect_filled(rect,3.,rgba(list.background));
            if self.preferences.show_grid {let painter=painter.with_clip_rect(rect);let step=(48.*scale).max(12.);let mut x=rect.left();while x<rect.right(){painter.line_segment([Pos2::new(x,rect.top()),Pos2::new(x,rect.bottom())],Stroke::new(0.5,Color32::from_rgba_unmultiplied(125,135,150,45)));x+=step;}let mut y=rect.top();while y<rect.bottom(){painter.line_segment([Pos2::new(rect.left(),y),Pos2::new(rect.right(),y)],Stroke::new(0.5,Color32::from_rgba_unmultiplied(125,135,150,45)));y+=step;}}

            if self.preferences.onion && !self.preferences.smart_onion {for (offset,c) in [(-3.,Color32::from_rgba_unmultiplied(96,165,250,38)),(3.,Color32::from_rgba_unmultiplied(244,114,182,38))]{paint_list(&painter,&display_list(p,(self.frame+offset).clamp(0.,(p.duration-1) as f64)),rect,scale,Some(c));}}
            paint_list(&painter,&list,rect,scale,None);
            let scene=evaluate(p,self.frame);let to_screen=|point:Point|Pos2::new(rect.left()+((point.x-scene.center.x)*scene.zoom+p.width as f32/2.)*scale,rect.top()+((point.y-scene.center.y)*scene.zoom+p.height as f32/2.)*scale);
            let to_world=|point:Pos2|Point::new(((point.x-rect.left())/scale-p.width as f32/2.)/scene.zoom+scene.center.x,((point.y-rect.top())/scale-p.height as f32/2.)/scene.zoom+scene.center.y);
            self.paint_pose_suggestion(p,&painter,to_screen,scale*scene.zoom);
            self.paint_motion_overlay(p,&painter,to_screen,scale*scene.zoom);
            self.handle_library_drop(p,ui,rect,to_world);
            if self.motion.draw_path {self.motion_canvas_input(ui,&response,rect,to_world);} else if self.preferences.mode==WorkspaceMode::Draw{
                if self.edit_stroke { self.edit_stroke_on_stage(p,&painter,&response,&to_screen,&to_world); } else {
                if response.drag_started(){self.stroke.clear()}
                if response.dragged()&& let Some(pos)=response.interact_pointer_pos()&& rect.contains(pos){let point=to_world(pos);if self.stroke.last().is_none_or(|p|p.distance(point)>1.){self.stroke.push(point)}}
                if self.stroke.len()>1{let points=self.stroke.iter().map(|p|to_screen(*p)).collect();painter.add(egui::Shape::line(points,Stroke::new(self.draw_width*scale,self.draw_color)));}
                if response.drag_stopped()&&self.stroke.len()>1{let points=std::mem::take(&mut self.stroke);self.apply(vec![Command::AddDrawing{drawing:Drawing{id:Id::new_v4(),name:"Drawing".into(),start:self.frame as u32,end:p.duration-1,points,color:self.draw_color.to_array(),width:self.draw_width}}]);}
                }
            }else{
                for c in &scene.characters{
                    if self.selected==Some(c.id)&&self.preferences.mode==WorkspaceMode::Rig{for(i,j)in c.pose.joints.iter().enumerate(){let point=to_screen(c.pose.root+*j);painter.circle_filled(point,if self.preferences.mode==WorkspaceMode::Rig{4.}else{2.5},VIOLET);if self.preferences.mode==WorkspaceMode::Rig&& let Some(name)=p.character(c.id).and_then(|v|v.joints.get(i)).map(|j|&j.name)&& response.hover_pos().is_some_and(|pos|pos.distance(point)<8.){painter.text(point+Vec2::new(8.,-8.),egui::Align2::LEFT_BOTTOM,name,egui::FontId::proportional(11.),Color32::WHITE);}}
                    }
                }
                if (response.clicked()||response.drag_started())&& let Some(pos)=response.interact_pointer_pos(){let mut nearest=None;let mut distance=18.;for c in &scene.characters{if p.character(c.id).is_some_and(|c|c.locked){continue}for(i,j)in c.pose.joints.iter().enumerate(){let d=pos.distance(to_screen(c.pose.root+*j));if d<distance{distance=d;nearest=Some((c.id,i,c.pose.clone()));}}}if let Some((id,joint,pose))=nearest{self.selected=Some(id);if response.drag_started(){self.stop();self.motion.last_drag_constrained=p.character(id).is_some_and(|c|c.preserve_bones);self.drag_pose=Some(pose);self.drag_joint=if self.preferences.mode==WorkspaceMode::Rig{Some(joint)}else{None};self.drag_origin=Some(pos);}}}
                if response.dragged()&& let (Some(pose),Some(origin),Some(pos))=(&mut self.drag_pose,self.drag_origin,response.interact_pointer_pos()){
                    let start=to_world(origin);let end=to_world(pos);let delta=end-start;
                    if let Some(j)=self.drag_joint{ self.motion.last_drag_constrained=false; if self.preferences.smart_pose && !ui.input(|i|i.modifiers.alt) && let Some(c)=self.selected.and_then(|id|p.character(id)) && let Some(original)=scene.characters.iter().find(|v|v.id==c.id) && let Ok((next,_))=astral_core::studio::pose_with_goal(c,&original.pose,&c.joints[j].name,end,self.preferences.movement_style) { *pose=next; self.motion.last_drag_constrained=true; } else {pose.joints[j]=end-pose.root;} }else{let base=scene.characters.iter().find(|c|Some(c.id)==self.selected).map(|c|c.pose.root).unwrap_or(pose.root);pose.root=base+delta;}
                    let color=Color32::from_rgba_unmultiplied(230,222,255,180);if let Some(c)=scene.characters.iter().find(|c|Some(c.id)==self.selected){for(i,parent)in c.parents.iter().enumerate(){if let Some(parent)=parent{painter.line_segment([to_screen(pose.root+pose.joints[*parent]),to_screen(pose.root+pose.joints[i])],Stroke::new(c.thickness*scale,color));}}}
                }
                if response.drag_stopped(){if let (Some(pose),Some(character))=(self.drag_pose.take(),self.selected){self.apply(vec![Command::SetBoneConstraints{character,enabled:self.motion.last_drag_constrained},Command::SetPose{character,frame:self.frame as u32,pose,easing:Easing::Smooth}]);}self.drag_origin=None;self.drag_joint=None;}
            }
            if p.characters.is_empty()&&p.drawings.is_empty()&&p.rasters.is_empty(){painter.text(rect.center(),egui::Align2::CENTER_CENTER,if p.metadata.contains_key("legacy"){ "Legacy source archived | Native conversion pending\nOpen the original in the legacy editor" }else{"Add a character to animate"},egui::FontId::proportional(18.),MUTED);}
            ui.painter().text(Pos2::new(area.center().x,area.bottom()-1.),egui::Align2::CENTER_BOTTOM,"Space: play    Drag: move    Rig: pose",egui::FontId::proportional(10.),Color32::from_gray(100));
        });
    }
}
impl Studio {
    fn close_confirmation(&mut self, ctx: &egui::Context, p: &Project) {
        if self.confirm_close {
            egui::Window::new("Save before closing?")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(ctx, |ui| {
                    ui.label("Open projects have unsaved changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Save all & close").clicked() {
                            match self.save_all() {
                                Ok(()) => {
                                    self.exit_confirmed = true;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    self.confirm_close = false;
                                }
                                Err(e) => self.status = e.to_string(),
                            }
                        }
                        if ui.button("Keep editing").clicked() {
                            self.confirm_close = false;
                        }
                        if ui.button("Close without saving").clicked() {
                            self.dirty = false;
                            self.exit_confirmed = true;
                            self.saved_revision = Some(p.revision);
                            self.confirm_close = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }
    }
}
fn shortcut_flags(ctx: &egui::Context) -> (bool, bool, bool, bool, bool) {
    // Query focus before reading input: nested Context locks deadlock at startup.
    let text = ctx.egui_wants_keyboard_input();
    ctx.input(|i| {
        (
            i.modifiers.command && i.key_pressed(egui::Key::S),
            !text && i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
            !text && i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z),
            i.modifiers.command && i.key_pressed(egui::Key::P),
            !text && i.key_pressed(egui::Key::Space),
        )
    })
}

impl eframe::App for Studio {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.service.snapshot(self.project).is_err() {
            self.finish_close_project(self.project);
        }
        theme::customize(ctx, &self.preferences);
        self.assist_tick(ctx);
        self.motion_tick(ctx);
        let global = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::N),
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.key_pressed(egui::Key::W),
            )
        });
        if global.0 {
            self.new_project();
        }
        if global.1 {
            self.open_project();
        }
        if global.2 {
            self.save_project_as();
        }
        if global.3 && !self.manager.home {
            self.request_close_project(self.project);
        }
        if !global.2
            && !self.manager.home
            && !self.manager.create
            && !self.manager.properties
            && self.manager.close.is_none()
            && !self.confirm_close
            && !self.assist.blocks_shortcuts()
        {
            self.shortcuts(ctx);
            self.editor_shortcuts(ctx);
        }
        ctx.set_zoom_factor(theme::effective_scale(
            ctx.content_rect().size() * ctx.zoom_factor(),
            self.preferences.scale,
            self.preferences.fit_ui,
        ));
        ctx.global_style_mut(|style| {
            style.animation_time = if self.preferences.reduced_motion {
                0.
            } else {
                0.12
            }
        });
        let now = Instant::now();
        if self.playing
            && let Ok(p) = self.service.snapshot(self.project)
        {
            self.frame += now.duration_since(self.last_tick).as_secs_f64() * p.fps as f64;
            if self.frame >= p.duration as f64 {
                self.frame = 0.;
                self.stop();
            } else {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
        self.last_tick = now;
        if let Some(receiver) = &self.pending_audio {
            if let Ok(result) = receiver.try_recv() {
                self.pending_audio = None;
                match result {
                    Ok(bytes) => {
                        let result = (|| -> anyhow::Result<()> {
                            use rodio::Source;
                            let stream = rodio::OutputStreamBuilder::open_default_stream()?;
                            let sink = rodio::Sink::connect_new(stream.mixer());
                            let decoder = rodio::Decoder::try_from(std::io::Cursor::new(bytes))?;
                            let p = self.service.snapshot(self.project)?;
                            sink.append(
                                decoder.skip_duration(Duration::from_secs_f64(
                                    self.frame / p.fps as f64,
                                )),
                            );
                            self.audio_sink = Some(sink);
                            self.audio_stream = Some(stream);
                            Ok(())
                        })();
                        if let Err(e) = result {
                            self.status = format!("Audio unavailable: {e}")
                        }
                    }
                    Err(e) => self.status = format!("Audio mix failed: {e}"),
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(25));
            }
        }
        if self.last_recovery.elapsed()
            > Duration::from_secs(self.preferences.recovery_seconds.clamp(5, 120))
        {
            if !self
                .recovery_busy
                .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                let projects: Vec<_> = self
                    .service
                    .store
                    .lock()
                    .unwrap()
                    .sessions
                    .values()
                    .map(|s| s.project.clone())
                    .collect();
                let projects: Vec<_> = projects
                    .into_iter()
                    .filter(|p| Some(p.id) != self.manager.placeholder || p.revision > 0)
                    .map(|p| {
                        let w = self.service.project_workspace(p.id);
                        (p, w)
                    })
                    .collect();
                let busy = self.recovery_busy.clone();
                std::thread::spawn(move || {
                    for (p, w) in projects {
                        if let Err(e) = w.save_recovery(&p) {
                            eprintln!("Recovery save: {e}");
                        }
                    }
                    busy.store(false, std::sync::atomic::Ordering::Release);
                });
            }
            self.last_recovery = now;
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        if let Ok(p) = self.service.snapshot(self.project) {
            let saved = self
                .service
                .store
                .lock()
                .unwrap()
                .saved_revisions
                .get(&self.project)
                .copied();
            if saved.is_some() {
                self.saved_revision = saved;
            }
            self.dirty = self.saved_revision != Some(p.revision);
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.exit_confirmed
            && self.has_unsaved_projects()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let Ok(p) = self.service.snapshot(self.project) else {
            return;
        };
        let ctx = ui.ctx().clone();
        self.poll_artwork(&ctx);
        self.assist_windows(&ctx, &p);
        self.motion_windows(&ctx, &p);
        if self.manager.home {
            self.show_projects(ui);
            self.close_confirmation(&ctx, &p);
            return;
        }
        let was_storyboard = self.preferences.mode == WorkspaceMode::Storyboard;
        self.header(ui, &p);
        if was_storyboard
            && self.preferences.mode != WorkspaceMode::Storyboard
            && !self.flush_storyboard()
        {
            self.preferences.mode = WorkspaceMode::Storyboard;
        }

        if self.project != p.id || self.manager.home {
            ctx.request_repaint();
            return;
        }
        egui::Panel::bottom("astral.status")
            .exact_size(28.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(if self.saved_revision == Some(p.revision) {
                            "Saved"
                        } else {
                            "Unsaved changes"
                        })
                        .size(11.)
                        .color(MUTED),
                    );
                    ui.separator();
                    ui.label(RichText::new(&self.status).size(11.).color(MUTED));
                });
            });
        if self.preferences.mode == WorkspaceMode::Storyboard {
            self.storyboard_workspace(ui, &p);
            self.project_dialogs(&ctx);
            self.close_confirmation(&ctx, &p);
            return;
        }
        if self.preferences.advanced_timeline || self.preferences.mode == WorkspaceMode::Audio {
            self.timeline(ui, &p);
        } else {
            self.frame_strip(ui, &p);
        }
        if self.preferences.mode == WorkspaceMode::Draw {
            self.drawing_panel(ui, &p);
            self.draw_stage(ui, &p);
        } else {
            self.scene_panel(ui, &p);
            self.inspector(ui, &p);
            if self.preferences.mode == WorkspaceMode::Artwork {
                self.artwork_canvas(ui, &p);
            } else {
                self.stage(ui, &p);
            }
        }
        self.project_dialogs(&ctx);
        if self.palette {
            let mut open = true;
            egui::Window::new("Commands")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0., 120.])
                .show(&ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search commands...")
                            .desired_width(ui.available_width()),
                    );
                    for (label, action) in [
                        ("Add character", 0),
                        ("Apply action", 1),
                        ("Add effect", 2),
                        ("Save project", 3),
                        ("Export studio", 4),
                        ("Undo", 5),
                    ] {
                        if label.to_lowercase().contains(&self.search.to_lowercase())
                            && ui.button(label).clicked()
                        {
                            match action {
                                0 => self.add_character(),
                                1 => self.add_action(),
                                2 => self.add_effect(),
                                3 => self.save_project(),
                                4 => self.start_export(),
                                _ => self.history(false),
                            }
                            self.palette = false;
                        }
                    }
                });
            if !open {
                self.palette = false;
            }
        }
        if let Some(spec) = self.export_panel.show(&ctx, &p) {
            match self.service.request(Request::ExportMedia {
                project: self.project,
                spec,
            }) {
                Ok(value) => {
                    self.export_job = serde_json::from_value(value["job"].clone()).ok();
                    self.status = "Export queued | you can continue editing".into();
                }
                Err(error) => self.status = error.to_string(),
            }
        }
        if let Some(spec) = self
            .export_panel
            .show_assembly(&ctx, &self.service.project_workspace(self.project))
        {
            match self.service.request(Request::AssembleVideo {
                project: self.project,
                spec,
            }) {
                Ok(value) => {
                    self.export_job = serde_json::from_value(value["job"].clone()).ok();
                    self.status = "Shot assembly queued".into();
                }
                Err(error) => self.status = error.to_string(),
            }
        }
        if let Some(job) = self.export_job
            && let Ok(value) = self.service.request(Request::JobStatus { job })
        {
            egui::Window::new("Render queue")
                .default_width(330.)
                .show(&ctx, |ui| {
                    ui.label(value["state"].as_str().unwrap_or("queued"));
                    ui.add(
                        egui::ProgressBar::new(value["progress"].as_f64().unwrap_or(0.) as f32)
                            .show_percentage(),
                    );
                    if let Some(e) = value["error"].as_str() {
                        ui.colored_label(Color32::LIGHT_RED, e);
                    }
                    if let Some(warnings) = value["warnings"].as_array() {
                        for warning in warnings {
                            if let Some(text) = warning.as_str() {
                                ui.colored_label(Color32::from_rgb(242, 199, 116), text);
                            }
                        }
                    }
                    if let Some(path) = value["path"].as_str() {
                        ui.label(path);
                    }
                    if matches!(
                        value["state"].as_str(),
                        Some("completed" | "failed" | "cancelled")
                    ) {
                        if ui.button("Close").clicked() {
                            self.export_job = None
                        }
                    } else {
                        if ui.button("Cancel export").clicked() {
                            let _ = self.service.request(Request::CancelJob { job });
                        }
                        ctx.request_repaint_after(Duration::from_millis(150));
                    }
                });
        }
        self.close_confirmation(&ctx, &p);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "astral.layout.v1", &self.preferences);
        let projects: Vec<_> = self
            .service
            .store
            .lock()
            .unwrap()
            .sessions
            .values()
            .map(|s| s.project.clone())
            .collect();
        for project in projects {
            if Some(project.id) != self.manager.placeholder || project.revision > 0 {
                let _ = self
                    .service
                    .project_workspace(project.id)
                    .save_recovery(&project);
            }
        }
    }
}
fn rgba(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
fn paint_list(
    painter: &egui::Painter,
    list: &astral_render::DisplayList,
    rect: Rect,
    scale: f32,
    tint: Option<Color32>,
) {
    let pos = |p: Point| Pos2::new(rect.left() + p.x * scale, rect.top() + p.y * scale);
    for primitive in &list.primitives {
        match primitive {
            astral_render::Primitive::Image {
                bitmap,
                vertices,
                indices,
                opacity,
            } => astral_render::paint_image(
                painter, bitmap, vertices, indices, *opacity, rect, scale,
            ),
            Primitive::Path { contours, color } => {
                if let Ok((vertices, indices)) = astral_render::tessellate(contours) {
                    let mesh = egui::Mesh {
                        vertices: vertices
                            .iter()
                            .map(|v| egui::epaint::Vertex {
                                pos: pos(*v),
                                uv: egui::epaint::WHITE_UV,
                                color: tint.unwrap_or_else(|| rgba(*color)),
                            })
                            .collect(),
                        indices,
                        ..Default::default()
                    };
                    painter.add(egui::Shape::mesh(mesh));
                }
            }
            Primitive::Line { a, b, width, color } => {
                painter.line_segment(
                    [pos(*a), pos(*b)],
                    Stroke::new(width * scale, tint.unwrap_or_else(|| rgba(*color))),
                );
            }
            Primitive::Circle {
                center,
                radius,
                color,
            } => {
                painter.circle_filled(
                    pos(*center),
                    radius * scale,
                    tint.unwrap_or_else(|| rgba(*color)),
                );
            }
            Primitive::Polyline {
                points,
                width,
                color,
            } => {
                painter.add(egui::Shape::line(
                    points.iter().map(|p| pos(*p)).collect(),
                    Stroke::new(width * scale, tint.unwrap_or_else(|| rgba(*color))),
                ));
            }
        }
    }
}
pub fn run(service: Service) -> anyhow::Result<()> {
    let id = service.insert(Project::new("Untitled", 1920, 1080, 24, 120))?;
    let ipc = service.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::ipc::serve(ipc).await {
            eprintln!("Desktop agent bridge: {e:#}");
        }
    });
    let data_root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| service.workspace.root().join(".settings"))
        .join("Astral");
    std::fs::create_dir_all(&data_root)?;
    let workspace_settings = data_root.join("workspace.ron");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Astral")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!(
                "../../../assets/brand/astral.png"
            ))?)
            .with_inner_size([1280., 800.])
            .with_maximized(!workspace_settings.is_file())
            .with_min_inner_size([960., 620.])
            .with_app_id("org.astral.studio"),
        renderer: eframe::Renderer::Wgpu,
        multisampling: 4,
        centered: !workspace_settings.is_file(),
        persistence_path: Some(workspace_settings),
        ..Default::default()
    };
    eframe::run_native(
        "Astral",
        options,
        Box::new(move |cc| Ok(Box::new(Studio::new(cc, service, id)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn studio() -> (tempfile::TempDir, Studio, Id) {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path()).unwrap();
        let id = service
            .insert(Project::new("Test", 100, 100, 24, 24))
            .unwrap();
        let layer = DrawingLayer::new("Ink");
        let lid = layer.id;
        service
            .request(Request::ApplyCommands {
                batch: Batch {
                    project: id,
                    expected_revision: 0,
                    commands: vec![
                        Command::AddLayer { layer },
                        Command::SetCel {
                            layer: lid,
                            frame: 0,
                            strokes: vec![],
                        },
                    ],
                },
            })
            .unwrap();
        let mut studio = Studio::from_context(
            egui::Context::default(),
            service,
            id,
            Preferences::default(),
        );
        studio.manager.placeholder = None;
        studio.manager.home = false;
        studio.manager.settings = dir.path().join("settings");
        studio.selected_layer = Some(lid);
        (dir, studio, lid)
    }
    fn paint(studio: &mut Studio, events: Vec<egui::Event>) {
        let ctx = studio.context.clone();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(rect),
                events,
                ..Default::default()
            },
            |ctx| {
                let mut ui = egui::Ui::new(
                    ctx.clone(),
                    egui::Id::unique("test-root"),
                    egui::UiBuilder::new().max_rect(rect),
                );
                let p = studio.service.snapshot(studio.project).unwrap();
                studio.draw_stage(&mut ui, &p);
            },
        );
        output.textures_delta.clear();
    }
    fn drag(studio: &mut Studio, a: Pos2, b: Pos2) {
        paint(studio, vec![]);
        paint(
            studio,
            vec![
                egui::Event::PointerMoved(a),
                egui::Event::PointerButton {
                    pos: a,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        paint(studio, vec![egui::Event::PointerMoved(b)]);
        paint(
            studio,
            vec![egui::Event::PointerButton {
                pos: b,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }
    #[test]
    fn icon_controls_keep_accessible_names_and_fit_small_editors() {
        for size in [
            Vec2::new(800., 520.),
            Vec2::new(960., 620.),
            Vec2::new(1280., 800.),
        ] {
            let (_dir, mut studio, _layer) = studio();
            studio.preferences.onion = true;
            let ctx = studio.context.clone();
            theme::install(&ctx);
            ctx.enable_accesskit();
            studio.apply(vec![
                Command::SetProject {
                    name: Some(
                        "A project with an unusually long name to exercise toolbar sizing"
                            .repeat(3),
                    ),
                    duration: None,
                    background: None,
                },
                Command::SetLayer {
                    layer: studio.selected_layer.unwrap(),
                    name: Some("A long drawing layer name that must truncate".repeat(3)),
                    visible: None,
                    locked: None,
                    opacity: None,
                },
            ]);
            let mut nodes = vec![];
            for _ in 0..4 {
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| {
                        let mut ui = egui::Ui::new(
                            ctx.clone(),
                            egui::Id::unique("layout-test"),
                            egui::UiBuilder::new().max_rect(ctx.content_rect()),
                        );
                        let p = studio.service.snapshot(studio.project).unwrap();
                        studio.header(&mut ui, &p);
                        egui::Panel::bottom("test.status")
                            .exact_size(28.)
                            .show(&mut ui, |_| {});
                        studio.frame_strip(&mut ui, &p);
                        let panel_id = ui.scope_id().with_salt(egui::IdSalt::new("drawing.layers"));
                        ctx.data_mut(|d| {
                            d.insert_persisted(
                                panel_id,
                                egui::containers::panel::PanelState {
                                    outer_rect: Rect::from_min_size(
                                        Pos2::ZERO,
                                        Vec2::new(188., 300.),
                                    ),
                                },
                            )
                        });
                        studio.drawing_panel(&mut ui, &p);
                        assert!(
                            egui::containers::panel::PanelState::load(ctx, panel_id)
                                .unwrap()
                                .size()
                                .x
                                <= 190.,
                            "Drawing controls widened the panel"
                        );
                        studio.draw_stage(&mut ui, &p);
                    },
                );
                output.textures_delta.clear();
                nodes = output.platform_output.accesskit_update.unwrap().nodes;
            }
            let mut controls = vec![];
            for label in [
                "Settings",
                "Save · Ctrl+S",
                "Export",
                "Pencil · B",
                "Eraser · E",
                "Blank",
                "Paste",
                "Detailed timeline",
            ] {
                let node = nodes
                    .iter()
                    .find(|(_, n)| n.role() == egui::Role::Button && n.label() == Some(label))
                    .unwrap_or_else(|| panic!("Missing accessible control: {label}"));
                let bounds = node.1.bounds().unwrap();
                assert!(
                    bounds.x0 >= -0.5
                        && bounds.y0 >= -0.5
                        && bounds.x1 <= size.x as f64 + 0.5
                        && bounds.y1 <= size.y as f64 + 0.5,
                    "{label} escapes {size:?}: {bounds:?}"
                );
                assert!(
                    bounds.x1 - bounds.x0 >= 30. && bounds.y1 - bounds.y0 >= 30.,
                    "Small hit area: {label}"
                );
                controls.push((label, bounds));
            }
            for (i, (a, r)) in controls.iter().enumerate() {
                for (b, s) in &controls[i + 1..] {
                    let overlap_x = r.x1.min(s.x1) - r.x0.max(s.x0);
                    let overlap_y = r.y1.min(s.y1) - r.y0.max(s.y0);
                    assert!(
                        overlap_x <= 0.5 || overlap_y <= 0.5,
                        "Overlapping editor controls: {a} and {b}"
                    );
                }
            }
        }
    }
    #[test]
    fn motion_workspace_tabs_fit_a_compact_window() {
        use motion::Tab;
        for tab in [
            Tab::Pose,
            Tab::Motion,
            Tab::Fight,
            Tab::Coach,
            Tab::Frames,
            Tab::Library,
            Tab::Finish,
            Tab::Camera,
        ] {
            let (_dir, mut studio, _) = studio();
            studio.manager.home = false;
            let a = stick_character(
                "First character".into(),
                Point::new(30., 60.),
                [20, 20, 20, 255],
            );
            let b = stick_character(
                "Second character".into(),
                Point::new(65., 60.),
                [40, 80, 140, 255],
            );
            studio.selected = Some(a.id);
            studio.apply(vec![
                Command::AddRig { character: a },
                Command::AddRig { character: b },
            ]);
            studio.motion.open = true;
            studio.motion.tab = tab;
            let ctx = studio.context.clone();
            theme::install(&ctx);
            ctx.enable_accesskit();
            let mut nodes = vec![];
            for _ in 0..4 {
                let p = studio.service.snapshot(studio.project).unwrap();
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 520.))),
                        ..Default::default()
                    },
                    |ctx| studio.motion_windows(ctx, &p),
                );
                output.textures_delta.clear();
                nodes = output.platform_output.accesskit_update.unwrap().nodes;
            }
            let tabs = nodes
                .iter()
                .filter(|(_, n)| n.role() == egui::Role::Button && n.label() == Some("Fight"))
                .count();
            assert!(tabs > 0);
            for (_, node) in nodes {
                if matches!(
                    format!("{:?}", node.role()).as_str(),
                    "Button" | "Slider" | "TextInput" | "SpinButton" | "ComboBox"
                ) && let Some(r) = node.bounds()
                {
                    assert!(
                        r.x0 >= -0.5 && r.x1 <= 800.5,
                        "Motion control escapes compact window: {:?} {r:?}",
                        node.label()
                    );
                }
            }
        }
    }
    #[test]
    fn inspector_controls_fit_their_panel_in_every_workspace() {
        for mode in [
            WorkspaceMode::Artwork,
            WorkspaceMode::Animate,
            WorkspaceMode::Rig,
            WorkspaceMode::Effects,
            WorkspaceMode::Audio,
            WorkspaceMode::Graphics,
            WorkspaceMode::Dialogue,
        ] {
            let (_dir, mut studio, _layer) = studio();
            studio.preferences.mode = mode;
            let figure =
                stick_character("Character".into(), Point::new(50., 70.), [20, 20, 20, 255]);
            studio.selected = Some(figure.id);
            studio.apply(vec![Command::AddRig { character: figure }]);
            let ctx = studio.context.clone();
            theme::install(&ctx);
            ctx.enable_accesskit();
            let mut nodes = vec![];
            for _ in 0..4 {
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 620.))),
                        ..Default::default()
                    },
                    |ctx| {
                        let mut ui = egui::Ui::new(
                            ctx.clone(),
                            egui::Id::unique("inspector-layout"),
                            egui::UiBuilder::new().max_rect(ctx.content_rect()),
                        );
                        let panel_id = ui
                            .scope_id()
                            .with_salt(egui::IdSalt::new("astral.inspector"));
                        ctx.data_mut(|d| {
                            d.insert_persisted(
                                panel_id,
                                egui::containers::panel::PanelState {
                                    outer_rect: Rect::from_min_size(
                                        Pos2::new(590., 0.),
                                        Vec2::new(210., 620.),
                                    ),
                                },
                            )
                        });
                        let p = studio.service.snapshot(studio.project).unwrap();
                        studio.header(&mut ui, &p);
                        studio.frame_strip(&mut ui, &p);
                        studio.scene_panel(&mut ui, &p);
                        studio.inspector(&mut ui, &p);
                        let panel =
                            egui::containers::panel::PanelState::load(ctx, panel_id).unwrap();
                        assert!(
                            panel.size().x <= 212.,
                            "Inspector grew beyond requested width: {} in mode {}",
                            panel.size().x,
                            mode as u8
                        );
                        studio.stage(&mut ui, &p);
                    },
                );
                output.textures_delta.clear();
                nodes = output.platform_output.accesskit_update.unwrap().nodes;
            }
            for (_, node) in nodes {
                if matches!(
                    format!("{:?}", node.role()).as_str(),
                    "Button" | "Slider" | "TextInput" | "SpinButton" | "ComboBox" | "Label"
                ) && let Some(r) = node.bounds()
                {
                    assert!(
                        r.x0 >= -0.5 && r.x1 <= 800.5,
                        "Control escapes editor in mode {}: {:?} {:?}",
                        mode as u8,
                        node.label(),
                        r
                    );
                }
            }
        }
    }
    #[test]
    fn new_project_action_stays_visible_in_short_windows() {
        for size in [Vec2::new(800., 520.), Vec2::new(1280., 800.)] {
            let (_dir, mut studio, _layer) = studio();
            studio.manager.create = true;
            let ctx = studio.context.clone();
            theme::install(&ctx);
            ctx.enable_accesskit();
            let mut nodes = vec![];
            for _ in 0..4 {
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| studio.project_dialogs(ctx),
                );
                output.textures_delta.clear();
                nodes = output.platform_output.accesskit_update.unwrap().nodes;
            }
            let node = nodes
                .iter()
                .find(|(_, n)| {
                    n.role() == egui::Role::Button && n.label() == Some("Create project")
                })
                .expect("Visible Create action");
            let bounds = node.1.bounds().unwrap();
            assert!(
                bounds.y1 <= size.y as f64 && bounds.y0 >= 0.,
                "Create action clipped: {bounds:?}"
            );
        }
    }
    #[test]
    fn requested_ui_scale_preserves_a_usable_logical_viewport() {
        for size in [
            Vec2::new(960., 620.),
            Vec2::new(1280., 800.),
            Vec2::new(1920., 1080.),
        ] {
            for requested in [0.75, 1., 1.5, 2.] {
                let scale = theme::effective_scale(size, requested, true);
                assert!(scale <= requested && (0.75..=2.).contains(&scale));
                assert!(size.x / scale >= 799. && size.y / scale >= 519.);
            }
        }
        assert_eq!(theme::effective_scale(Vec2::new(960., 620.), 2., false), 2.);
        assert_eq!(
            theme::effective_scale(Vec2::new(1920., 1080.), f32::NAN, true),
            1.
        );
    }
    #[test]
    fn showing_a_suggestion_keeps_canvas_coordinates_fixed() {
        let (_dir, mut studio, layer) = studio();
        let a = Pos2::new(250., 250.);
        let b = Pos2::new(500., 350.);
        drag(&mut studio, a, b);
        assert!(studio.assist.proposal.is_some());
        drag(&mut studio, a, b);
        let p = studio.service.snapshot(studio.project).unwrap();
        let strokes = &p.cel_at(layer, 0).unwrap().strokes;
        assert_eq!(strokes.len(), 2);
        assert_eq!(strokes[0].points, strokes[1].points);
    }
    #[test]
    fn completion_is_explicit_undoable_and_rejects_a_stale_revision() {
        let (_dir, mut studio, layer) = studio();
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(500., 350.));
        let authored = studio.service.snapshot(studio.project).unwrap();
        assert!(studio.assist.proposal.is_some());
        assert_eq!(
            authored.cel_at(layer, 0).unwrap().strokes[0].points.len(),
            2
        );
        studio.accept_suggestion();
        let completed = studio.service.snapshot(studio.project).unwrap();
        assert_eq!(completed.revision, authored.revision + 1);
        assert!(completed.cel_at(layer, 0).unwrap().strokes[0].points.len() > 2);
        studio.history(false);
        assert_eq!(
            serde_json::to_value(studio.service.snapshot(studio.project).unwrap().cels).unwrap(),
            serde_json::to_value(authored.cels).unwrap()
        );
        studio.suggest_after_stroke();
        assert!(studio.assist.proposal.is_some());
        studio.apply(vec![Command::SetProject {
            name: Some("Concurrent edit".into()),
            duration: None,
            background: None,
        }]);
        let current = studio.service.snapshot(studio.project).unwrap();
        studio.accept_suggestion();
        assert_eq!(
            serde_json::to_value(studio.service.snapshot(studio.project).unwrap().cels).unwrap(),
            serde_json::to_value(current.cels).unwrap()
        );
        assert_eq!(
            studio.service.snapshot(studio.project).unwrap().revision,
            current.revision
        );
    }
    #[test]
    fn fast_pen_drag_preserves_its_start_and_splits_a_held_cel() {
        let (_dir, mut studio, layer) = studio();
        studio.preferences.single_frame_drawing = false;
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(500., 350.));
        let p = studio.service.snapshot(studio.project).unwrap();
        let strokes = &p.cel_at(layer, 0).unwrap().strokes;
        assert_eq!(strokes.len(), 1);
        assert!(
            strokes[0]
                .points
                .first()
                .unwrap()
                .distance(*strokes[0].points.last().unwrap())
                > 40.
        );
        let original = serde_json::to_value(strokes).unwrap();
        studio.seek_frame(5);
        drag(&mut studio, Pos2::new(260., 260.), Pos2::new(460., 380.));
        let p = studio.service.snapshot(studio.project).unwrap();
        assert_eq!(
            serde_json::to_value(&p.cel_at(layer, 0).unwrap().strokes).unwrap(),
            original
        );
        assert_eq!(p.cel_at(layer, 5).unwrap().strokes.len(), 2);
    }
    #[test]
    fn manual_drawing_is_single_frame_by_default_and_next_pose_starts_blank() {
        let (_dir, mut studio, layer) = studio();
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(500., 350.));
        let p = studio.service.snapshot(studio.project).unwrap();
        assert_eq!(p.cel_at(layer, 0).unwrap().strokes.len(), 1);
        assert!(p.cel_at(layer, 1).unwrap().strokes.is_empty());
        studio.seek_frame(1);
        drag(&mut studio, Pos2::new(280., 260.), Pos2::new(480., 380.));
        let p = studio.service.snapshot(studio.project).unwrap();
        assert_eq!(p.cel_at(layer, 0).unwrap().strokes.len(), 1);
        assert_eq!(p.cel_at(layer, 1).unwrap().strokes.len(), 1);
        assert!(p.cel_at(layer, 2).unwrap().strokes.is_empty());
        studio.frame_action(&p, "Delete");
        let cleared = studio.service.snapshot(studio.project).unwrap();
        assert!(cleared.cel_at(layer, 1).unwrap().strokes.is_empty());
        assert_eq!(cleared.cel_at(layer, 0).unwrap().strokes.len(), 1);
        studio.history(false);
        assert_eq!(
            studio
                .service
                .snapshot(studio.project)
                .unwrap()
                .cel_at(layer, 1)
                .unwrap()
                .strokes
                .len(),
            1
        );
    }
    #[test]
    fn moving_a_held_drawing_is_independent_and_one_undo_step() {
        let (_dir, mut studio, layer) = studio();
        studio.preferences.single_frame_drawing = false;
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(500., 350.));
        let before = studio.service.snapshot(studio.project).unwrap();
        let old = before.cel_at(layer, 0).unwrap().strokes[0].points[0];
        studio.seek_frame(5);
        studio.draw_tool = frame_editor::DrawTool::Move;
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(350., 250.));
        let p = studio.service.snapshot(studio.project).unwrap();
        assert_eq!(p.cel_at(layer, 0).unwrap().strokes[0].points[0], old);
        assert!(p.cel_at(layer, 5).unwrap().strokes[0].points[0].x > old.x + 10.);
        studio.history(false);
        assert_eq!(
            studio.service.snapshot(studio.project).unwrap().cels.len(),
            before.cels.len()
        );
    }
    #[test]
    fn reopening_an_open_tab_does_not_mark_unsaved_edits_as_saved() {
        let (dir, mut studio, _layer) = studio();
        let file = dir.path().join("Test.astral");
        studio
            .service
            .save_local_project_as(studio.project, &file)
            .unwrap();
        studio.accept_project_file(file.clone()).unwrap();
        let saved = studio.saved_revision;
        studio.apply(vec![Command::SetProject {
            name: Some("Edited".into()),
            duration: None,
            background: None,
        }]);
        studio.open_project_path(file);
        assert_eq!(studio.saved_revision, saved);
        assert!(studio.has_unsaved_projects());
    }
    #[test]
    fn closing_a_dirty_project_requires_a_choice_and_saved_projects_reopen() {
        let (dir, mut studio, _) = studio();
        let file = studio
            .service
            .save_local_project_as(studio.project, &dir.path().join("Test.astral"))
            .unwrap();
        studio.accept_project_file(file.clone()).unwrap();
        let id = studio.project;
        studio.apply(vec![Command::SetProject {
            name: Some("Edited".into()),
            duration: None,
            background: None,
        }]);
        studio.request_close_project(id);
        assert_eq!(studio.manager.close, Some(id));
        assert!(studio.service.snapshot(id).is_ok());
        studio.save_project();
        studio.request_close_project(id);
        assert!(studio.manager.home);
        assert!(studio.service.snapshot(id).is_err());
        assert!(!studio.documents.contains_key(&id));
        studio.open_project_path(file);
        assert_eq!(studio.project, id);
        assert_eq!(studio.service.snapshot(id).unwrap().name, "Edited");
        assert!(!studio.has_unsaved_projects());
    }
    #[test]
    fn fast_eraser_sweeps_the_whole_gesture_and_undo_restores_ink() {
        let (_dir, mut studio, layer) = studio();
        drag(&mut studio, Pos2::new(250., 250.), Pos2::new(500., 350.));
        studio.draw_tool = frame_editor::DrawTool::Eraser;
        drag(&mut studio, Pos2::new(350., 450.), Pos2::new(350., 150.));
        assert!(
            studio
                .service
                .snapshot(studio.project)
                .unwrap()
                .cel_at(layer, 0)
                .unwrap()
                .strokes
                .is_empty()
        );
        studio.history(false);
        assert_eq!(
            studio
                .service
                .snapshot(studio.project)
                .unwrap()
                .cel_at(layer, 0)
                .unwrap()
                .strokes
                .len(),
            1
        );
    }
    #[test]
    fn shortcut_input_does_not_reenter_context_lock() {
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let input = egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: Some(egui::Key::S),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_pass(input, |ctx| {
            let (save, _, _, _, play) = shortcut_flags(ctx);
            assert!(save);
            assert!(!play);
        });
        output.textures_delta.clear();
    }
}
