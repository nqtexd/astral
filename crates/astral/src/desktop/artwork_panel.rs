use super::*;
use crate::artwork::ArtworkRequest;
#[derive(Default)]
pub(super) struct ArtworkState {
    selected: Option<Id>,
    job: Option<Id>,
    tracking_seed: Point,
    tracking_frames: u32,
    paint: bool,
    erase: bool,
    radius: f32,
    points: Vec<Point>,
    pending: Option<std::sync::mpsc::Receiver<anyhow::Result<serde_json::Value>>>,
    recording: Option<crate::recording::Recording>,
    recording_target: Option<(Id, u32)>,
    clip: Option<Id>,
    text: String,
}
impl Studio {
    pub(super) fn artwork_browser(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.left {
            return;
        }
        egui::Panel::left("astral.artwork.layers")
            .default_size(210.)
            .min_size(160.)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(12.);
                ui.heading("Layers");
                ui.add_space(10.);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for raster in p.rasters.iter().rev() {
                        ui.push_id(raster.id, |ui| {
                            ui.horizontal(|ui| {
                                if theme::icon_button(
                                    ui,
                                    if raster.visible {
                                        "Hide artwork"
                                    } else {
                                        "Show artwork"
                                    },
                                    if raster.visible {
                                        theme::Icon::Eye
                                    } else {
                                        theme::Icon::EyeOff
                                    },
                                    false,
                                )
                                .clicked()
                                {
                                    let mut next = raster.clone();
                                    next.visible = !next.visible;
                                    self.apply(vec![Command::PutRasterLayer { layer: next }]);
                                }
                                let width = ui.available_width();
                                if ui
                                    .add_sized(
                                        [width, 32.],
                                        egui::Button::new(&raster.name)
                                            .selected(self.artwork.selected == Some(raster.id))
                                            .truncate(),
                                    )
                                    .clicked()
                                {
                                    self.artwork.selected = Some(raster.id);
                                    self.artwork.paint = false;
                                }
                            });
                        });
                    }
                    if p.rasters.is_empty() {
                        ui.label(
                            RichText::new("Import artwork in the Inspector to begin.").color(MUTED),
                        );
                    }
                });
            });
    }
    fn artwork_request(&mut self, request: ArtworkRequest) {
        if self.artwork.pending.is_some() {
            return;
        }
        let service = self.service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.artwork.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(service.artwork(request));
        });
    }
    pub(super) fn poll_artwork(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.artwork.pending {
            ctx.request_repaint_after(Duration::from_millis(80));
            if let Ok(result) = rx.try_recv() {
                self.artwork.pending = None;
                match result {
                    Ok(value) => {
                        if let Some(id) = value
                            .get("job")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                        {
                            self.export_job = Some(id);
                            self.artwork.job = Some(id);
                        }
                        self.status = "Artwork updated".into();
                        self.dirty = true;
                    }
                    Err(e) => self.status = format!("{e:#}"),
                }
            }
        }
        if let Some(job) = self.artwork.job {
            ctx.request_repaint_after(Duration::from_millis(100));
            if let Ok(value) = self.service.request(Request::JobStatus { job })
                && matches!(
                    value["state"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                )
            {
                self.artwork.job = None;
                if let Some(id) = value["created_layers"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                {
                    self.artwork.selected = Some(id);
                }
                if value["plan"].is_object()
                    && let Ok(p) = self.service.snapshot(self.project)
                {
                    if value["revision"].as_u64() == Some(p.revision)
                        && value["project"] == serde_json::json!(p.id)
                    {
                        if let Ok(commands) =
                            serde_json::from_value(value["plan"]["commands"].clone())
                        {
                            self.motion_plan(
                                &p,
                                studio::StudioOperation::Commands {
                                    name: "Track artwork camera".into(),
                                    commands,
                                },
                                self.frame as u32,
                            );
                        }
                    } else {
                        self.status="Tracking finished, but the document changed. Track again at the current revision.".into();
                    }
                }
            }
        }
    }
    pub(super) fn artwork_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.heading("Artwork");
        ui.label(RichText::new("Images, cels & deformation").color(MUTED));
        ui.add_space(10.);
        let busy = self.artwork.pending.is_some() || self.artwork.job.is_some();
        if ui
            .add_enabled(!busy, egui::Button::new("Import artwork…"))
            .on_hover_text("PNG, JPEG, WebP, BMP, PSD layers or an ordered PNG sequence")
            .clicked()
            && let Some(mut paths) = rfd::FileDialog::new()
                .add_filter(
                    "Artwork",
                    &["png", "jpg", "jpeg", "webp", "bmp", "gif", "psd"],
                )
                .pick_files()
        {
            paths.sort();
            let service = self.service.clone();
            let project = p.id;
            let revision = p.revision;
            let start = self.frame as u32;
            let fps = p.fps;
            let (tx, rx) = std::sync::mpsc::channel();
            self.artwork.pending = Some(rx);
            std::thread::spawn(move || {
                let result = (|| -> anyhow::Result<_> {
                    anyhow::ensure!(paths.len() <= 240, "Import at most 240 images");
                    let workspace = service.project_workspace(project);
                    let folder = format!("assets/artwork-{}", Id::new_v4());
                    let mut relative = vec![];
                    for path in paths {
                        anyhow::ensure!(
                            std::fs::metadata(&path)?.len() <= 32 * 1024 * 1024,
                            "Artwork exceeds 32 MiB input limit"
                        );
                        let name = path
                            .file_name()
                            .ok_or_else(|| anyhow::anyhow!("Missing filename"))?
                            .to_string_lossy();
                        let target = format!("{folder}/{name}");
                        workspace.atomic_write(&target, &std::fs::read(&path)?)?;
                        relative.push(target);
                    }
                    service.artwork(ArtworkRequest::Import {
                        project,
                        expected_revision: revision,
                        paths: relative,
                        start,
                        source_fps: fps,
                        looping: false,
                    })
                })();
                let _ = tx.send(result);
            });
        }
        if ui
            .add_enabled(!busy, egui::Button::new("Extract video frames…"))
            .on_hover_text("Imports up to 120 editable frames at project FPS; add audio separately")
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Video", &["mp4", "mov", "webm", "mkv"])
                .pick_file()
        {
            let service = self.service.clone();
            let project = p.id;
            let revision = p.revision;
            let start = self.frame as u32;
            let fps = p.fps.min(60);
            let frames = (p.duration - start).min(120);
            let scale = (640. / p.width.max(p.height) as f32).min(1.);
            let width = (p.width as f32 * scale).round().max(1.) as u32;
            let height = (p.height as f32 * scale).round().max(1.) as u32;
            let (tx, rx) = std::sync::mpsc::channel();
            self.artwork.pending = Some(rx);
            std::thread::spawn(move || {
                let result = (|| -> anyhow::Result<_> {
                    anyhow::ensure!(
                        std::fs::metadata(&path)?.len() <= 512 * 1024 * 1024,
                        "Video input exceeds 512 MiB"
                    );
                    let relative = format!(
                        "assets/video-{}.{}",
                        Id::new_v4(),
                        path.extension().unwrap_or_default().to_string_lossy()
                    );
                    service
                        .project_workspace(project)
                        .atomic_write(&relative, &std::fs::read(path)?)?;
                    service.artwork(ArtworkRequest::Video {
                        project,
                        expected_revision: revision,
                        path: relative,
                        start,
                        frames,
                        fps,
                        width,
                        height,
                    })
                })();
                let _ = tx.send(result);
            });
        }
        ui.add_space(12.);
        for raster in &p.rasters {
            if ui
                .selectable_label(self.artwork.selected == Some(raster.id), &raster.name)
                .clicked()
            {
                self.artwork.selected = Some(raster.id);
                self.artwork.paint = false;
            }
        }
        if self.artwork.selected.is_none() {
            self.artwork.selected = p.rasters.first().map(|r| r.id);
        }
        if let Some(source) = self
            .artwork
            .selected
            .and_then(|id| p.rasters.iter().find(|r| r.id == id))
        {
            ui.separator();
            let mut raster = source.clone();
            let mut changed = false;
            changed |= ui.text_edit_singleline(&mut raster.name).changed();
            changed |= ui.checkbox(&mut raster.visible, "Visible").changed();
            changed |= ui
                .checkbox(&mut raster.looping, "Loop source frames")
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut raster.source_fps)
                        .range(1..=240)
                        .prefix("Source FPS "),
                )
                .changed();
            ui.label(format!(
                "{} source images · {} painted cels",
                raster.sources.len(),
                raster.cels.iter().filter(|c| c.bitmap.is_some()).count()
            ));
            ui.checkbox(&mut self.artwork.paint,"Paint this frame").on_hover_text("Shows original image coordinates. A stroke changes only this frame; later sequence frames remain independent.");
            if self.artwork.paint {
                ui.checkbox(&mut self.artwork.erase, "Eraser");
                if self.artwork.radius < 0.5 {
                    self.artwork.radius = 8.;
                }
                ui.add(egui::Slider::new(&mut self.artwork.radius, 0.5..=128.).text("Brush"));
                ui.color_edit_button_srgba(&mut self.draw_color);
            }
            let mut key = raster.at(self.frame);
            key.frame = self.frame as u32;
            ui.collapsing("Transform", |ui| {
                let mut edit = false;
                edit |= ui
                    .add(egui::DragValue::new(&mut key.position.x).prefix("X "))
                    .changed();
                edit |= ui
                    .add(egui::DragValue::new(&mut key.position.y).prefix("Y "))
                    .changed();
                edit |= ui
                    .add(
                        egui::DragValue::new(&mut key.scale.x)
                            .speed(0.01)
                            .range(0.001..=100.)
                            .prefix("Scale X "),
                    )
                    .changed();
                edit |= ui
                    .add(
                        egui::DragValue::new(&mut key.scale.y)
                            .speed(0.01)
                            .range(0.001..=100.)
                            .prefix("Scale Y "),
                    )
                    .changed();
                edit |= ui
                    .add(
                        egui::Slider::new(
                            &mut key.rotation,
                            -std::f32::consts::PI..=std::f32::consts::PI,
                        )
                        .text("Rotation"),
                    )
                    .changed();
                edit |= ui
                    .add(egui::Slider::new(&mut key.opacity, 0.0..=1.).text("Opacity"))
                    .changed();
                if edit {
                    raster.keys.retain(|k| k.frame != key.frame);
                    raster.keys.push(key.clone());
                    raster.keys.sort_by_key(|k| k.frame);
                    changed = true;
                }
            });
            ui.collapsing("Track camera",|ui|{
                ui.label("Choose a distinctive source-image point. Preview local tracking before accepting camera keys.");
                ui.add(egui::DragValue::new(&mut self.artwork.tracking_seed.x).prefix("Source X "));
                ui.add(egui::DragValue::new(&mut self.artwork.tracking_seed.y).prefix("Source Y "));
                self.artwork.tracking_frames=self.artwork.tracking_frames.clamp(1,120);
                ui.add(egui::DragValue::new(&mut self.artwork.tracking_frames).range(1..=120).prefix("Frames "));
                if ui.add_enabled(!busy&&self.artwork.job.is_none(),egui::Button::new("Preview tracking")).clicked(){self.artwork_request(ArtworkRequest::Track{project:p.id,expected_revision:p.revision,layer:source.id,start:self.frame as u32,end:(self.frame as u32+self.artwork.tracking_frames-1).min(source.end),point:self.artwork.tracking_seed,apply:false});}
            });
            ui.collapsing("Deformation", |ui| {
                if raster.mesh.is_none() && ui.button("Create pin mesh").clicked() {
                    raster.mesh = Some(DeformMesh {
                        columns: 12,
                        rows: 12,
                        pins: [(0., 0.), (1., 0.), (0., 1.), (1., 1.), (0.5, 0.5)]
                            .iter()
                            .enumerate()
                            .map(|(i, (x, y))| MeshPin {
                                id: Id::new_v4(),
                                name: if i == 4 {
                                    "Center".into()
                                } else {
                                    format!("Anchor {}", i + 1)
                                },
                                rest: Point::new(*x, *y),
                                keys: vec![PinKey {
                                    frame: 0,
                                    offset: Point::default(),
                                }],
                            })
                            .collect(),
                    });
                    changed = true;
                }
                if let Some(mesh) = raster.mesh.as_mut() {
                    ui.label(
                        "Offsets are fractions of artwork size. Changes key the current frame.",
                    );
                    for pin in &mut mesh.pins {
                        ui.label(&pin.name);
                        let mut offset = pin
                            .keys
                            .iter()
                            .rev()
                            .find(|k| k.frame <= self.frame as u32)
                            .map_or(Point::default(), |k| k.offset);
                        let mut edit = false;
                        edit |= ui
                            .add(egui::Slider::new(&mut offset.x, -0.5..=0.5).text("X"))
                            .changed();
                        edit |= ui
                            .add(egui::Slider::new(&mut offset.y, -0.5..=0.5).text("Y"))
                            .changed();
                        if edit {
                            pin.keys.retain(|k| k.frame != self.frame as u32);
                            pin.keys.push(PinKey {
                                frame: self.frame as u32,
                                offset,
                            });
                            pin.keys.sort_by_key(|k| k.frame);
                            changed = true;
                        }
                    }
                }
            });
            if changed {
                self.apply(vec![Command::PutRasterLayer { layer: raster }]);
            }
            if ui.button("Delete layer").clicked() {
                self.apply(vec![Command::Remove { id: source.id }]);
                self.artwork.selected = None;
            }
        }
        ui.separator();
        self.voice_artwork_panel(ui, p);
    }
    pub(super) fn voice_artwork_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.collapsing("Voice & mouth timing",|ui|{
            if let Some(recording)=self.artwork.recording.as_ref(){
                ui.label(format!("Recording {:.1} / 60 s",recording.seconds()));
                ui.ctx().request_repaint_after(Duration::from_millis(100));
                if ui.add_enabled(self.artwork.pending.is_none(),egui::Button::new("Stop & add audio")).clicked(){self.finish_recording();}
            }else if ui.add_enabled(self.artwork.pending.is_none(),egui::Button::new("Record microphone")).clicked(){
                self.stop();match crate::recording::Recording::start(){
                    Ok(recording)=>{self.artwork.recording=Some(recording);self.artwork.recording_target=Some((p.id,self.frame as u32));},
                    Err(error)=>self.status=error.to_string(),
                }
            }
            egui::ComboBox::from_id_salt("mouth.audio").selected_text(self.artwork.clip.and_then(|id|p.audio.iter().find(|a|a.id==id)).map_or("Choose audio",|a|a.name.as_str())).show_ui(ui,|ui|{for audio in &p.audio{ui.selectable_value(&mut self.artwork.clip,Some(audio.id),&audio.name);}});
            ui.add(egui::TextEdit::multiline(&mut self.artwork.text).hint_text("Optional transcript").desired_width(ui.available_width()));
            if ui.add_enabled(self.selected.is_some()&&self.artwork.clip.is_some()&&self.artwork.pending.is_none(),egui::Button::new("Generate mouth cues")).clicked(){
                self.artwork_request(ArtworkRequest::LipSync{project:p.id,expected_revision:p.revision,clip:self.artwork.clip.unwrap(),character:self.selected.unwrap(),text:self.artwork.text.clone(),threshold:None});
            }
            ui.label(RichText::new("Select a character first. Cues approximate sound energy; review the keys below.").size(11.).color(MUTED));
            for track in &p.mouths {
                ui.label(&track.name);
                let current=track.keys.iter().rev().find(|k|k.frame<=self.frame as u32).map_or(MouthShape::Rest,|k|k.shape);
                ui.horizontal_wrapped(|ui|{
                    for (shape,label) in [(MouthShape::Rest,"Rest"),(MouthShape::A,"A"),(MouthShape::E,"E"),(MouthShape::O,"O"),(MouthShape::F,"F"),(MouthShape::M,"M")] {
                        if ui.selectable_label(current==shape,label).clicked(){
                            let mut next=track.clone();next.keys.retain(|k|k.frame!=self.frame as u32);next.keys.push(MouthKey{frame:self.frame as u32,shape});next.keys.sort_by_key(|k|k.frame);self.apply(vec![Command::PutMouthTrack{track:next}]);
                        }
                    }
                });
                if ui.small_button("Delete mouth track").clicked(){self.apply(vec![Command::Remove{id:track.id}]);}
            }
        });
    }
    fn finish_recording(&mut self) {
        let Some(recording) = self.artwork.recording.take() else {
            return;
        };
        let Some((project, start)) = self.artwork.recording_target.take() else {
            return;
        };
        let capture = match recording.finish() {
            Ok(capture) => capture,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let service = self.service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.artwork.pending = Some(rx);
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<_> {
                let p = service.snapshot(project)?;
                anyhow::ensure!(
                    start < p.duration,
                    "Recording start is outside the edited timeline"
                );
                let seconds =
                    capture.samples.len() as f64 / capture.channels as f64 / capture.rate as f64;
                let bytes =
                    crate::recording::wav(&capture.samples, capture.rate, capture.channels)?;
                let path = format!("assets/recording-{}.wav", Id::new_v4());
                service
                    .project_workspace(project)
                    .atomic_write(&path, &bytes)?;
                let asset = Asset {
                    id: Id::new_v4(),
                    name: "Microphone recording".into(),
                    path,
                    kind: "audio".into(),
                    tags: vec!["recording".into()],
                };
                let audio = AudioClip {
                    id: Id::new_v4(),
                    name: asset.name.clone(),
                    asset: asset.id,
                    start,
                    offset_seconds: 0.,
                    duration_seconds: seconds.min((p.duration - start) as f64 / p.fps as f64),
                    gain: 1.,
                    fade_in: 0.,
                    fade_out: 0.,
                    looping: false,
                };
                service.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision: p.revision,
                        commands: vec![
                            Command::AddAsset { asset },
                            Command::AddAudio { clip: audio },
                        ],
                    },
                })
            })();
            let _ = tx.send(result);
        });
    }
    pub(super) fn artwork_canvas(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.artwork.paint {
            self.stage(ui, p);
            return;
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(r) = self
                .artwork
                .selected
                .and_then(|id| p.rasters.iter().find(|r| r.id == id))
            else {
                return;
            };
            let Some(b) = p
                .bitmaps
                .iter()
                .find(|b| b.id == r.bitmap_at(self.frame, p.fps))
            else {
                return;
            };
            let Ok(bitmap) = astral_render::decode_bitmap(b) else {
                return;
            };
            ui.label("Paint source • changes affect this frame only");
            let available = ui.available_size();
            let scale = (available.x / b.width as f32)
                .min(available.y / b.height as f32)
                .max(0.001);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(b.width as f32 * scale, b.height as f32 * scale),
                Sense::drag(),
            );
            ui.painter().rect_filled(rect, 0., Color32::from_gray(70));
            astral_render::paint_image(
                ui.painter(),
                &bitmap,
                &[
                    astral_render::TextureVertex {
                        position: Point::new(0., 0.),
                        uv: Point::new(0., 0.),
                    },
                    astral_render::TextureVertex {
                        position: Point::new(b.width as f32, 0.),
                        uv: Point::new(1., 0.),
                    },
                    astral_render::TextureVertex {
                        position: Point::new(b.width as f32, b.height as f32),
                        uv: Point::new(1., 1.),
                    },
                    astral_render::TextureVertex {
                        position: Point::new(0., b.height as f32),
                        uv: Point::new(0., 1.),
                    },
                ],
                &[0, 1, 2, 0, 2, 3],
                1.,
                rect,
                scale,
            );
            if response.drag_started() && self.artwork.pending.is_none() {
                self.artwork.points.clear();
                if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                    self.artwork.points.push(Point::new(
                        (origin.x - rect.left()) / scale,
                        (origin.y - rect.top()) / scale,
                    ));
                }
                self.stop();
            }
            if response.dragged()
                && self.artwork.pending.is_none()
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.artwork.points.push(Point::new(
                    (pos.x - rect.left()) / scale,
                    (pos.y - rect.top()) / scale,
                ));
            }
            if self.artwork.points.len() > 1 {
                ui.painter().add(egui::Shape::line(
                    self.artwork
                        .points
                        .iter()
                        .map(|q| Pos2::new(rect.left() + q.x * scale, rect.top() + q.y * scale))
                        .collect(),
                    Stroke::new(self.artwork.radius.max(0.5) * 2. * scale, self.draw_color),
                ));
            }
            if response.drag_stopped() && !self.artwork.points.is_empty() {
                if let Some(end) = response.interact_pointer_pos() {
                    self.artwork.points.push(Point::new(
                        (end.x - rect.left()) / scale,
                        (end.y - rect.top()) / scale,
                    ));
                }
                let points = std::mem::take(&mut self.artwork.points);
                self.artwork_request(ArtworkRequest::Paint {
                    project: p.id,
                    expected_revision: p.revision,
                    layer: r.id,
                    frame: self.frame as u32,
                    points,
                    radius: self.artwork.radius.max(0.5),
                    color: self.draw_color.to_array(),
                    erase: self.artwork.erase,
                });
            }
        });
    }
}
