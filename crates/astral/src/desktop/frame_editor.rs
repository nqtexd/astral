use super::*;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum DrawTool {
    #[default]
    Pencil,
    Eraser,
    Move,
    Hand,
    Nodes,
}
impl Studio {
    pub(super) fn cel_tracks(
        &mut self,
        ui: &mut egui::Ui,
        p: &Project,
        width: f32,
        gutter: f32,
        pixels: f32,
        height: f32,
    ) {
        for layer in p.layers.iter().rev() {
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [gutter, 24.],
                        egui::Button::new(&layer.name)
                            .selected(self.selected_layer == Some(layer.id)),
                    )
                    .clicked()
                {
                    self.selected_layer = Some(layer.id);
                    self.layer_rename = layer.name.clone();
                }
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(width - gutter - 12., height),
                    Sense::click_and_drag(),
                );
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 3., Color32::from_rgb(31, 34, 40));
                for cel in p.cels.iter().filter(|c| c.layer == layer.id) {
                    let key = Rect::from_min_max(
                        Pos2::new(rect.left() + cel.frame as f32 * pixels, rect.top() + 3.),
                        Pos2::new(
                            rect.left() + (p.cel_end(cel) + 1) as f32 * pixels,
                            rect.bottom() - 3.,
                        ),
                    );
                    painter.rect_filled(
                        key,
                        3.,
                        if cel.strokes.is_empty() {
                            Color32::from_rgb(61, 65, 72)
                        } else {
                            Color32::from_rgb(65, 95, 118)
                        },
                    );
                    painter.rect_filled(
                        Rect::from_min_size(
                            key.min + Vec2::new(2., 5.),
                            Vec2::new(3., key.height() - 10.),
                        ),
                        1.,
                        Color32::from_rgb(208, 170, 121),
                    );
                    if key.width() > 55. {
                        painter.text(
                            key.left_center() + Vec2::new(10., 0.),
                            egui::Align2::LEFT_CENTER,
                            if cel.strokes.is_empty() {
                                "Blank"
                            } else {
                                "Drawing"
                            },
                            egui::FontId::proportional(11.),
                            Color32::LIGHT_GRAY,
                        );
                    }
                }
                let x = rect.left() + self.frame as f32 * pixels;
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(1.5, Color32::from_rgb(209, 160, 112)),
                );
                if response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    self.selected_layer = Some(layer.id);
                    self.layer_rename = layer.name.clone();
                    self.seek_frame(
                        ((pos.x - rect.left()) / pixels)
                            .floor()
                            .clamp(0., (p.duration - 1) as f32) as u32,
                    );
                }
                if response.drag_started()
                    && !layer.locked
                    && let Some(pos) = ui.input(|i| i.pointer.press_origin())
                {
                    let frame = ((pos.x - rect.left()) / pixels)
                        .floor()
                        .clamp(0., (p.duration - 1) as f32) as u32;
                    if let Some(cel) = p.cel_at(layer.id, frame) {
                        self.stop();
                        self.drag_cel = Some((layer.id, cel.frame, pos.x));
                    }
                }
                if response.dragged()
                    && let (Some((id, from, origin)), Some(pos)) =
                        (self.drag_cel, response.interact_pointer_pos())
                    && id == layer.id
                {
                    let to = (from as f32 + (pos.x - origin) / pixels)
                        .round()
                        .clamp(0., (p.duration - 1) as f32);
                    let x = rect.left() + to * pixels;
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(3., theme::ACCENT),
                    );
                }
                if response.drag_stopped()
                    && let (Some((id, from, origin)), Some(pos)) =
                        (self.drag_cel.take(), response.interact_pointer_pos())
                {
                    let to = (from as f32 + (pos.x - origin) / pixels)
                        .round()
                        .clamp(0., (p.duration - 1) as f32) as u32;
                    if from != to {
                        self.apply(vec![Command::MoveCel {
                            layer: id,
                            from,
                            to,
                        }]);
                    }
                }
            });
        }
    }
    pub(super) fn add_drawing_layer(&mut self) {
        let Ok(p) = self.service.snapshot(self.project) else {
            return;
        };
        let layer = DrawingLayer::new(format!("Layer {}", p.layers.len() + 1));
        let id = layer.id;
        self.apply(vec![
            Command::AddLayer { layer },
            Command::SetCel {
                layer: id,
                frame: self.frame as u32,
                strokes: vec![],
            },
        ]);
        self.selected_layer = Some(id);
        self.layer_rename = format!("Layer {}", p.layers.len() + 1);
    }
    pub(super) fn drawing_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.left {
            return;
        }
        egui::Panel::left("drawing.layers")
            .default_size(224.)
            .min_size(188.)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("drawing.settings")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(i18n::tr("Layers"));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if theme::icon_button(
                                        ui,
                                        "Add drawing layer",
                                        theme::Icon::Plus,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        self.add_drawing_layer();
                                    }
                                },
                            );
                        });
                        ui.add_space(8.);
                        egui::ScrollArea::vertical()
                            .id_salt("drawing.layer-list")
                            .max_height(200.)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for layer in p.layers.iter().rev() {
                                    ui.push_id(layer.id, |ui| {
                                        egui::Frame::new()
                                            .fill(if self.selected_layer == Some(layer.id) {
                                                theme::SELECTED
                                            } else {
                                                theme::SURFACE
                                            })
                                            .corner_radius(9)
                                            .inner_margin(7.)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    let name_width =
                                                        (ui.available_width() - 76.).max(24.);
                                                    if ui
                                                        .add_sized(
                                                            [name_width, 32.],
                                                            egui::Button::new(&layer.name)
                                                                .selected(
                                                                    self.selected_layer
                                                                        == Some(layer.id),
                                                                )
                                                                .truncate(),
                                                        )
                                                        .clicked()
                                                    {
                                                        self.selected_layer = Some(layer.id);
                                                        self.layer_rename = layer.name.clone();
                                                    }
                                                    if theme::icon_button(
                                                        ui,
                                                        if layer.visible {
                                                            "Hide layer"
                                                        } else {
                                                            "Show layer"
                                                        },
                                                        if layer.visible {
                                                            theme::Icon::Eye
                                                        } else {
                                                            theme::Icon::EyeOff
                                                        },
                                                        false,
                                                    )
                                                    .clicked()
                                                    {
                                                        self.apply(vec![Command::SetLayer {
                                                            layer: layer.id,
                                                            name: None,
                                                            visible: Some(!layer.visible),
                                                            locked: None,
                                                            opacity: None,
                                                        }]);
                                                    }
                                                    if theme::icon_button(
                                                        ui,
                                                        if layer.locked {
                                                            "Unlock layer"
                                                        } else {
                                                            "Lock layer"
                                                        },
                                                        if layer.locked {
                                                            theme::Icon::Lock
                                                        } else {
                                                            theme::Icon::Unlock
                                                        },
                                                        layer.locked,
                                                    )
                                                    .clicked()
                                                    {
                                                        self.apply(vec![Command::SetLayer {
                                                            layer: layer.id,
                                                            name: None,
                                                            visible: None,
                                                            locked: Some(!layer.locked),
                                                            opacity: None,
                                                        }]);
                                                    }
                                                });
                                            });
                                        ui.add_space(5.);
                                    });
                                }
                            });
                        if let Some(layer) =
                            p.layers.iter().find(|l| Some(l.id) == self.selected_layer)
                        {
                            ui.add_space(6.);
                            ui.horizontal(|ui| {
                                if theme::icon_button(ui, "Move layer up", theme::Icon::Up, false)
                                    .clicked()
                                {
                                    let i = p.layers.iter().position(|l| l.id == layer.id).unwrap();
                                    if i + 1 < p.layers.len() {
                                        self.apply(vec![Command::MoveLayer {
                                            layer: layer.id,
                                            index: i + 1,
                                        }]);
                                    }
                                }
                                if theme::icon_button(
                                    ui,
                                    "Move layer down",
                                    theme::Icon::Down,
                                    false,
                                )
                                .clicked()
                                {
                                    let i = p.layers.iter().position(|l| l.id == layer.id).unwrap();
                                    if i > 0 {
                                        self.apply(vec![Command::MoveLayer {
                                            layer: layer.id,
                                            index: i - 1,
                                        }]);
                                    }
                                }
                                theme::icon_menu(ui, "Layer options", theme::Icon::More, |ui| {
                                    ui.label(i18n::tr("Layer name"));
                                    ui.text_edit_singleline(&mut self.layer_rename);
                                    if ui.button(i18n::tr("Rename")).clicked() {
                                        self.apply(vec![Command::SetLayer {
                                            layer: layer.id,
                                            name: Some(self.layer_rename.trim().into()),
                                            visible: None,
                                            locked: None,
                                            opacity: None,
                                        }]);
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .add_enabled(
                                            !layer.locked,
                                            egui::Button::new("Delete layer"),
                                        )
                                        .clicked()
                                    {
                                        self.apply(vec![Command::Remove { id: layer.id }]);
                                        self.selected_layer = p
                                            .layers
                                            .iter()
                                            .rev()
                                            .find(|l| l.id != layer.id)
                                            .map(|l| l.id);
                                        ui.close();
                                    }
                                });
                            });
                            let mut opacity = self
                                .layer_opacity
                                .filter(|(id, _)| *id == layer.id)
                                .map_or(layer.opacity, |(_, value)| value);
                            let response = theme::slider(
                                ui,
                                "Opacity",
                                egui::Slider::new(&mut opacity, 0.0..=1.)
                                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.))
                                    .custom_parser(|s| {
                                        s.trim_end_matches('%')
                                            .parse::<f64>()
                                            .ok()
                                            .map(|v| v / 100.)
                                    }),
                            );
                            self.layer_opacity = if response.dragged() {
                                Some((layer.id, opacity))
                            } else {
                                None
                            };
                            if response.drag_stopped()
                                || (response.changed() && !response.dragged())
                            {
                                self.apply(vec![Command::SetLayer {
                                    layer: layer.id,
                                    name: None,
                                    visible: None,
                                    locked: None,
                                    opacity: Some(opacity),
                                }]);
                                self.layer_opacity = None;
                            }
                            if layer.locked {
                                ui.label(
                                    RichText::new("Unlock this layer to draw.")
                                        .small()
                                        .color(MUTED),
                                );
                            }
                            if !layer.visible {
                                ui.label(
                                    RichText::new("This layer is hidden.").small().color(MUTED),
                                );
                            }
                        } else {
                            ui.label(i18n::tr("Add a layer to start drawing."));
                        }
                        ui.add_space(16.);
                        ui.separator();
                        theme::section(ui, "BRUSH");
                        egui::ComboBox::from_id_salt("brush.style").selected_text(format!("{:?}",self.preferences.brush_style)).show_ui(ui,|ui|{for style in [BrushStyle::Solid,BrushStyle::Marker,BrushStyle::Dashed,BrushStyle::Stipple]{ui.selectable_value(&mut self.preferences.brush_style,style,format!("{style:?}"));}});
                        ui.checkbox(&mut self.preferences.pressure_brush,i18n::tr("Pressure controls width")).on_hover_text("Uses normalized pressure reported by the device. Mouse uses full pressure. Tablet support depends on the operating-system input driver.");
                        ui.checkbox(&mut self.preferences.curved_brush,i18n::tr("Cubic smoothing"));ui.checkbox(&mut self.preferences.filled_brush,i18n::tr("Closed fill"));
                        if theme::button(ui,"Edit points",theme::Icon::Pencil,self.draw_tool==DrawTool::Nodes).clicked(){self.draw_tool=DrawTool::Nodes;}

                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut self.draw_color);
                        });
                        ui.horizontal_wrapped(|ui| {
                            for color in [
                                Color32::from_rgb(30, 32, 37),
                                Color32::WHITE,
                                Color32::from_rgb(203, 62, 62),
                                Color32::from_rgb(230, 146, 49),
                                Color32::from_rgb(226, 194, 73),
                                Color32::from_rgb(65, 147, 103),
                                Color32::from_rgb(58, 121, 185),
                                Color32::from_rgb(133, 88, 167),
                            ] {
                                let (rect, r) =
                                    ui.allocate_exact_size(Vec2::splat(21.), Sense::click());
                                ui.painter().rect_filled(rect.shrink(2.), 3., color);
                                if self.draw_color == color {
                                    ui.painter().rect_stroke(
                                        rect,
                                        4.,
                                        Stroke::new(1., Color32::LIGHT_GRAY),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if r.clicked() {
                                    self.draw_color = color;
                                }
                            }
                        });
                        theme::slider(
                            ui,
                            "Size",
                            egui::Slider::new(&mut self.draw_width, 0.5..=64.),
                        );
                        ui.add_space(12.);
                        ui.separator();
                        ui.checkbox(&mut self.preferences.single_frame_drawing, i18n::tr("Single-frame drawing"))
                            .on_hover_text("Manual strokes affect only this frame. Disable to hold drawings until the next key; use Frame exposure for an intentional hold.");
                        ui.checkbox(&mut self.preferences.onion, i18n::tr("Onion skin"));
                        if self.preferences.onion {
                            egui::Grid::new("drawing.onion-counts")
                                .num_columns(2)
                                .show(ui, |ui| {
                                    ui.label(i18n::tr("Before"));
                                    ui.add(
                                        egui::DragValue::new(&mut self.preferences.onion_before)
                                            .range(0..=4),
                                    );
                                    ui.end_row();
                                    ui.label(i18n::tr("After"));
                                    ui.add(
                                        egui::DragValue::new(&mut self.preferences.onion_after)
                                            .range(0..=4),
                                    );
                                    ui.end_row();
                                });
                            theme::slider(
                                ui,
                                "Opacity",
                                egui::Slider::new(&mut self.preferences.onion_opacity, 0.05..=0.6),
                            );
                        }
                        if !p.drawings.is_empty() {
                            ui.add_space(10.);
                            ui.collapsing(i18n::tr("Existing drawing tools"), |ui| self.feature_panel(ui, p));
                        }
                    });
            });
    }
    fn current_strokes(&self, p: &Project) -> Vec<InkStroke> {
        self.selected_layer
            .and_then(|id| p.cel_at(id, self.frame as u32))
            .map(|c| c.strokes.clone())
            .unwrap_or_default()
    }
    pub(super) fn set_frame_strokes(&mut self, strokes: Vec<InkStroke>) {
        if let Some(layer) = self.selected_layer {
            self.apply(vec![self.drawing_command(
                layer,
                self.frame as u32,
                strokes,
            )]);
        }
    }
    pub(super) fn drawing_command(
        &self,
        layer: Id,
        frame: u32,
        strokes: Vec<InkStroke>,
    ) -> Command {
        if self.preferences.single_frame_drawing {
            Command::SetFrameDrawing {
                layer,
                frame,
                strokes,
            }
        } else {
            Command::SetCel {
                layer,
                frame,
                strokes,
            }
        }
    }
    pub(super) fn frame_action(&mut self, p: &Project, action: &str) {
        if action == "Copy" && self.motion_copy_range(p) {
            return;
        }
        if action == "Paste" && self.motion_paste_range(p) {
            return;
        }
        if action == "Copy" {
            self.motion.clipboard = None;
        }
        let Some(layer) = self.selected_layer else {
            self.status = "Select a drawing layer first".into();
            return;
        };
        let frame = self.frame as u32;
        match action {
            "Copy" => {
                self.frame_clipboard = Some(self.current_strokes(p));
                self.status = "Drawing copied from the active layer".into();
            }
            "Paste" => {
                if let Some(strokes) = self.frame_clipboard.clone() {
                    self.set_frame_strokes(strokes);
                }
            }
            "Clear" => self.set_frame_strokes(vec![]),
            "Blank" | "Duplicate" => {
                let next = frame + 1;
                let strokes = if action == "Duplicate" {
                    self.current_strokes(p)
                } else {
                    vec![]
                };
                let mut commands = vec![];
                let shift = p.cels.iter().any(|c| c.layer == layer && c.frame >= next);
                if next >= p.duration || shift {
                    commands.push(Command::SetProject {
                        name: None,
                        duration: Some(p.duration + 1),
                        background: None,
                    });
                }
                if shift {
                    commands.push(Command::ShiftCels {
                        layer,
                        start: next,
                        delta: 1,
                    });
                }
                commands.push(self.drawing_command(layer, next, strokes));
                let before = p.revision;
                self.apply(commands);
                if self
                    .service
                    .snapshot(self.project)
                    .is_ok_and(|p| p.revision > before)
                {
                    self.stop();
                    self.frame = next as f64;
                    self.follow_frame = true;
                }
            }
            "Delete" => {
                if self.preferences.single_frame_drawing {
                    self.set_frame_strokes(vec![]);
                } else if p.cels.iter().any(|c| c.layer == layer && c.frame == frame) {
                    self.apply(vec![Command::DeleteCel { layer, frame }]);
                } else {
                    self.status =
                        "This frame holds an earlier drawing. Clear creates a blank key here."
                            .into();
                }
            }
            _ => {}
        }
    }
    pub(super) fn frame_strip(&mut self, ui: &mut egui::Ui, p: &Project) {
        if !self.preferences.timeline {
            return;
        }
        egui::Panel::bottom("drawing.frames")
            .default_size(152.)
            .min_size(144.)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("frames.controls")
                    .auto_shrink([false, true])
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
                            if theme::icon_button(
                                ui,
                                "Previous frame · Left arrow",
                                theme::Icon::Previous,
                                false,
                            )
                            .clicked()
                            {
                                self.seek_frame((self.frame as u32).saturating_sub(1));
                            }
                            if theme::icon_button(
                                ui,
                                "Next frame · Right arrow",
                                theme::Icon::Next,
                                false,
                            )
                            .clicked()
                            {
                                self.seek_frame((self.frame as u32 + 1).min(p.duration - 1));
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} / {}",
                                    self.frame as u32 + 1,
                                    p.duration
                                ))
                                .monospace(),
                            );
                            ui.label(RichText::new(format!("{} fps", p.fps)).small().color(MUTED));
                            ui.separator();
                            let editable = self
                                .selected_layer
                                .is_some_and(|id| p.layers.iter().any(|l| l.id == id && !l.locked));
                            if self.preferences.mode != WorkspaceMode::Artwork {
                                for (action, icon) in [
                                    ("Blank", theme::Icon::Blank),
                                    ("Duplicate", theme::Icon::Duplicate),
                                    ("Copy", theme::Icon::Copy),
                                    ("Paste", theme::Icon::Paste),
                                    ("Clear", theme::Icon::Clear),
                                    ("Delete", theme::Icon::Delete),
                                ] {
                                    let enabled = if action == "Copy" {
                                        self.selected_layer.is_some()
                                    } else {
                                        editable
                                            && (action != "Paste" || self.frame_clipboard.is_some())
                                    };
                                    if ui
                                        .add_enabled(enabled, theme::IconButton::new(action, icon))
                                        .clicked()
                                    {
                                        self.frame_action(p, action);
                                    }
                                }
                            }
                            if let Some(cel) = self
                                .selected_layer
                                .and_then(|id| p.cel_at(id, self.frame as u32))
                            {
                                theme::icon_menu(ui, "Frame exposure", theme::Icon::Hold, |ui| {
                                    ui.label(format!("Key begins at frame {}", cel.frame + 1));
                                    ui.label(format!(
                                        "Exposure: {} frames",
                                        p.cel_end(cel) - cel.frame + 1
                                    ));
                                    if ui.button(i18n::tr("Make one frame")).clicked() {
                                        self.apply(vec![Command::SetExposure {
                                            layer: cel.layer,
                                            frame: cel.frame,
                                            frames: 1,
                                        }]);
                                        ui.close();
                                    }
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.hold_frames)
                                                .range(1..=10000),
                                        );
                                        ui.label(i18n::tr("frames"));
                                    });
                                    if ui.button(i18n::tr("Set exposure")).clicked() {
                                        self.apply(vec![Command::SetExposure {
                                            layer: cel.layer,
                                            frame: cel.frame,
                                            frames: self.hold_frames,
                                        }]);
                                        ui.close();
                                    }
                                });
                            }
                            ui.separator();
                            if theme::icon_button(
                                ui,
                                "Onion skin",
                                theme::Icon::Onion,
                                self.preferences.onion,
                            )
                            .clicked()
                            {
                                self.preferences.onion = !self.preferences.onion;
                            }
                            if theme::icon_button(
                                ui,
                                "Detailed timeline",
                                theme::Icon::Timeline,
                                self.preferences.advanced_timeline,
                            )
                            .clicked()
                            {
                                self.preferences.advanced_timeline =
                                    !self.preferences.advanced_timeline;
                            }
                        });
                    });
                ui.add_space(6.);
                let layer = self.selected_layer;
                let width = self.preferences.frame_cell_width.clamp(56., 128.);
                let height = 83.;
                if self
                    .frame_cache
                    .keys()
                    .next()
                    .is_some_and(|k| k.0 != p.id || k.1 != p.revision)
                {
                    self.frame_cache.clear();
                }
                egui::ScrollArea::horizontal()
                    .id_salt(("frame.strip", p.id))
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        let (all, _) = ui.allocate_exact_size(
                            Vec2::new(p.duration as f32 * width, height),
                            Sense::hover(),
                        );
                        let first = (viewport.min.x / width).floor().max(0.) as u32;
                        let last = ((viewport.max.x / width).ceil() as u32 + 1).min(p.duration);
                        for frame in first..last {
                            let rect = Rect::from_min_size(
                                all.min + Vec2::new(frame as f32 * width, 0.),
                                Vec2::new(width - 6., height),
                            );
                            let response =
                                ui.interact(rect, ui.scope_id().with(frame), Sense::click());
                            let selected = self.frame as u32 == frame;
                            ui.painter().rect_filled(
                                rect,
                                5.,
                                if selected {
                                    Color32::from_rgb(57, 66, 77)
                                } else {
                                    Color32::from_rgb(33, 36, 42)
                                },
                            );
                            let key = (p.id, p.revision, frame);
                            if !self.frame_cache.contains_key(&key) {
                                if self.frame_cache.len() >= 128 {
                                    self.frame_cache.clear();
                                }
                                if let Ok(pix) = astral_render::rasterize_framed(
                                    &display_list(p, frame as f64),
                                    144,
                                    90,
                                    false,
                                ) {
                                    let texture = ui.ctx().load_texture(
                                        format!("frame-{frame}"),
                                        egui::ColorImage::from_rgba_premultiplied(
                                            [144, 90],
                                            pix.data(),
                                        ),
                                        egui::TextureOptions::LINEAR,
                                    );
                                    self.frame_cache.insert(key, texture);
                                }
                            }
                            if let Some(texture) = self.frame_cache.get(&key) {
                                ui.painter().image(
                                    texture.id(),
                                    Rect::from_min_size(
                                        rect.min + Vec2::splat(5.),
                                        Vec2::new(width - 16., 47.),
                                    ),
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                                    Color32::WHITE,
                                );
                            }
                            let authored = layer.is_some_and(|id| {
                                p.cels.iter().any(|c| c.layer == id && c.frame == frame)
                            });
                            ui.painter().text(
                                rect.left_bottom() + Vec2::new(8., -9.),
                                egui::Align2::LEFT_BOTTOM,
                                (frame + 1).to_string(),
                                egui::FontId::monospace(11.),
                                if selected { Color32::WHITE } else { MUTED },
                            );
                            if authored {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        rect.right_bottom() - Vec2::new(12., 18.),
                                        Vec2::splat(5.),
                                    ),
                                    1.,
                                    Color32::from_rgb(213, 159, 101),
                                );
                            }
                            if selected {
                                ui.painter().rect_stroke(
                                    rect,
                                    5.,
                                    Stroke::new(2., theme::ACCENT),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if self
                                .motion
                                .selection
                                .is_some_and(|(a, b)| frame >= a && frame <= b)
                            {
                                ui.painter().rect_stroke(
                                    rect,
                                    5.,
                                    Stroke::new(2., theme::ACCENT),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if response.clicked() {
                                if ui.input(|i| i.modifiers.shift) {
                                    let anchor = self.motion.anchor.unwrap_or(self.frame as u32);
                                    let range = (anchor.min(frame), anchor.max(frame));
                                    self.motion.selection = Some(range);
                                    self.motion.start = range.0;
                                    self.motion.end = range.1;
                                } else {
                                    self.motion.anchor = Some(frame);
                                    self.motion.selection = None;
                                }
                                self.seek_frame(frame);
                            }
                            response
                                .on_hover_text(if authored {
                                    "Drawing key on the active layer"
                                } else {
                                    "Held frame on the active layer"
                                })
                                .context_menu(|ui| {
                                    if ui.button(i18n::tr("Edit selected range…")).clicked() {
                                        self.motion.open = true;
                                        self.motion.tab = super::motion::Tab::Frames;
                                        ui.close();
                                    }
                                    for action in
                                        ["Blank", "Duplicate", "Copy", "Paste", "Clear", "Delete"]
                                    {
                                        if ui.button(action).clicked() {
                                            self.seek_frame(frame);
                                            self.frame_action(p, action);
                                            ui.close();
                                        }
                                    }
                                });
                        }
                        if self.follow_frame || self.playing {
                            let current = Rect::from_min_size(
                                all.min + Vec2::new(self.frame as f32 * width, 0.),
                                Vec2::new(width, height),
                            );
                            ui.scroll_to_rect(current, Some(egui::Align::Center));
                            self.follow_frame = false;
                        }
                    });
            });
    }
    pub(super) fn seek_frame(&mut self, frame: u32) {
        self.stop();
        self.frame = frame as f64;
        self.follow_frame = true;
        self.context.request_repaint();
    }
    pub(super) fn editor_shortcuts(&mut self, ctx: &egui::Context) {
        if self.manager.home || ctx.egui_wants_keyboard_input() {
            return;
        }
        let Ok(p) = self.service.snapshot(self.project) else {
            return;
        };
        let input = ctx.input(|i| {
            (
                keymap::pressed(i, &self.preferences.shortcuts, "previous"),
                keymap::pressed(i, &self.preferences.shortcuts, "next"),
                keymap::pressed(i, &self.preferences.shortcuts, "copy"),
                keymap::pressed(i, &self.preferences.shortcuts, "paste"),
                keymap::pressed(i, &self.preferences.shortcuts, "duplicate"),
                keymap::pressed(i, &self.preferences.shortcuts, "pencil"),
                keymap::pressed(i, &self.preferences.shortcuts, "eraser"),
                keymap::pressed(i, &self.preferences.shortcuts, "move"),
                keymap::pressed(i, &self.preferences.shortcuts, "hand"),
                i.key_pressed(egui::Key::Home),
                i.modifiers.command,
            )
        });
        if input.0 {
            self.seek_frame((self.frame as u32).saturating_sub(1));
        }
        if input.1 {
            self.seek_frame((self.frame as u32 + 1).min(p.duration - 1));
        }
        if self.preferences.mode == WorkspaceMode::Draw {
            if input.2 {
                self.frame_action(&p, "Copy");
            }
            if input.3 {
                self.frame_action(&p, "Paste");
            }
            if input.4 {
                self.frame_action(&p, "Duplicate");
            }
            {
                if input.5 {
                    self.draw_tool = DrawTool::Pencil;
                }
                if input.6 {
                    self.draw_tool = DrawTool::Eraser;
                }
                if input.7 {
                    self.draw_tool = DrawTool::Move;
                }
                if input.8 {
                    self.draw_tool = DrawTool::Hand;
                }
            }
        }
        if input.9 {
            self.view_zoom = 1.;
            self.view_pan = Vec2::ZERO;
        }
    }
    pub(super) fn draw_stage(&mut self, ui: &mut egui::Ui, p: &Project) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(rgba(self.preferences.canvas_surround))
                    .inner_margin(20.),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let name = p
                        .layers
                        .iter()
                        .find(|l| Some(l.id) == self.selected_layer)
                        .map_or("No layer", |l| l.name.as_str());
                    ui.label(RichText::new(name).small().color(MUTED));
                    ui.label(
                        RichText::new(format!("Frame {}", self.frame as u32 + 1))
                            .small()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                theme::IconButton::new("Fit canvas · Home", theme::Icon::Fit)
                                    .compact(),
                            )
                            .clicked()
                        {
                            self.view_zoom = 1.;
                            self.view_pan = Vec2::ZERO;
                        }
                        ui.label(
                            RichText::new(format!("{:.0}%", self.view_zoom * 100.))
                                .small()
                                .color(MUTED),
                        );
                    });
                });
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 36.),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_height(36.);
                        self.suggestion_controls(ui);
                    },
                );
                ui.add_space(8.);
                let size = ui.available_size();
                let (_, area) = ui.allocate_space(size);
                let response = ui.interact(
                    area,
                    ui.scope_id().with("drawing-canvas"),
                    Sense::click_and_drag(),
                );
                let fit = (area.width() / p.width as f32)
                    .min(area.height() / p.height as f32)
                    .max(0.001);
                if response.hovered() {
                    let delta = ui.input(|i| i.smooth_scroll_delta.y);
                    if delta != 0.
                        && let Some(pointer) = response.hover_pos()
                    {
                        let old = self.view_zoom;
                        self.view_zoom = (old * (delta * 0.003).exp()).clamp(0.1, 20.);
                        self.view_pan = pointer
                            - area.center()
                            - (pointer - area.center() - self.view_pan) * (self.view_zoom / old);
                    }
                }
                let scale = fit * self.view_zoom;
                let rect = Rect::from_center_size(
                    area.center() + self.view_pan,
                    Vec2::new(p.width as f32 * scale, p.height as f32 * scale),
                );
                let painter = ui.painter_at(rect.intersect(area));
                // A neutral border makes the page boundary clear without decorative graphics.
                ui.painter_at(area).rect_stroke(
                    rect.expand(1.),
                    0.,
                    Stroke::new(1., Color32::from_rgb(12, 13, 15)),
                    egui::StrokeKind::Outside,
                );
                if p.background[3] < 255 {
                    let cell = 16.;
                    let visible = rect.intersect(area);
                    let cols = (visible.width() / cell).ceil() as i32;
                    let rows = (visible.height() / cell).ceil() as i32;
                    for y in 0..rows {
                        for x in 0..cols {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    visible.min + Vec2::new(x as f32 * cell, y as f32 * cell),
                                    Vec2::splat(cell),
                                ),
                                0.,
                                if (x + y) % 2 == 0 {
                                    Color32::from_gray(205)
                                } else {
                                    Color32::from_gray(235)
                                },
                            );
                        }
                    }
                }
                painter.rect_filled(rect, 0., rgba(p.background));
                let scene = evaluate(p, self.frame);
                let to_world = |point: Pos2| {
                    Point::new(
                        ((point.x - rect.left()) / scale - p.width as f32 / 2.) / scene.zoom
                            + scene.center.x,
                        ((point.y - rect.top()) / scale - p.height as f32 / 2.) / scene.zoom
                            + scene.center.y,
                    )
                };
                let to_screen = |point: Point| {
                    Pos2::new(
                        rect.left()
                            + ((point.x - scene.center.x) * scene.zoom + p.width as f32 / 2.)
                                * scale,
                        rect.top()
                            + ((point.y - scene.center.y) * scene.zoom + p.height as f32 / 2.)
                                * scale,
                    )
                };
                if self.preferences.onion
                    && let Some(layer) = self.selected_layer.filter(|id| {
                        p.layers
                            .iter()
                            .any(|l| l.id == *id && l.visible && l.opacity > 0.)
                    })
                {
                    let active = p.cel_at(layer, self.frame as u32).map(|c| c.frame);
                    let mut before: Vec<_> = p
                        .cels
                        .iter()
                        .filter(|c| {
                            c.layer == layer
                                && c.frame < self.frame as u32
                                && Some(c.frame) != active
                        })
                        .collect();
                    before.sort_by_key(|c| std::cmp::Reverse(c.frame));
                    let mut after: Vec<_> = p
                        .cels
                        .iter()
                        .filter(|c| c.layer == layer && c.frame > self.frame as u32)
                        .collect();
                    after.sort_by_key(|c| c.frame);
                    for (cels, count, color) in [
                        (
                            before,
                            self.preferences.onion_before,
                            Color32::from_rgb(72, 126, 175),
                        ),
                        (
                            after,
                            self.preferences.onion_after,
                            Color32::from_rgb(187, 89, 86),
                        ),
                    ] {
                        for (index, cel) in cels.into_iter().take(count as usize).enumerate() {
                            for stroke in &cel.strokes {
                                let tint = color.gamma_multiply(
                                    self.preferences.onion_opacity / (index + 1) as f32,
                                );
                                painter.add(egui::Shape::line(
                                    stroke.points.iter().map(|p| to_screen(*p)).collect(),
                                    Stroke::new(stroke.width * scale * scene.zoom, tint),
                                ));
                            }
                        }
                    }
                }
                self.handle_library_drop(p, ui, rect.intersect(area), to_world);
                self.paint_drawing_suggestion(p, &painter, to_screen, scale * scene.zoom);
                if self.preferences.show_grid {
                    let grid = painter.with_clip_rect(rect);
                    let step = (48. * scale).max(12.);
                    let color = Color32::from_rgba_unmultiplied(125, 135, 150, 45);
                    let mut x = rect.left();
                    while x < rect.right() {
                        grid.line_segment(
                            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                            Stroke::new(0.5, color),
                        );
                        x += step;
                    }
                    let mut y = rect.top();
                    while y < rect.bottom() {
                        grid.line_segment(
                            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                            Stroke::new(0.5, color),
                        );
                        y += step;
                    }
                }
                let mut preview = None;
                if let (Some(layer), Some(strokes)) = (self.selected_layer, &self.cel_work) {
                    let mut value = p.clone();
                    value
                        .cels
                        .retain(|c| !(c.layer == layer && c.frame == self.frame as u32));
                    value.cels.push(DrawingCel {
                        id: Id::nil(),
                        layer,
                        frame: self.frame as u32,
                        strokes: strokes.clone(),
                    });
                    preview = Some(value);
                }
                paint_list(
                    &painter,
                    &display_list(preview.as_ref().unwrap_or(p), self.frame),
                    rect,
                    scale,
                    None,
                );
                let pan = response.dragged_by(egui::PointerButton::Middle)
                    || (self.draw_tool == DrawTool::Hand && response.dragged());
                if pan {
                    self.view_pan += response.drag_delta();
                    self.stroke.clear();
                    return;
                }
                let editable = self.selected_layer.is_some_and(|id| {
                    p.layers
                        .iter()
                        .any(|l| l.id == id && !l.locked && l.visible)
                });
                if response.hovered() {
                    ui.ctx().set_cursor_icon(match self.draw_tool {
                        DrawTool::Hand => egui::CursorIcon::Grab,
                        DrawTool::Move => egui::CursorIcon::Move,
                        _ => egui::CursorIcon::Crosshair,
                    });
                }
                let primary_start = response.drag_started_by(egui::PointerButton::Primary);
                if (primary_start || response.clicked()) && !editable {
                    self.status = "Select a visible, unlocked layer to draw".into();
                    return;
                }
                if !editable {
                    return;
                }
                let pointer = response
                    .interact_pointer_pos()
                    .filter(|pos| rect.contains(*pos) && area.contains(*pos));
                if primary_start {
                    self.stop();
                    self.assist.proposal = None;
                    self.stroke.clear();
                    self.stroke_pressure.clear();
                    if self.draw_tool == DrawTool::Pencil
                        && let Some(origin) = ui
                            .input(|i| i.pointer.press_origin())
                            .filter(|pos| rect.contains(*pos) && area.contains(*pos))
                    {
                        self.stroke.push(to_world(origin));
                        self.stroke_pressure.push(1.);
                    }
                    self.cel_origin = ui
                        .input(|i| i.pointer.press_origin())
                        .or(pointer)
                        .map(to_world);
                    if self.draw_tool != DrawTool::Pencil {
                        self.cel_base = self.current_strokes(p);
                        self.cel_work = Some(self.cel_base.clone());
                    }
                }
                if self.draw_tool == DrawTool::Nodes {
                    let strokes = self
                        .cel_work
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| self.current_strokes(p));
                    for (si, stroke) in strokes.iter().enumerate() {
                        for (pi, point) in stroke.world_node_points(&scene).iter().enumerate() {
                            let screen = to_screen(*point);
                            painter.circle_filled(screen, 3.5, theme::ACCENT);
                            if primary_start
                                && pointer.is_some_and(|pos| pos.distance(screen) < 10.)
                            {
                                self.node_drag = Some((si, pi));
                            }
                        }
                    }
                    if response.dragged()
                        && let (Some((si, pi)), Some(pos)) = (self.node_drag, pointer)
                    {
                        let mut next = strokes;
                        if let Some(stroke) = next.get_mut(si) {
                            if stroke.skin.is_some() {
                                stroke.points = stroke.world_node_points(&scene);
                                stroke.skin = None;
                            }
                            if let Some(point) = stroke.points.get_mut(pi) {
                                *point = to_world(pos);
                                self.cel_work = Some(next);
                            }
                        }
                    }
                    if response.drag_stopped() {
                        self.node_drag = None;
                        if let Some(next) = self.cel_work.take() {
                            self.set_frame_strokes(next);
                        }
                    }
                } else if self.draw_tool == DrawTool::Pencil {
                    if (primary_start
                        || response.dragged_by(egui::PointerButton::Primary)
                        || response.clicked())
                        && let Some(pos) = pointer
                    {
                        let point = to_world(pos);
                        if self
                            .stroke
                            .last()
                            .is_none_or(|q| q.distance(point) > 0.3 / scale)
                        {
                            self.stroke.push(point);
                            let pressure = if self.preferences.pressure_brush {
                                ui.input(|i| {
                                    i.events.iter().rev().find_map(|e| match e {
                                        egui::Event::Touch { force: Some(f), .. } => {
                                            Some(f.clamp(0., 1.))
                                        }
                                        _ => None,
                                    })
                                })
                                .unwrap_or(1.)
                            } else {
                                1.
                            };
                            self.stroke_pressure.push(pressure);
                        }
                    }
                    if self.stroke.len() > 1 {
                        painter.add(egui::Shape::line(
                            self.stroke.iter().map(|q| to_screen(*q)).collect(),
                            Stroke::new(self.draw_width * scale * scene.zoom, self.draw_color),
                        ));
                    }
                    if (response.drag_stopped_by(egui::PointerButton::Primary)
                        || response.clicked())
                        && !self.stroke.is_empty()
                    {
                        let mut points = std::mem::take(&mut self.stroke);
                        if points.len() == 1 {
                            points.push(points[0] + Point::new(0.001, 0.001));
                        }
                        let mut strokes = self.current_strokes(p);
                        strokes.push(InkStroke {
                            bitmap: None,
                            skin: None,
                            pressures: {
                                let mut pressure = std::mem::take(&mut self.stroke_pressure);
                                pressure.resize(points.len(), *pressure.last().unwrap_or(&1.));
                                if self.preferences.pressure_brush
                                    && pressure.iter().any(|v| (*v - 1.).abs() > 0.0001)
                                {
                                    pressure
                                } else {
                                    vec![]
                                }
                            },
                            style: self.preferences.brush_style,
                            filled: self.preferences.filled_brush,
                            curved: self.preferences.curved_brush,
                            points,
                            color: self.draw_color.to_srgba_unmultiplied(),
                            width: self.draw_width,
                        });
                        if strokes != self.current_strokes(p) {
                            self.set_frame_strokes(strokes);
                            self.suggest_after_stroke();
                        }
                    }
                } else if self.draw_tool == DrawTool::Eraser {
                    if response.clicked() {
                        self.cel_work = Some(self.current_strokes(p));
                    }
                    if let Some(pos) = pointer {
                        let point = to_world(pos);
                        let radius = self.draw_width.max(8.) / scene.zoom;
                        if let Some(strokes) = &mut self.cel_work {
                            let origin = self.cel_origin.unwrap_or(point);
                            strokes.retain(|s| {
                                !s.points.windows(2).any(|pair| {
                                    swept_distance(origin, point, pair[0], pair[1])
                                        <= radius + s.width / 2.
                                })
                            });
                            self.cel_origin = Some(point);
                        }
                        painter.circle_stroke(
                            pos,
                            radius * scale * scene.zoom,
                            Stroke::new(1., Color32::from_gray(120)),
                        );
                    }
                    if (response.drag_stopped_by(egui::PointerButton::Primary)
                        || response.clicked())
                        && let Some(strokes) = self.cel_work.take()
                        && strokes != self.current_strokes(p)
                    {
                        self.set_frame_strokes(strokes);
                    }
                } else if self.draw_tool == DrawTool::Move {
                    if response.dragged_by(egui::PointerButton::Primary)
                        && let (Some(origin), Some(pos)) = (self.cel_origin, pointer)
                    {
                        let delta = to_world(pos) - origin;
                        self.cel_work = Some(
                            self.cel_base
                                .iter()
                                .map(|s| {
                                    let mut value = s.clone();
                                    for q in &mut value.points {
                                        *q = *q + delta;
                                    }
                                    value
                                })
                                .collect(),
                        );
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary)
                        && let Some(strokes) = self.cel_work.take()
                    {
                        if strokes != self.current_strokes(p) {
                            self.set_frame_strokes(strokes);
                        }
                        self.cel_origin = None;
                    }
                }
            });
    }
}
fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let d = b - a;
    let length = d.x * d.x + d.y * d.y;
    if length < 1e-8 {
        return p.distance(a);
    }
    let q = p - a;
    let t = ((q.x * d.x + q.y * d.y) / length).clamp(0., 1.);
    p.distance(Point::new(a.x + d.x * t, a.y + d.y * t))
}
fn swept_distance(a: Point, b: Point, c: Point, d: Point) -> f32 {
    let cross = |u: Point, v: Point| u.x * v.y - u.y * v.x;
    let ab = b - a;
    let cd = d - c;
    let divisor = cross(ab, cd);
    if divisor.abs() > 1e-8 {
        let t = cross(c - a, cd) / divisor;
        let u = cross(c - a, ab) / divisor;
        if (0.0..=1.).contains(&t) && (0.0..=1.).contains(&u) {
            return 0.;
        }
    }
    segment_distance(a, c, d)
        .min(segment_distance(b, c, d))
        .min(segment_distance(c, a, b))
        .min(segment_distance(d, a, b))
}
