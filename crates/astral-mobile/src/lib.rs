//! Touch client for the shared Rust document engine. Development preview, not
//! desktop feature parity or a signed store release.
mod icons;
use anyhow::Result;
use astral_core::*;
use astral_storage::Workspace;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use icons::{Icon, button as icon};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Draw,
    Erase,
    Pose,
    Pan,
}

pub struct TouchStudio {
    session: Session,
    workspace: Workspace,
    frame: u32,
    layer: Id,
    playing: bool,
    tick: Instant,
    fractional_frame: f64,
    tool: Tool,
    ink: Vec<Point>,
    color: Color,
    width: f32,
    onion: bool,
    zoom: f32,
    pan: Vec2,
    library: bool,
    project_files: Vec<(PathBuf, String)>,
    settings: bool,
    status: String,
    name: String,
    pending_open: Option<PathBuf>,
    pending_new: bool,
    proposal: Option<Vec<Command>>,
    pose_drag: Option<(Id, usize, Pose)>,
    last_canvas: Option<Rect>,
    #[cfg(target_os = "android")]
    android: Option<android_activity::AndroidApp>,
}

fn blank() -> Project {
    let mut p = Project::new("Untitled", 1280, 720, 24, 144);
    p.schema = 3;
    p.background = [250, 250, 252, 255];
    p.layers.push(DrawingLayer::new("Ink"));
    p
}
fn touch_load(workspace: &Workspace, path: impl AsRef<std::path::Path>) -> Result<Project> {
    let bytes = workspace.read(path)?;
    anyhow::ensure!(bytes.len() >= 60, "Truncated project");
    let size = u64::from_le_bytes(bytes[8..16].try_into()?);
    anyhow::ensure!(
        size <= 64 * 1024 * 1024,
        "Touch preview project limit is 64 MiB uncompressed"
    );
    let p = astral_storage::decode(&bytes)?;
    anyhow::ensure!(
        p.characters.len() <= 32
            && p.graphics.len() <= 2048
            && p.characters.iter().map(|c| c.keys.len()).sum::<usize>() <= 12000,
        "Scene exceeds the touch preview budget; edit this project on desktop"
    );
    anyhow::ensure!(
        !p.assets
            .iter()
            .any(|a| a.kind.to_lowercase().contains("image")
                || a.kind.to_lowercase().contains("video")),
        "Image/video assets are not rendered by this preview; the original project is unchanged"
    );
    Ok(p)
}
impl TouchStudio {
    pub fn new(root: PathBuf) -> Result<Self> {
        let workspace = Workspace::new(root)?;
        let mut recovery_warning = None;
        let p = if workspace.path(".recovery/active.astral")?.is_file() {
            match touch_load(&workspace, ".recovery/active.astral") {
                Ok(p) => p,
                Err(error) => {
                    let bytes = workspace.read(".recovery/active.astral")?;
                    workspace.atomic_write(
                        format!(".recovery/unreadable-{}.astral", Id::new_v4()),
                        &bytes,
                    )?;
                    recovery_warning = Some(format!(
                        "Recovery unavailable ({error}); the original recovery was preserved"
                    ));
                    blank()
                }
            }
        } else {
            blank()
        };
        let layer = p.layers.first().map(|l| l.id).unwrap_or_else(Id::new_v4);
        let name = p.name.clone();
        Ok(Self {
            session: Session::new(p)?,
            workspace,
            frame: 0,
            layer,
            playing: false,
            tick: Instant::now(),
            fractional_frame: 0.,
            tool: Tool::Draw,
            ink: vec![],
            color: [32, 37, 46, 255],
            width: 5.,
            onion: false,
            zoom: 1.,
            pan: Vec2::ZERO,
            library: false,
            project_files: vec![],
            settings: false,
            status: recovery_warning.unwrap_or_else(|| {
                "Development preview - desktop feature parity is incomplete".into()
            }),
            name,
            pending_open: None,
            pending_new: false,
            proposal: None,
            pose_drag: None,
            last_canvas: None,
            #[cfg(target_os = "android")]
            android: None,
        })
    }
    fn apply(&mut self, commands: Vec<Command>) {
        match self.session.apply(Batch {
            project: self.session.project.id,
            expected_revision: self.session.project.revision,
            commands,
        }) {
            Ok(_) => {
                self.status = "Edited".into();
                self.recover();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn recover(&mut self) {
        if let Err(e) = self
            .workspace
            .save(".recovery/active.astral", &self.session.project)
        {
            self.status = format!("Recovery: {e}");
        }
    }
    fn save_project(&mut self) -> bool {
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            self.status = "Enter a project name".into();
            return false;
        }
        if self.session.project.name != name {
            self.apply(vec![Command::SetProject {
                name: Some(name),
                duration: None,
                background: None,
            }]);
        }
        let filename = format!("project-{}.astral", self.session.project.id);
        match self.workspace.save(&filename, &self.session.project) {
            Ok(()) => {
                self.status = "Project saved".into();
                true
            }
            Err(e) => {
                self.status = e.to_string();
                false
            }
        }
    }
    fn seek(&mut self, frame: u32) {
        self.frame = frame.min(self.session.project.duration - 1);
        self.fractional_frame = self.frame as f64;
        self.playing = false;
        self.ink.clear();
        self.pose_drag = None;
    }
    fn history(&mut self, redo: bool) {
        let id = self.session.project.id;
        let rev = self.session.project.revision;
        let result = if redo {
            self.session.redo(id, rev)
        } else {
            self.session.undo(id, rev)
        };
        match result {
            Ok(_) => self.recover(),
            Err(e) => self.status = e.to_string(),
        }
    }
    fn prepare_layer(&mut self) {
        if !self
            .session
            .project
            .layers
            .iter()
            .any(|l| l.id == self.layer)
        {
            let layer = DrawingLayer::new("Ink");
            self.layer = layer.id;
            self.apply(vec![Command::AddLayer { layer }]);
        }
    }
    fn set_ink(&mut self, strokes: Vec<InkStroke>) {
        self.prepare_layer();
        self.apply(vec![Command::SetFrameDrawing {
            layer: self.layer,
            frame: self.frame,
            strokes,
        }]);
    }
    fn export_png(&mut self) {
        let p = &self.session.project;
        let result = (|| -> Result<()> {
            let png = astral_render::rasterize_framed(
                &astral_render::display_list(p, self.frame as f64),
                p.width.min(1920),
                p.height.min(1080),
                false,
            )?
            .encode_png()?;
            self.workspace.atomic_write("exports/frame.png", &png)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.status = "PNG saved to exports/frame.png".into();
                #[cfg(target_os = "android")]
                self.android_action("sharePreview");
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn reset(&mut self, p: Project) -> Result<()> {
        self.session = Session::new(p)?;
        self.layer = self
            .session
            .project
            .layers
            .first()
            .map(|l| l.id)
            .unwrap_or_else(Id::new_v4);
        self.name = self.session.project.name.clone();
        self.seek(0);
        self.zoom = 1.;
        self.pan = Vec2::ZERO;
        self.recover();
        Ok(())
    }
    fn file_actions(&mut self, ui: &mut egui::Ui) {
        if icon(ui, "Projects", Icon::Projects, false).clicked() {
            self.refresh_projects();
            self.library = true;
        }
        if icon(ui, "Save", Icon::Save, false).clicked() {
            self.save_project();
        }
        if icon(ui, "Undo", Icon::Undo, false).clicked() {
            self.history(false);
        }
        if icon(ui, "Redo", Icon::Redo, false).clicked() {
            self.history(true);
        }
        if icon(ui, "Settings", Icon::Settings, false).clicked() {
            self.settings = true;
        }
    }
    fn tool_actions(&mut self, ui: &mut egui::Ui) {
        for (label, tool, kind) in [
            ("Draw", Tool::Draw, Icon::Draw),
            ("Erase", Tool::Erase, Icon::Erase),
            ("Pose", Tool::Pose, Icon::Pose),
            ("Pan", Tool::Pan, Icon::Pan),
        ] {
            if icon(ui, label, kind, self.tool == tool).clicked() {
                self.tool = tool;
                self.ink.clear();
                self.pose_drag = None;
            }
        }
        if icon(ui, "Add rig", Icon::AddRig, false).clicked() {
            let p = &self.session.project;
            self.apply(vec![Command::AddCharacter {
                name: "Character".into(),
                position: Point::new(p.width as f32 * 0.5, p.height as f32 * 0.65),
                color: [65, 120, 180, 255],
            }]);
        }
    }
    fn menus(&mut self, ui: &mut egui::Ui, compact: bool) {
        if compact && ui.available_width() > 580. {
            ui.horizontal(|ui| {
                self.file_actions(ui);
                ui.separator();
                self.tool_actions(ui);
            });
        } else {
            ui.horizontal(|ui| self.file_actions(ui));
            ui.horizontal(|ui| self.tool_actions(ui));
        }
    }
    fn transport(&mut self, ui: &mut egui::Ui) {
        if icon(
            ui,
            if self.playing { "Pause" } else { "Play" },
            if self.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            self.playing,
        )
        .clicked()
        {
            self.playing = !self.playing;
            self.tick = Instant::now();
        }
        if icon(ui, "Previous", Icon::Previous, false).clicked() {
            self.seek(self.frame.saturating_sub(1));
        }
        if icon(ui, "Next", Icon::Next, false).clicked() {
            self.seek((self.frame + 1).min(self.session.project.duration - 1));
        }
        if icon(ui, "Blank", Icon::Blank, false).clicked() {
            self.set_ink(vec![]);
        }
        if icon(ui, "Duplicate", Icon::Duplicate, false).clicked() {
            let strokes = self
                .session
                .project
                .cel_at(self.layer, self.frame)
                .map(|c| c.strokes.clone())
                .unwrap_or_default();
            let next = self.frame + 1;
            if next < self.session.project.duration {
                self.seek(next);
                self.set_ink(strokes);
            }
        }
    }
    fn timeline(&mut self, ui: &mut egui::Ui, compact: bool) {
        if compact {
            ui.horizontal(|ui| {
                self.transport(ui);
                ui.label(format!(
                    "{} / {}",
                    self.frame + 1,
                    self.session.project.duration
                ));
                let mut frame = self.frame;
                ui.spacing_mut().slider_width = (ui.available_width() - 10.).max(30.);
                if ui
                    .add(
                        egui::Slider::new(&mut frame, 0..=self.session.project.duration - 1)
                            .show_value(false),
                    )
                    .changed()
                {
                    self.seek(frame);
                }
            });
            return;
        }
        ui.horizontal(|ui| self.transport(ui));
        let max = self.session.project.duration - 1;
        let mut frame = self.frame;
        ui.horizontal(|ui| {
            ui.label(format!("{} / {}", frame + 1, max + 1));
            if ui
                .add(egui::Slider::new(&mut frame, 0..=max).show_value(false))
                .changed()
            {
                self.seek(frame);
            }
        });
        if compact {
            return;
        }
        egui::ScrollArea::horizontal()
            .max_height(56.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let start = self.frame.saturating_sub(5);
                    let end = (start + 20).min(max);
                    for f in start..=end {
                        let has = self
                            .session
                            .project
                            .cel_at(self.layer, f)
                            .is_some_and(|c| !c.strokes.is_empty());
                        if ui
                            .add_sized(
                                [44., 44.],
                                egui::Button::new(format!(
                                    "{}{}",
                                    f + 1,
                                    if has { " *" } else { "" }
                                ))
                                .selected(f == self.frame),
                            )
                            .clicked()
                        {
                            self.seek(f);
                        }
                    }
                });
            });
    }
    fn canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available = ui.available_size();
        let (area, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        self.last_canvas = Some(area);
        let multi = ctx
            .input(|i| i.multi_touch())
            .filter(|touch| touch.num_touches >= 2);
        if let Some(t) = &multi
            && area.contains(t.center_pos)
        {
            self.zoom = (self.zoom * t.zoom_delta).clamp(0.25, 6.);
            self.pan += t.translation_delta;
            self.ink.clear();
            self.pose_drag = None;
        }
        let p = &self.session.project;
        let fit = (area.width() / p.width as f32)
            .min(area.height() / p.height as f32)
            .max(0.01)
            * self.zoom;
        let rect = Rect::from_center_size(
            area.center() + self.pan,
            Vec2::new(p.width as f32, p.height as f32) * fit,
        );
        let painter = ui.painter_at(area);
        painter.rect_filled(rect, 0., rgba(p.background));
        if self.onion && self.frame > 0 {
            paint(
                &painter,
                &astral_render::display_list(p, (self.frame - 1) as f64),
                rect,
                fit,
                Some(Color32::from_rgba_unmultiplied(70, 130, 200, 48)),
            );
        }
        let ghost = self.pose_drag.as_ref().map(|(id, _, pose)| {
            let mut view = p.clone();
            if let Some(c) = view.characters.iter_mut().find(|c| c.id == *id) {
                c.keys.retain(|k| k.frame != self.frame);
                c.keys.push(PoseKey {
                    frame: self.frame,
                    pose: pose.clone(),
                    easing: Easing::Linear,
                });
                c.keys.sort_by_key(|k| k.frame);
            }
            view
        });
        let view = ghost.as_ref().unwrap_or(p);
        paint(
            &painter,
            &astral_render::display_list(view, self.frame as f64),
            rect,
            fit,
            None,
        );
        if self.tool == Tool::Pose {
            let scene = evaluate(view, self.frame as f64);
            for c in &scene.characters {
                for q in &c.pose.joints {
                    let world = c.pose.root + *q;
                    let v = Point::new(
                        (world.x - scene.center.x) * scene.zoom + view.width as f32 / 2.,
                        (world.y - scene.center.y) * scene.zoom + view.height as f32 / 2.,
                    );
                    painter.circle_stroke(
                        Pos2::new(rect.left() + v.x * fit, rect.top() + v.y * fit),
                        5.,
                        Stroke::new(1., Color32::from_rgb(100, 150, 220)),
                    );
                }
            }
        }
        if self.ink.len() > 1 {
            let scene = evaluate(view, self.frame as f64);
            painter.add(egui::Shape::line(
                self.ink
                    .iter()
                    .map(|q| {
                        Pos2::new(
                            rect.left()
                                + ((q.x - scene.center.x) * scene.zoom + view.width as f32 / 2.)
                                    * fit,
                            rect.top()
                                + ((q.y - scene.center.y) * scene.zoom + view.height as f32 / 2.)
                                    * fit,
                        )
                    })
                    .collect(),
                Stroke::new(self.width * fit, rgba(self.color)),
            ));
        }
        if multi.is_some() || self.playing {
            return;
        }
        let pointer = response.interact_pointer_pos();
        let scene = evaluate(&self.session.project, self.frame as f64);
        let size = Point::new(
            self.session.project.width as f32,
            self.session.project.height as f32,
        );
        let world = |pos: Pos2| {
            Point::new(
                ((pos.x - rect.left()) / fit - size.x / 2.) / scene.zoom + scene.center.x,
                ((pos.y - rect.top()) / fit - size.y / 2.) / scene.zoom + scene.center.y,
            )
        };
        if self.tool == Tool::Pan {
            if response.dragged() {
                self.pan += ctx.input(|i| i.pointer.delta());
            }
            return;
        }
        if response.drag_started()
            && let Some(pos) = ctx
                .input(|i| i.pointer.press_origin())
                .filter(|p| rect.contains(*p))
        {
            let q = world(pos);
            if self.tool == Tool::Draw {
                self.ink = vec![q];
            }
            if self.tool == Tool::Pose {
                let mut nearest = None;
                let mut distance = 24. / fit / scene.zoom;
                for c in &scene.characters {
                    if self
                        .session
                        .project
                        .character(c.id)
                        .is_some_and(|c| c.locked)
                    {
                        continue;
                    }
                    for (i, v) in c.pose.joints.iter().enumerate() {
                        let d = (c.pose.root + *v).distance(q);
                        if d < distance {
                            distance = d;
                            nearest = Some((c.id, i, c.pose.clone()));
                        }
                    }
                }
                self.pose_drag = nearest;
            }
        }
        if response.dragged()
            && let Some(pos) = pointer
        {
            let q = world(pos);
            if self.tool == Tool::Draw
                && self
                    .ink
                    .last()
                    .is_some_and(|last| last.distance(q) > 0.3 / fit)
            {
                self.ink.push(q);
            }
            if self.tool == Tool::Pose
                && let Some((id, index, pose)) = &mut self.pose_drag
            {
                let c = self.session.project.character(*id).unwrap();
                let name = &c.joints[*index].name;
                if name == "hips" {
                    pose.root = q;
                } else if name.ends_with("hand") || name.ends_with("foot") {
                    let mid = c.joints[*index].parent.unwrap();
                    let anchor = c.joints[mid].parent.unwrap();
                    let a = pose.joints[*index] - pose.joints[anchor];
                    let b = pose.joints[mid] - pose.joints[anchor];
                    let bend = if a.x * b.y - a.y * b.x >= 0. { 1. } else { -1. };
                    let _ = staging::aim_limb(c, pose, name, q, bend);
                } else {
                    pose.joints[*index] = q - pose.root;
                }
            }
        }
        if response.drag_stopped() {
            if self.tool == Tool::Draw && self.ink.len() > 1 {
                let mut strokes = self
                    .session
                    .project
                    .cel_at(self.layer, self.frame)
                    .map(|c| c.strokes.clone())
                    .unwrap_or_default();
                strokes.push(InkStroke {
                    points: std::mem::take(&mut self.ink),
                    width: self.width,
                    color: self.color,
                });
                self.set_ink(strokes);
            }
            if self.tool == Tool::Pose
                && let Some((id, _, pose)) = self.pose_drag.take()
            {
                self.apply(vec![Command::SetPose {
                    character: id,
                    frame: self.frame,
                    pose,
                    easing: Easing::Smooth,
                }]);
            }
        }
        if self.tool == Tool::Erase
            && (response.clicked() || response.drag_stopped())
            && let Some(pos) = pointer
        {
            let q = world(pos);
            let mut strokes = self
                .session
                .project
                .cel_at(self.layer, self.frame)
                .map(|c| c.strokes.clone())
                .unwrap_or_default();
            strokes.retain(|s| !s.points.iter().any(|v| v.distance(q) < 24. / fit));
            self.set_ink(strokes);
        }
    }
    fn refresh_projects(&mut self) {
        self.project_files = std::fs::read_dir(self.workspace.root())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "astral"))
            .take(200)
            .map(|path| {
                let label = touch_load(&self.workspace, path.file_name().unwrap())
                    .map(|p| p.name)
                    .unwrap_or_else(|_| "Unreadable project".into());
                (path, label)
            })
            .collect();
        self.project_files.sort_by(|a, b| a.1.cmp(&b.1));
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.library {
            ctx.request_repaint_after(Duration::from_millis(250));
            let mut open = true;
            egui::Window::new("Projects")
                .open(&mut open)
                .default_width(300.)
                .vscroll(true)
                .max_height((ctx.content_rect().height() - 80.).max(140.))
                .show(ctx, |ui| {
                    if ui.button("New project").clicked() {
                        self.pending_new = true;
                    }
                    #[cfg(target_os = "android")]
                    if ui.button("Import .astral").clicked() {
                        self.android_action("chooseProject");
                    }
                    egui::ScrollArea::vertical()
                        .max_height(300.)
                        .show(ui, |ui| {
                            for (path, label) in &self.project_files {
                                if ui.button(label).clicked() {
                                    self.pending_open = Some(path.clone());
                                }
                            }
                        });
                    ui.small("Save before creating or opening another project.");
                });
            self.library = open;
        }
        if self.pending_new || self.pending_open.is_some() {
            egui::Window::new("Save current project?").collapsible(false).show(ctx,|ui| {ui.label("The active recovery will be replaced. Save first or keep the current project.");
                if ui.button("Save and continue").clicked()&&self.save_project(){let result=if self.pending_new {Ok(blank())}else{touch_load(&self.workspace,self.pending_open.as_ref().unwrap().file_name().unwrap())};match result.and_then(|p|self.reset(p)){Ok(())=>self.library=false,Err(e)=>self.status=e.to_string()}self.pending_new=false;self.pending_open=None;}
                if ui.button("Cancel").clicked(){self.pending_new=false;self.pending_open=None;}
            });
        }
        if self.settings {
            let mut open = true;
            egui::Window::new("Project settings").open(&mut open).default_width(300.).vscroll(true).max_height((ctx.content_rect().height()-80.).max(140.)).show(ctx,|ui|{
                ui.text_edit_singleline(&mut self.name);ui.add(egui::Slider::new(&mut self.width,1.0..=30.0).text("Brush"));ui.color_edit_button_srgba_unmultiplied(&mut self.color);ui.checkbox(&mut self.onion,"Onion skin");
                let mut fps=self.session.project.fps;if ui.add(egui::Slider::new(&mut fps,1..=60).text("FPS")).changed(){self.apply(vec![Command::SetFrameRate{fps}]);}
                let mut duration=self.session.project.duration;if ui.add(egui::DragValue::new(&mut duration).range(2..=1200).prefix("Frames ")).changed(){self.apply(vec![Command::SetProject{name:None,duration:Some(duration),background:None}]);self.frame=self.frame.min(self.session.project.duration-1);}
                if ui.button("Export PNG / share").clicked(){self.export_png();}
                if ui.button("Reset view").clicked(){self.zoom=1.;self.pan=Vec2::ZERO;}
                if ui.button("Propose a six-second spar").clicked(){match director::plan(&self.session.project,director::AutoSpec{end:(self.session.project.fps*6).min(960)-1,..Default::default()}){Ok(p)if p.quality_gate_passed=>self.proposal=Some(p.plan.commands),Ok(_)=>self.status="Proposal failed geometric checks".into(),Err(e)=>self.status=e.to_string()}}
                ui.small("Preview: drawing, rig posing, timeline, project storage and PNG export. Audio, video export and desktop parity are not yet available on mobile.");
            });
            self.settings = open;
        }
        if self.proposal.is_some() {
            egui::Window::new("Editable spar proposal").collapsible(false).show(ctx,|ui|{ui.label("Creates two rigs, editable poses, storyboard beats, camera and impacts. Existing drawings are retained. You can undo it in one step.");if ui.button("Apply proposal").clicked(){let commands=self.proposal.take().unwrap();self.apply(commands);}
if ui.button("Cancel").clicked(){self.proposal=None;}});
        }
        if self
            .workspace
            .path("import-incoming.astral")
            .is_ok_and(|p| p.exists())
        {
            let result = (|| -> Result<PathBuf> {
                let p = touch_load(&self.workspace, "import-incoming.astral")?;
                let name = format!("import-{}.astral", p.id);
                self.workspace.save(&name, &p)?;
                Ok(self.workspace.root().join(name))
            })();
            match result {
                Ok(path) => {
                    self.pending_open = Some(path);
                    self.refresh_projects();
                }
                Err(error) => {
                    self.pending_open = None;
                    self.status = format!("Import rejected: {error}");
                    if let Ok(bytes) = self.workspace.read("import-incoming.astral") {
                        let _ = self.workspace.atomic_write(
                            format!(".recovery/rejected-import-{}.astral", Id::new_v4()),
                            &bytes,
                        );
                    }
                }
            }
            let _ = std::fs::remove_file(self.workspace.root().join("import-incoming.astral"));
        }
    }
}

impl TouchStudio {
    fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        if self.playing {
            self.fractional_frame = (self.fractional_frame
                + now.duration_since(self.tick).as_secs_f64().min(0.2)
                    * self.session.project.fps as f64)
                % self.session.project.duration as f64;
            self.frame = self.fractional_frame as u32;
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.tick = now;
        let safe = ctx.content_rect().intersect(ui.max_rect());
        let compact = safe.height() < 500.;
        let top_height = if compact && safe.width() > 600. {
            64.
        } else {
            120.
        };
        let bottom_height = if compact { 92. } else { 228. };
        let top = Rect::from_min_max(safe.min, Pos2::new(safe.right(), safe.top() + top_height));
        let bottom = Rect::from_min_max(
            Pos2::new(safe.left(), safe.bottom() - bottom_height),
            safe.max,
        );
        let middle = Rect::from_min_max(
            Pos2::new(safe.left(), top.bottom()),
            Pos2::new(safe.right(), bottom.top()),
        );
        for (id, rect) in [("touch.top", top), ("touch.bottom", bottom)] {
            ui.painter()
                .rect_filled(rect, 0., Color32::from_rgb(26, 29, 36));
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect.shrink(8.)));
            child.shrink_clip_rect(rect);
            if id == "touch.top" {
                self.menus(&mut child, compact);
            } else {
                self.timeline(&mut child, compact);
                child.label(egui::RichText::new(&self.status).small());
            }
        }
        let mut canvas = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("touch.canvas")
                .max_rect(middle.shrink(8.)),
        );
        canvas.shrink_clip_rect(middle);
        self.canvas(&mut canvas, &ctx);
        self.dialogs(&ctx);
    }
}
impl eframe::App for TouchStudio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
    fn on_exit(&mut self) {
        self.recover();
    }
    #[cfg(target_os = "android")]
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some(v) = self.android_safe_insets() {
            let scale = input
                .viewports
                .get(&egui::ViewportId::ROOT)
                .and_then(|v| v.native_pixels_per_point)
                .or_else(|| ctx.native_pixels_per_point())
                .unwrap_or(1.)
                .max(0.1);
            input.safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 {
                left: v[0] as f32 / scale,
                top: v[1] as f32 / scale,
                right: v[2] as f32 / scale,
                bottom: v[3] as f32 / scale,
            }));
        }
    }
}
fn rgba(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
fn paint(
    painter: &egui::Painter,
    list: &astral_render::DisplayList,
    rect: Rect,
    scale: f32,
    tint: Option<Color32>,
) {
    let pos = |p: Point| Pos2::new(rect.left() + p.x * scale, rect.top() + p.y * scale);
    for p in &list.primitives {
        match p {
            astral_render::Primitive::Path { contours, color } => {
                if let Ok((v, i)) = astral_render::tessellate(contours) {
                    painter.add(egui::Shape::mesh(egui::Mesh {
                        vertices: v
                            .iter()
                            .map(|v| egui::epaint::Vertex {
                                pos: pos(*v),
                                uv: egui::epaint::WHITE_UV,
                                color: tint.unwrap_or_else(|| rgba(*color)),
                            })
                            .collect(),
                        indices: i,
                        ..Default::default()
                    }));
                }
            }
            astral_render::Primitive::Line { a, b, width, color } => {
                painter.line_segment(
                    [pos(*a), pos(*b)],
                    Stroke::new(width * scale, tint.unwrap_or_else(|| rgba(*color))),
                );
            }
            astral_render::Primitive::Circle {
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
            astral_render::Primitive::Polyline {
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
fn mobile_options(mut options: eframe::NativeOptions) -> eframe::NativeOptions {
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        // GLES avoids Android emulator Vulkan surface-acquisition stalls.
        // Keep the preview on one tested mobile backend until device profiling.
        #[cfg(target_os = "android")]
        {
            setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        setup.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
            label: Some("Astral touch device"),
            // The UI uses sampled textures, vertices and a small uniform block,
            // not compute/storage shaders. Do not demand desktop 64-KiB UBOs
            // from Android adapters whose supported maximum is 16 KiB.
            required_limits: eframe::wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            ..Default::default()
        });
    }
    options
}
pub fn run(root: PathBuf, mut options: eframe::NativeOptions) -> Result<()> {
    options.persistence_path = Some(root.join("window.ron"));
    let app = TouchStudio::new(root)?;
    eframe::run_native(
        "Astral Touch Preview",
        mobile_options(options),
        Box::new(move |cc| {
            configure(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
fn configure(ctx: &egui::Context) {
    let mut style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };
    style.spacing.interact_size = Vec2::splat(48.);
    style.spacing.item_spacing = Vec2::new(8., 8.);
    style.visuals.selection.bg_fill = Color32::from_rgb(64, 99, 154);
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_recovery_is_preserved_and_does_not_prevent_startup() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        workspace
            .atomic_write(".recovery/active.astral", b"broken recovery")
            .unwrap();
        let app = TouchStudio::new(dir.path().to_owned()).unwrap();
        assert!(app.status.contains("preserved"));
        let copies: Vec<_> = std::fs::read_dir(dir.path().join(".recovery"))
            .unwrap()
            .flatten()
            .filter(|f| f.file_name().to_string_lossy().starts_with("unreadable-"))
            .collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(std::fs::read(copies[0].path()).unwrap(), b"broken recovery");
    }
    #[test]
    fn single_frame_edits_save_recover_and_undo_on_touch_client() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = TouchStudio::new(dir.path().to_owned()).unwrap();
        let stroke = InkStroke {
            points: vec![Point::new(10., 10.), Point::new(60., 60.)],
            color: [10, 10, 10, 255],
            width: 3.,
        };
        app.set_ink(vec![stroke]);
        assert_eq!(
            app.session
                .project
                .cel_at(app.layer, 0)
                .unwrap()
                .strokes
                .len(),
            1
        );
        app.seek(1);
        assert!(
            app.session
                .project
                .cel_at(app.layer, 1)
                .unwrap()
                .strokes
                .is_empty()
        );
        app.name = "Touch project".into();
        assert!(app.save_project());
        let recovered = TouchStudio::new(dir.path().to_owned()).unwrap();
        assert_eq!(recovered.session.project.name, "Touch project");
        assert!(
            recovered
                .session
                .project
                .cel_at(recovered.layer, 1)
                .unwrap()
                .strokes
                .is_empty()
        );
        app.seek(0);
        app.set_ink(vec![]);
        app.history(false);
        assert_eq!(
            app.session
                .project
                .cel_at(app.layer, 0)
                .unwrap()
                .strokes
                .len(),
            1
        );
    }
    #[test]
    fn touch_toolbar_receives_pointer_clicks() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = TouchStudio::new(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        configure(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(390., 844.));
        let pos = Pos2::new(88., 32.);
        for pressed in [None, None, Some(true), Some(false)] {
            let mut input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            if let Some(pressed) = pressed {
                input.events = vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ];
            }
            let mut output = ctx.run_pass(input, |ctx| {
                let mut ui = egui::Ui::new(
                    ctx.clone(),
                    egui::Id::unique("touch-test"),
                    egui::UiBuilder::new().max_rect(screen),
                );
                app.show(&mut ui);
            });
            output.textures_delta.clear();
        }
        assert!(
            dir.path()
                .join(format!("project-{}.astral", app.session.project.id))
                .is_file(),
            "{}",
            app.status
        );
    }
    #[test]
    fn phone_and_tablet_layouts_leave_a_visible_canvas() {
        for size in [
            Vec2::new(390., 844.),
            Vec2::new(844., 390.),
            Vec2::new(1024., 768.),
            Vec2::new(320., 568.),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut app = TouchStudio::new(dir.path().to_owned()).unwrap();
            let ctx = egui::Context::default();
            configure(&ctx);
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            for _ in 0..3 {
                ctx.data_mut(|data| {
                    data.remove::<Vec<(String, Rect, Rect)>>(egui::Id::unique(
                        "touch.control-audit",
                    ))
                });
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    },
                    |ctx| {
                        let mut ui = egui::Ui::new(
                            ctx.clone(),
                            egui::Id::unique("touch-test"),
                            egui::UiBuilder::new().max_rect(screen),
                        );
                        app.show(&mut ui);
                    },
                );
                output.textures_delta.clear();
                let controls = ctx.data(|data| {
                    data.get_temp::<Vec<(String, Rect, Rect)>>(egui::Id::unique(
                        "touch.control-audit",
                    ))
                    .unwrap_or_default()
                });
                for (label, rect, clip) in controls {
                    assert!(
                        clip.expand(1.).contains_rect(rect),
                        "{size:?}: {label} {rect:?} outside {clip:?}"
                    );
                }
            }
            let rect = app.last_canvas.unwrap();
            assert!(
                rect.width() > 100. && rect.height() > 60.,
                "{size:?}: {rect:?}"
            );
            assert!(
                screen.expand(1.).contains_rect(rect),
                "Canvas escaped viewport: {rect:?}"
            );
        }
    }
}
#[cfg(target_os = "android")]
impl TouchStudio {
    fn android_safe_insets(&self) -> Option<[i32; 4]> {
        let result = (|| -> Result<[i32; 4]> {
            let app = self
                .android
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No Android host"))?;
            let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
            let mut env = vm.attach_current_thread()?;
            let value = env.with_local_frame(8, |env| -> jni::errors::Result<[i32; 4]> {
                let activity =
                    unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr().cast()) };
                let array = jni::objects::JIntArray::from(
                    env.call_method(&activity, "getSafeInsets", "()[I", &[])?
                        .l()?,
                );
                let mut result = [0; 4];
                env.get_int_array_region(&array, 0, &mut result)?;
                Ok(result)
            })?;
            Ok(value)
        })();
        result.ok()
    }
    fn android_action(&mut self, method: &str) {
        let result = (|| -> Result<()> {
            let app = self
                .android
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Android host missing"))?;
            // AndroidApp owns the VM and global activity reference for this call.
            let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
            let mut env = vm.attach_current_thread()?;
            let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr().cast()) };
            env.call_method(&activity, method, "()V", &[])?;
            Ok(())
        })();
        if let Err(e) = result {
            self.status = e.to_string();
        }
    }
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    let result = (|| -> Result<()> {
        let root = app
            .internal_data_path()
            .ok_or_else(|| anyhow::anyhow!("No private data path"))?
            .join("projects");
        let persistence = root.join("window.ron");
        let mut studio = TouchStudio::new(root)?;
        studio.android = Some(app.clone());
        eframe::run_native(
            "Astral Touch Preview",
            mobile_options(eframe::NativeOptions {
                android_app: Some(app),
                renderer: eframe::Renderer::Wgpu,
                persistence_path: Some(persistence),
                ..Default::default()
            }),
            Box::new(move |cc| {
                configure(&cc.egui_ctx);
                Ok(Box::new(studio))
            }),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))
    })();
    if let Err(e) = result {
        log::error!("Astral startup: {e:#}");
    }
}
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn astral_ios_main() {
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Documents/Astral");
    if let Err(e) = run(root, eframe::NativeOptions::default()) {
        eprintln!("Astral startup: {e:#}");
    }
}
